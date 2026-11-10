//! Files and folders as the scripts' `find`, `cp -R` and `rm -rf` handled them, the same on every
//! system.

use std::path::{Path, PathBuf};

/// Removes `path` and everything in it, as `rm -rf` does: nothing there is no error.
pub fn remove(path: &Path) -> Result<(), String> {
    let removed = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    match removed {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("{}: {error}", path.display()))
        }
        _ => Ok(()),
    }
}

/// Makes the folder `path`, with the folders above it, as `mkdir -p` does.
pub fn folder(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|error| format!("{}: {error}", path.display()))
}

/// Copies the file `from` to `to`.
pub fn copy(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| format!("{} to {}: {error}", from.display(), to.display()))
}

/// Every file at any depth of `folder`, each folder's entries in the order of their names, as
/// `find <folder> -type f | sort` lists them; links are left out, as `-type f` leaves them.
pub fn files(folder: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<(PathBuf, std::fs::FileType)> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| Some((entry.path(), entry.file_type().ok()?)))
        .collect();
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    let mut found = Vec::new();
    for (path, kind) in entries {
        if kind.is_dir() {
            found.extend(files(&path));
        } else if kind.is_file() {
            found.push(path);
        }
    }
    found
}

/// `path` below `folder`, its parts joined by `/` on every system, as an archive or a listing
/// names it.
pub fn relative(path: &Path, folder: &Path) -> String {
    let below = path.strip_prefix(folder).unwrap_or(path);
    let parts: Vec<String> = below
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.join("/")
}

/// Copies everything in the folder `from` into the folder `into`, folders and all, as
/// `cp -R <from>/. <into>/` does: a link stays a link on Unix, and is copied as what it names on
/// Windows.
pub fn copy_folder(from: &Path, into: &Path) -> Result<(), String> {
    folder(into)?;
    let list = std::fs::read_dir(from).map_err(|error| format!("{}: {error}", from.display()))?;
    for entry in list {
        let entry = entry.map_err(|error| format!("{}: {error}", from.display()))?;
        let source = entry.path();
        let target = into.join(entry.file_name());
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", source.display()))?;
        if kind.is_dir() {
            copy_folder(&source, &target)?;
        } else if kind.is_symlink() && cfg!(unix) {
            link(&source, &target)?;
        } else {
            copy(&source, &target)?;
        }
    }
    Ok(())
}

/// Makes `target` a link to what the link `source` names.
#[cfg(unix)]
fn link(source: &Path, target: &Path) -> Result<(), String> {
    let named =
        std::fs::read_link(source).map_err(|error| format!("{}: {error}", source.display()))?;
    remove(target)?;
    std::os::unix::fs::symlink(named, target)
        .map_err(|error| format!("{}: {error}", target.display()))
}

/// Windows copies what a link names instead.
#[cfg(not(unix))]
fn link(source: &Path, target: &Path) -> Result<(), String> {
    copy(source, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder's files are listed at every depth, in the order of their names, and copied with
    /// their folders.
    #[test]
    fn a_folder_s_files_are_listed_in_order_and_copied() {
        let root = tempfile::tempdir().expect("a folder");
        let from = root.path().join("from");
        folder(&from.join("b").join("c")).expect("folders");
        for name in ["z.txt", "a.txt", "b/c/d.txt", "b/a.txt"] {
            std::fs::write(from.join(name), name).expect("a file");
        }
        let b = from.join("b");
        assert_eq!(
            files(&from),
            [
                from.join("a.txt"),
                b.join("a.txt"),
                b.join("c").join("d.txt"),
                from.join("z.txt"),
            ]
        );
        let into = root.path().join("into");
        copy_folder(&from, &into).expect("copied");
        assert_eq!(
            std::fs::read_to_string(into.join("b").join("c").join("d.txt")).expect("copied"),
            "b/c/d.txt"
        );
        remove(&into).expect("removed");
        assert!(!into.exists());
        remove(&into).expect("nothing to remove is no error");
    }
}
