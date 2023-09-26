//! Configuration structs and parser.

use std::collections::HashMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use uzers::uid_t;

use crate::workspace::Workspace;

#[cfg(test)]
mod tests;

/// Default value of `config` setting in control.
const DEFAULT_USER_CONFIG: &str = "~/.narrowssh.conf";

/// Default value of `authorized_keys` setting in control.
const DEFAULT_AUTHORIZED_KEYS: &str = "~/.ssh/authorized_keys";

/// Iterates over configuration file and its extensions and checks permissions.
///
/// In particular, `file` and the contents of `{file}.d` directory, if any, are
/// checked and passed to the consumer as [`Path`s][Path]. Readability of files
/// is not tested.
///
/// If `{file}` has an [extension][Path::extension()], only files with the same
/// extension will be considered inside `{file}.d`.
///
/// Symbolic links are always resolved.
///
/// # Errors
/// The function will fail in these cases:
///   - the consumer returns an error,
///   - some symbolic link could not be read,
///   - `{file}.d` exists but could not be read,
///   - `{file}.d` includes non-file extensions,
///   - some file is not owned by `owner`,
///   - `{file}.d` exists but is not owned by `owner`,
///   - some file has some world or group permissions, or
///   - `{file}.d` exists and has some world or group permissions.
///
/// The checks above are evaluated lazily, so `consumer` may be invoked even if
/// the function eventually fails.
pub fn visit_config_files<P, C, W>(
    file: P,
    owner: uid_t,
    mut consumer: C,
    ws: &W,
) -> Result<()>
where
    P: AsRef<Path>,
    C: FnMut(&Path) -> Result<()>,
    W: Workspace,
{
    // Runs safety checks.
    let perm_check = |file: &Path, expect_dir: bool| -> Result<()> {
        let suffix = "[security; refusing to proceed]";

        let metadata = std::fs::metadata(file)?;

        // Check file type
        if expect_dir {
            if !metadata.is_dir() {
                bail!("not a (symlink to a) directory {suffix}");
            }
        } else if !metadata.is_file() {
            bail!("not a (symlink to a) regular file {suffix}");
        }

        // Check permission bits
        let mode = metadata.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            bail!(
                "file has permissions {mode:o}, change to {:o} {suffix}",
                mode & 0o700
            );
        }

        // Check owner
        let actual_owner =
            ws.get_mock_owner_uid(file).unwrap_or(metadata.uid());
        if actual_owner != owner {
            bail!(
                "must be owned by UID {owner}, not {actual_owner} {suffix}"
            );
        }

        Ok(())
    };

    // Prepare paths
    let main_file = file.as_ref();
    let mut dir: std::ffi::OsString = main_file.into();
    dir.push(".d");
    let dir: std::path::PathBuf = dir.into();

    // Visit main file
    || -> Result<()> {
        perm_check(main_file, false)?;
        consumer(main_file)?;
        Ok(())
    }()
    .with_context(|| format!("loading main file {}", main_file.display()))?;

    // Try listing extensions
    let extensions = || -> Result<Option<Vec<PathBuf>>> {
        return match std::fs::read_dir(&dir) {
            Err(error) => {
                return if error.kind() == std::io::ErrorKind::NotFound {
                    // Extension directory does not exist - skip
                    Ok(None)
                } else {
                    Err(error.into())
                };
            }
            Ok(read_dir) => {
                let mut entries = read_dir
                    .map(|res| res.map(|e| e.path()))
                    .collect::<Result<Vec<_>, std::io::Error>>(
                )?;

                if let Some(main_ext) = main_file.extension() {
                    // Filter by extension
                    entries.retain(|p| p.extension() == Some(main_ext));
                    // Sort by name ensuring that "a.ext" < "a.a.ext"
                    entries.sort_by_key(|p| p.with_extension(""));
                } else {
                    entries.sort();
                }

                Ok(Some(entries))
            }
        };
    }()
    .with_context(|| format!("listing extensions in {}", dir.display()))?;

    // Visit extensions
    if let Some(dir_iter) = extensions {
        perm_check(&dir, true)?;
        for entry in dir_iter {
            || -> Result<()> {
                perm_check(&entry, false)?;
                consumer(&entry)?;
                Ok(())
            }()
            .with_context(|| {
                format!("loading extension file {}", entry.display())
            })?;
        }
    }

    Ok(())
}

/// Complete parsed configuration.
#[derive(Clone, Debug)]
pub struct Config {
    // TODO
}

/// A user's control settings.
#[derive(Clone, Debug)]
pub struct Control {
    /// Killswitch for all functionality.
    pub enable: bool,

