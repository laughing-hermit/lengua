//! The server's private data folder: the lock-signing key, stamped test copies and scratch space.
//!
//! One folder per user, shared by every agent harness, so a lock recorded in one harness verifies
//! in all of them.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Location of the server's private data folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir(PathBuf);

impl DataDir {
    /// Uses an explicit folder (tests pass `--data-dir`).
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    /// The default folder: `$XDG_DATA_HOME/lengua`, else `$HOME/.local/share/lengua`.
    ///
    /// Returns `None` when neither variable is set to an absolute path.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        Self::from_vars(std::env::var_os("XDG_DATA_HOME"), std::env::var_os("HOME"))
    }

    fn from_vars(xdg_data_home: Option<OsString>, home: Option<OsString>) -> Option<Self> {
        let absolute =
            |value: Option<OsString>| value.map(PathBuf::from).filter(|path| path.is_absolute());
        if let Some(xdg) = absolute(xdg_data_home) {
            return Some(Self(xdg.join("lengua")));
        }
        absolute(home).map(|home| Self(home.join(".local").join("share").join("lengua")))
    }

    /// The folder's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_xdg_data_home() {
        let dir = DataDir::from_vars(Some("/xdg".into()), Some("/home/u".into()));
        assert_eq!(dir, Some(DataDir::new(PathBuf::from("/xdg/lengua"))));
    }

    #[test]
    fn falls_back_to_home() {
        let dir = DataDir::from_vars(None, Some("/home/u".into()));
        assert_eq!(
            dir,
            Some(DataDir::new(PathBuf::from("/home/u/.local/share/lengua")))
        );
    }

    #[test]
    fn ignores_relative_values() {
        let dir = DataDir::from_vars(Some("relative".into()), Some("also-relative".into()));
        assert_eq!(dir, None);
    }
}
