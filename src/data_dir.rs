//! The server's private data folder: the lock-signing key, stamped test copies and scratch space.
//!
//! One folder per user, shared by every agent harness, so a lock recorded in one harness verifies
//! in all of them. It holds:
//! - `key`, the secret that signs every lock ([`crate::lock::Key`]);
//! - `stamped/<sha256>`, each idea.md check recorded, by lengua's hash of it, which later
//!   changes are shown against ([`crate::state::stamped_copy`]);
//! - `tested/<builder>/<sha256>.wasm`, each module whose tests passed, by the build that made it
//!   and the module's SHA-256, which a run from a shell runs once its bytes hash to the module
//!   hash its lock records ([`crate::state::tested_module`]);
//! - `wrong/<idea's hash>-<module's hash>`, what a proof found wrong about a module of an idea,
//!   which keeps `run` from running it whatever lock its folder holds
//!   ([`crate::state::wrong_record`]);
//! - `cache/`, the engine's compiled modules, unless the agent's harness gives the plugin a data
//!   folder of its own ([`crate::wasm::engine::cache_folder`]);
//! - `graph/<sha256>.lock`, which keeps lengua's processes from writing one bundle's index.md and
//!   log.md at once ([`DataDir::graph_lock`]).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// What to tell the user when no data folder can be found.
#[cfg(windows)]
pub const NOT_FOUND: &str = "can't find a data folder: set LOCALAPPDATA, or pass --data-dir";

/// What to tell the user when no data folder can be found.
#[cfg(target_os = "macos")]
pub const NOT_FOUND: &str = "can't find a data folder: set HOME, or pass --data-dir";

/// What to tell the user when no data folder can be found.
#[cfg(not(any(windows, target_os = "macos")))]
pub const NOT_FOUND: &str =
    "can't find a data folder: set HOME or XDG_DATA_HOME, or pass --data-dir";

/// Location of the server's private data folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir(PathBuf);

impl DataDir {
    /// Uses an explicit folder (tests pass `--data-dir`).
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    /// The default folder, where each system keeps a program's data: on Linux
    /// `$XDG_DATA_HOME/lengua`, else `$HOME/.local/share/lengua`; on macOS
    /// `$HOME/Library/Application Support/lengua`; on Windows `%LOCALAPPDATA%\lengua`.
    ///
    /// Returns `None` when the variables it needs aren't set to an absolute path.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        Self::from_vars(
            std::env::var_os("XDG_DATA_HOME"),
            std::env::var_os("HOME"),
            std::env::var_os("LOCALAPPDATA"),
        )
    }

    fn from_vars(
        xdg_data_home: Option<OsString>,
        home: Option<OsString>,
        local_app_data: Option<OsString>,
    ) -> Option<Self> {
        let absolute =
            |value: Option<OsString>| value.map(PathBuf::from).filter(|path| path.is_absolute());
        if cfg!(windows) {
            return absolute(local_app_data).map(|folder| Self(folder.join("lengua")));
        }
        if cfg!(target_os = "macos") {
            return absolute(home).map(|home| {
                Self(
                    home.join("Library")
                        .join("Application Support")
                        .join("lengua"),
                )
            });
        }
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

    /// The file that keeps lengua's processes from writing one bundle's index.md and log.md at
    /// once: kept here, never beside the user's files, and named by the bundle's root.
    #[must_use]
    pub fn graph_lock(&self, root: &Path) -> PathBuf {
        let name = crate::hash::Sha256Hex::of(root.to_string_lossy().as_bytes());
        self.0.join("graph").join(format!("{name}.lock"))
    }

    /// Makes the folder the user's alone, mode 0700 on Linux and macOS, where it is there with
    /// another mode: as when the engine's cache made it first, under the user's umask, or
    /// something changed it since. The server does this as it starts and a run from a shell
    /// before it reads the key, so the folder isn't open to others until the key is next made
    /// or loaded for a record. A folder that isn't there yet is left to whatever makes it, the
    /// key's first use among them, which makes it with that mode. On Windows it does nothing:
    /// the folder lies under the user's own `%LOCALAPPDATA%`, which the system keeps from other
    /// users. A mode that can't be set is left: lengua then works as it did.
    pub fn keep_private(&self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let open = std::fs::metadata(&self.0)
                .is_ok_and(|found| found.is_dir() && found.permissions().mode() & 0o777 != 0o700);
            if open {
                let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
            }
        }
        #[cfg(not(unix))]
        {
            let _ = &self.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absolute path on this system.
    fn absolute(name: &str) -> OsString {
        let root = if cfg!(windows) { "C:\\" } else { "/" };
        format!("{root}{name}").into()
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    #[test]
    fn prefers_xdg_data_home() {
        let dir = DataDir::from_vars(Some(absolute("xdg")), Some(absolute("home/u")), None);
        assert_eq!(
            dir,
            Some(DataDir::new(PathBuf::from(absolute("xdg")).join("lengua")))
        );
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    #[test]
    fn falls_back_to_home() {
        let dir = DataDir::from_vars(None, Some(absolute("home/u")), None);
        assert_eq!(
            dir,
            Some(DataDir::new(
                PathBuf::from(absolute("home/u")).join(".local/share/lengua")
            ))
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_keeps_it_in_application_support() {
        let dir = DataDir::from_vars(Some(absolute("xdg")), Some(absolute("Users/u")), None);
        assert_eq!(
            dir,
            Some(DataDir::new(
                PathBuf::from(absolute("Users/u")).join("Library/Application Support/lengua")
            ))
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_keeps_it_in_local_app_data() {
        let dir = DataDir::from_vars(None, Some(absolute("Users\\u")), Some(absolute("Local")));
        assert_eq!(
            dir,
            Some(DataDir::new(
                PathBuf::from(absolute("Local")).join("lengua")
            ))
        );
    }

    #[test]
    fn ignores_relative_values() {
        let dir = DataDir::from_vars(
            Some("relative".into()),
            Some("also-relative".into()),
            Some("and-this".into()),
        );
        assert_eq!(dir, None);
    }
}