    /// Path to user-defined config.
    ///
    /// The config extensions directory, referred to as `config.d`, is resolved
    /// to `config + '.d/'`.
    ///
    /// Note that this file, `config.d` and its contents must be owned by the
    /// user, and have modes `0600` or `0400` for files and `0700` or `0500`
    /// for `config.d`; otherwise, all user configuration is ignored.
    ///
    /// This path must either begin with a `/` to denote an absolute path,
    /// or with a `~` to denote a path relative to the home directory of the
    /// user. This path cannot end with a `/`.
    pub config: String,

    /// Path to the authorized_keys(5) file of this user.
    ///
    /// This path must either begin with a `/` to denote an absolute path,
    /// or with a `~` to denote a path relative to the home directory of the
    /// user. This path cannot end with a `/`.
    pub authorized_keys: String,
}

/// Copy of `Control` struct with every field wrapped in an Option.
#[derive(Debug, Deserialize)]
struct IncompleteControl {
    pub enable: Option<bool>,
    pub config: Option<String>,
    pub authorized_keys: Option<String>,
}

impl Control {
    fn fill_from(&mut self, source: &IncompleteControl) {
        if let Some(enable) = source.enable {
            self.enable = enable;
        }

        if let Some(config) = &source.config {
            self.config = config.clone();
        }

        if let Some(authorized_keys) = &source.authorized_keys {
            self.authorized_keys = authorized_keys.clone();
        }
    }
}

impl IncompleteControl {
    fn fill_from(&mut self, source: &IncompleteControl) {
        if let Some(enable) = source.enable {
            self.enable = Some(enable);
        }

        if let Some(config) = &source.config {
            self.config = Some(config.clone());
        }

        if let Some(authorized_keys) = &source.authorized_keys {
            self.authorized_keys = Some(authorized_keys.clone());
        }
    }
}

/// Manages the control settings for all users.
#[derive(Debug)]
pub struct ControlManager {
    /// Overrides for individual users.
    users: HashMap<uid_t, IncompleteControl>,

    /// Default values for all other users.
    fallback: Control,
}

impl ControlManager {
    /// Loads the control data from the filesystem.
    ///
    /// In particular, `from` and the contents of
    /// `{from}.d` directory are read and parsed.
    ///
    /// Symbolic links are always resolved.
    ///
    /// # Errors
    /// The load will fail in these cases:
    ///   - some file could not be read,
    ///   - some file is not a valid TOML file,
    ///   - some file is not structured as a control file, or
    ///   - [`visit_config_files`] complains.
    pub fn load<W, P>(ws: &W, from: P) -> Result<Self>
    where
        W: Workspace,
        P: AsRef<Path>,
    {
        let mut result = Self {
            users: HashMap::new(),
            fallback: Control {
                enable: false,
                config: String::from(DEFAULT_USER_CONFIG),
                authorized_keys: String::from(DEFAULT_AUTHORIZED_KEYS),
            },
        };

        let process = |file: &Path| -> Result<()> {
            println!("Reading control {}", file.display());

            let content = std::fs::read_to_string(file)?;
            let content = toml::from_str::<toml::Table>(&content)?;

            for (user, data) in content {
                let data: IncompleteControl = data.try_into()?;

                Self::validate(&data)?;

                if user == "*" {
                    result.fallback.fill_from(&data);
                    continue;
                }

                let uid = if let Ok(uid) = user.parse::<uid_t>() {
                    uid
                } else {
                    ws.users()
                        .user_by_username(&user)?
                        .ok_or(anyhow!("unknown user"))?
                        .uid()
                };

                result
                    .users
                    .entry(uid)
                    .and_modify(|ic| ic.fill_from(&data))
                    .or_insert(data);
            }

            Ok(())
        };

        visit_config_files(from, 0, process, ws)
            .context("could not load control configuration files")?;

        dbg!(&result);

        Ok(result)
    }

    /// Validates additional constraints on [`IncompleteControl`] fields in
    /// control files.
    fn validate(data: &IncompleteControl) -> Result<()> {
        fn validate_file_path(
            path: &Option<String>,
            name: &str,
        ) -> Result<()> {
            if let Some(path) = path {
                match path.chars().next() {
                    None => bail!("{name:?} fields in control files must not be empty"),
                    Some('/') | Some('~') => {},
                    _ => bail!("{name:?} fields in control files must begin with '/' or '~'"),
                }
            }
            Ok(())
        }

        validate_file_path(&data.config, "config")?;
        validate_file_path(&data.authorized_keys, "authorized_keys")?;

        Ok(())
    }

    /// Returns a [`Control`] structure for given user.
    #[must_use]
    pub fn get_user_control(&self, uid: uid_t) -> Control {
        let mut result = self.fallback.clone();

        if let Some(overrides) = self.users.get(&uid) {
            result.fill_from(overrides);
        }

        result
    }
}
