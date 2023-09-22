//! Mock implementation of [`Workspace`].

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use assert_fs::{fixture::ChildPath, prelude::*, TempDir};
use uzers::os::unix::UserExt;
use uzers::{uid_t, User};

use crate::workspace::{UserMap, Workspace};

/// Mock implementation of [`Workspace`].
///
/// Ownership of paths is inherited to all descendants of a directory.
/// All paths encountered in a test must be owned.
pub struct MockWorkspace {
    user_map: UserMap,
    owned_paths: HashMap<PathBuf, uid_t>,
    temp_dir: TempDir,
}

impl MockWorkspace {
    /// Returns a [`ChildPath`] located in the [`TempDir`].
    pub fn child<P: AsRef<Path>>(&self, path: P) -> ChildPath {
        self.temp_dir.child(path.as_ref())
    }

    /// Returns a [`PathBuf`] located in the [`TempDir`].
    pub fn path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.temp_dir.join(path)
    }

    /// A generic method for manipulating FS objects.
    ///
    /// A [`ChildPath`] is created at the [`TempDir`] and passed as an argument
    /// to the provided `action`.
    ///
    /// [`get_mock_owner_uid`] will later report that the path is owned by
    /// `owner`.
    pub fn add_path_and<P, F>(
        &mut self,
        path: P,
        owner: uid_t,
        action: F,
    ) -> Result<PathBuf>
    where
        P: AsRef<Path>,
        F: FnOnce(&assert_fs::fixture::ChildPath) -> Result<()>,
    {
        let child = self.child(path);
        let path = child.path().to_path_buf();
        self.owned_paths.entry(path.clone()).or_insert(owner);

        action(&child)?;

        Ok(path)
    }

    /// Creates a new file and writes `contents` into it.
    ///
    /// [`get_mock_owner_uid`] will later report that the path is owned by
    /// `owner`. The file will have Unix permissions set to `mode`.
    ///
    /// Any missing directories will be created, but these directories will
    /// not have any ownership information and will retain mode as set by
    /// the OS.
    pub fn add_file<P, S>(
        &mut self,
        path: P,
        owner: uid_t,
        mode: u32,
        contents: S,
    ) -> Result<PathBuf>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        self.add_path_and(path, owner, |c| {
            c.write_str(contents.as_ref())?;
            set_perms(c.path(), mode)?;
            Ok(())
        })
    }

    /// Creates a new directory.
    ///
    /// [`get_mock_owner_uid`] will later report that the path is owned by
    /// `owner`. The directory will have Unix permissions set to `mode`.
    ///
    /// Any missing parents will be created, but these directories will
    /// not have any ownership information and will retain mode as set by
    /// the OS.
    pub fn add_dir<P>(
        &mut self,
        path: P,
        owner: uid_t,
        mode: u32,
    ) -> Result<PathBuf>
    where
        P: AsRef<Path>,
    {
        self.add_path_and(path, owner, |c| {
            c.create_dir_all()?;
            set_perms(c.path(), mode)?;
            Ok(())
        })
    }

    /// Creates a new symbolic link from `path` to `link_to`.
    ///
    /// Both paths are interpreted as relative to the [`TempDir`].
    ///
    /// Any missing directories will be created, but these directories will
    /// not have any ownership information and will retain mode as set by
    /// the OS.
    pub fn add_symlink<P, Q>(&self, path: P, link_to: Q) -> Result<PathBuf>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let path = self.path(path);
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::os::unix::fs::symlink(self.path(link_to), &path)?;
        Ok(path)
    }

    /// Adds a mock system user and creates the home directory.
    ///
    /// The home directory will be owned by the newly-created user according
    /// to [`get_mock_owner_uid`]. The mode will be retained by the OS.
    pub fn add_user<P, S>(
        &mut self,
        uid: uid_t,
        name: S,
        home: P,
    ) -> Result<()>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        let home = self
            .add_path_and(home.as_ref(), uid, |c| Ok(c.create_dir_all()?))?;

        let user = User::new(uid, name.as_ref(), uid).with_home_dir(&home);
        self.user_map.add(user);

        Ok(())
    }

    /// Constructs a [`MockWorkspace`].
    ///
    /// [`Self::users`] is initialized empty with current UID set to 1000.
    pub fn new() -> Result<Self> {
        Ok(Self {
            temp_dir: TempDir::new()?,
            user_map: UserMap::new(std::iter::empty(), 1000),
            owned_paths: HashMap::new(),
        })
    }
}

/// Changes the permissions of the FS object to `mode`.
fn set_perms<P>(path: P, mode: u32) -> Result<()>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();

    let mut permissions = path.metadata()?.permissions();
    permissions.set_mode(mode);
    std::fs::set_permissions(path, permissions)?;

    Ok(())
}

impl Workspace for MockWorkspace {
    fn users(&self) -> &UserMap {
        &self.user_map
    }

    fn get_mock_owner_uid<P: AsRef<Path>>(&self, path: P) -> Option<uid_t> {
        let path = path.as_ref();

        // Find most specific parent that is owned or die trying
        Some(
            *path
                .canonicalize()
                .with_context(|| format!("Could not canonicalize {:?}", path))
                .unwrap()
                .ancestors()
                .find_map(|p| self.owned_paths.get(p))
                .with_context(|| format!("{:?} is not owned", path))
                .unwrap(),
        )
    }
}
