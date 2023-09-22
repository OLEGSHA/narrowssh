use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

use anyhow::{bail, Result};
use uzers::{uid_t, User};

#[cfg(test)]
pub mod mock;

/// Provides access to a snapshot of system users.
pub struct UserMap {
    data: HashMap<uid_t, User>,
    current_uid: uid_t,
}

impl UserMap {
    /// An iterator over all known users in the system.
    #[must_use]
    pub fn all_users(
        &self,
    ) -> std::collections::hash_map::Values<'_, uid_t, User> {
        self.data.values()
    }

    /// Returns the [`User`] with given UID if one exists.
    #[must_use]
    pub fn user_by_uid(&self, uid: uid_t) -> Option<&User> {
        self.data.get(&uid)
    }

    /// Returns the [`User`] with given username if exactly one exists.
    ///
    /// If no users are found, returns `Ok(None)`. If exactly one user `u` has
    /// given username, returns `Ok(Some(u))`. If at least two users share the
    /// username, returns `Err`.
    ///
    /// # Errors
    /// An error is returned if multiple users share the provided username.
    pub fn user_by_username<S: AsRef<OsStr>>(
        &self,
        name: S,
    ) -> Result<Option<&User>> {
        let mut iter = self.data.values();
        let name = name.as_ref();

        let first = iter.find(|&u| u.name() == name);
        if let Some(result) = first {
            if iter.any(|u| u.name() == name) {
                bail!("Username is not unique");
            }
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    /// Returns the current UID of the process.
    #[must_use]
    pub fn current_uid(&self) -> uid_t {
        self.current_uid
    }

    /// Add a [`User`] manually. For use in testing.
    pub fn add(&mut self, user: User) {
        self.data.insert(user.uid(), user);
    }

    /// Constructs a new `UserMap` from [`User`] values.
    pub fn new<I: Iterator<Item = User>>(
        users: I,
        current_uid: uid_t,
    ) -> Self {
        Self {
            data: users.map(|u| (u.uid(), u)).collect(),
            current_uid,
        }
    }
}

/// Helper that holds various universally desired data.
pub trait Workspace {
    /// Returns a system user manager.
    fn users(&self) -> &UserMap;

    /// Returns the mock owner UID of given filesystem object.
    ///
    /// This method is useful for testing purposes and should always return
    /// `None` in release builds.
    fn get_mock_owner_uid<P: AsRef<Path>>(&self, path: P) -> Option<uid_t>;
}

#[allow(clippy::module_name_repetitions)] // Makes little sense otherwise
/// The Workspace implementation used in release builds.
pub struct RealWorkspace {
    user_map: UserMap,
}

impl RealWorkspace {
    /// Constructs a [`RealWorkspace`].
    ///
    /// # Safety
    /// Calls [`all_users()`][uzers::all_users()].
    #[must_use]
    pub unsafe fn new() -> Self {
        Self {
            user_map: UserMap::new(
                uzers::all_users(),
                uzers::get_current_uid(),
            ),
        }
    }
}

impl Workspace for RealWorkspace {
    fn users(&self) -> &UserMap {
        &self.user_map
    }

    fn get_mock_owner_uid<P: AsRef<Path>>(&self, _: P) -> Option<uid_t> {
        None
    }
}
