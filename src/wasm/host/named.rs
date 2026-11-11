//! `file_open` and `folder_open`, lengua's own: what a program calls to open a file or a folder
//! one of its arguments names, when its idea's `- Files:` lines grant it ([`crate::named`]). The
//! program gives the argument's place and never a path, so no path it builds from what it read
//! is ever opened. The order of the checks is the defence: the grant and its mode first, then
//! the place lengua resolves from the argument's own text, held away from lengua's own folders
//! and the system's, then the open itself, with every rule of a file in the working folder.

use cap_std::fs::Dir;

use super::fds::Descriptor;
use super::files::{self, Folder, How};
use super::{Fail, Host, memory};
use crate::named::{self, Place, Unplaced, mode};
use crate::wasm::wasi::errno;

/// The argument at `place`, from 1, as a report names it.
fn shown(place: u32) -> String {
    format!("the file or folder its argument {place} names")
}

/// The place the argument at `place` names, as a folder when `folder`, once a grant of the
/// idea covers it; with the grant.
fn placed(host: &Host, place: u32, folder: bool) -> Result<(named::Grant, Place), Unplaced> {
    let refused = |why: String| Unplaced {
        errno: errno::NOTCAPABLE,
        why,
        refused: true,
    };
    let index = usize::try_from(place).unwrap_or(usize::MAX);
    let grants = &host.policy.reach.get().files;
    let Some(grant) = named::granted(grants, index, folder) else {
        return Err(refused(format!(
            "its idea has no `- Files:` line for a {} its argument {place} names",
            if folder { "folder" } else { "file" }
        )));
    };
    // The program's name is argument 0, which names no file.
    let Some(text) = host.args.get(index).filter(|_| index >= 1) else {
        return Err(Unplaced {
            errno: errno::INVAL,
            why: format!("the program has no argument {place}"),
            refused: false,
        });
    };
    let found = named::resolve(&host.ground, text, folder)?;
    Ok((grant, found))
}

/// The folder a place's names lead to: its root, opened as it is, and each name beneath it
/// through cap-std, which goes down no link.
fn folder_of(place: &Place) -> Result<Dir, Fail> {
    let failed = |error: &std::io::Error| Fail::Errno(files::errno_of(error));
    let root = Dir::open_ambient_dir(&place.root, cap_std::ambient_authority())
        .map_err(|error| failed(&error))?;
    if place.within.is_empty() {
        return Ok(root);
    }
    root.open_dir(place.within.join("/"))
        .map_err(|error| failed(&error))
}

/// What a call comes to: `opened`'s answer, noted for the run's report.
fn answer(
    host: &mut Host,
    place: u32,
    opened: Result<(String, Result<Descriptor, Fail>), Unplaced>,
) -> Result<u32, Fail> {
    match opened {
        Err(unplaced) => {
            host.note(shown(place), Some(unplaced.why), unplaced.refused);
            Err(Fail::Errno(unplaced.errno))
        }
        Ok((path, Err(fail))) => {
            if let Fail::Errno(number) = fail {
                host.note(path, Some(format!("it didn't open: error {number}")), false);
            }
            Err(fail)
        }
        Ok((path, Ok(descriptor))) => {
            let fd = host.fds.add(descriptor).map_err(Fail::Errno)?;
            host.note(path, None, false);
            Ok(fd)
        }
    }
}

/// `file_open(argument, mode, out_fd)`: opens the file the argument at `argument` names, as
/// `mode` says and the idea's grant for that argument allows, and writes its descriptor.
///
/// # Errors
///
/// A buffer outside memory; `INVAL` for a mode that isn't one, or an argument the program
/// doesn't have; `NOTCAPABLE` for an argument or a mode the idea doesn't grant, and for a place
/// lengua's rules refuse; and what opening a file in the working folder gives.
pub fn file_open(
    host: &mut Host,
    memory: &mut [u8],
    argument: u32,
    asked: u32,
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let has = |bit: u32| asked & bit != 0;
    let how = How {
        read: has(mode::READ),
        write: has(mode::WRITE),
        append: has(mode::APPEND),
        create: has(mode::CREATE),
        new: false,
        empty: has(mode::TRUNCATE),
    };
    let changes = how.append || how.create || how.empty;
    if asked & !mode::ALL != 0 || !(how.read || how.write) || (changes && !how.write) {
        return Err(Fail::Errno(errno::INVAL));
    }
    let opened = placed(host, argument, false).and_then(|(grant, place)| {
        if (how.read && !grant.reads) || (how.write && !grant.writes) {
            return Err(Unplaced {
                errno: errno::NOTCAPABLE,
                why: format!(
                    "its idea lets it {} the file its argument {argument} names, and no more",
                    if grant.reads { "read" } else { "write" }
                ),
                refused: true,
            });
        }
        Ok(place)
    });
    let opened = opened.map(|place| {
        let path = place.shown();
        let leaf = place.leaf.clone().unwrap_or_default();
        let open = folder_of(&place)
            .and_then(|folder| files::open_named(folder, &leaf, &mut host.limits, how))
            .map(|open| Descriptor::File(Box::new(open)));
        (path, open)
    });
    let fd = answer(host, argument, opened)?;
    memory::write_u32(memory, out, fd)
}

/// `folder_open(argument, out_fd)`: opens the folder the argument at `argument` names, and
/// writes its descriptor, which the `path_` functions and `fd_readdir` take. A folder granted
/// for reading alone takes no change.
///
/// # Errors
///
/// A buffer outside memory; `INVAL` for an argument the program doesn't have; `NOTCAPABLE` for
/// an argument the idea doesn't grant as a folder, and for a place lengua's rules refuse; and
/// what the system says of the folder.
pub fn folder_open(
    host: &mut Host,
    memory: &mut [u8],
    argument: u32,
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let index = usize::try_from(argument).unwrap_or(usize::MAX);
    let reversed = host.reversed;
    let opened = placed(host, argument, true).map(|(grant, place)| {
        let path = place.shown();
        // One root for each argument, however often the program opens it.
        let known = host
            .named_arguments
            .iter()
            .position(|known| *known == index);
        let root = match known {
            Some(at) => Ok(at),
            None => folder_of(&place).map(|folder| {
                host.named
                    .push(files::named_root(folder, grant.writes, reversed));
                host.named_arguments.push(index);
                host.named.len() - 1
            }),
        };
        let descriptor = root.map(|at| {
            Descriptor::Folder(Folder {
                root: u32::try_from(at + 1).unwrap_or(files::OUTSIDE),
                path: Vec::new(),
                listing: None,
            })
        });
        (path, descriptor)
    });
    let fd = answer(host, argument, opened)?;
    memory::write_u32(memory, out, fd)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::named::Ground;
    use crate::reach::Grants;
    use crate::wasm::host::policy::{Policy, Reach};
    use crate::wasm::host::tests::world;
    use crate::wasm::wasi::{oflags, rights};

    /// A host whose idea's `- Files:` lines say `lines`, given `args`, with its named paths
    /// resolved on `ground`.
    fn host_with(lines: &[&str], args: &[&str], ground: Ground) -> Host {
        let mut grants = Grants::default();
        for line in lines {
            let grant = named::Grant::read(line).unwrap_or_else(|why| panic!("{line}: {why}"));
            named::add(&mut grants.files, grant).expect("room for it");
        }
        let policy = Policy {
            reach: Reach::of(&grants),
            ..Policy::default()
        };
        let mut world = world(policy);
        world.args = args.iter().map(|arg| (*arg).to_owned()).collect();
        world.beyond.ground = Some(ground);
        Host::new(world)
    }

    /// A run's ground: paths count from `from`, and `own` are lengua's own folders.
    fn anywhere(from: &Path, own: &[&Path]) -> Ground {
        Ground::Anywhere {
            from: from.canonicalize().expect("it is there"),
            own: own.iter().map(|path| path.to_path_buf()).collect(),
        }
    }

    /// What `file_open` answers: the descriptor, or the error number.
    fn opened(host: &mut Host, argument: u32, asked: u32) -> Result<u32, u16> {
        let mut memory = vec![0_u8; 65_536];
        match file_open(host, &mut memory, argument, asked, 8) {
            Ok(()) => Ok(memory::read_u32(&memory, 8).expect("the descriptor")),
            Err(Fail::Errno(number)) => Err(number),
            Err(Fail::Stop(stop)) => panic!("{stop}"),
        }
    }

    /// What `folder_open` answers: the descriptor, or the error number.
    fn folder_opened(host: &mut Host, argument: u32) -> Result<u32, u16> {
        let mut memory = vec![0_u8; 65_536];
        match folder_open(host, &mut memory, argument, 8) {
            Ok(()) => Ok(memory::read_u32(&memory, 8).expect("the descriptor")),
            Err(Fail::Errno(number)) => Err(number),
            Err(Fail::Stop(stop)) => panic!("{stop}"),
        }
    }

    /// What `path_open` answers for `name` beneath the folder `fd`, to write when `write`.
    fn path_opened(host: &mut Host, fd: u32, name: &str, write: bool) -> Result<u32, u16> {
        let mut memory = vec![0_u8; 65_536];
        memory[100..100 + name.len()].copy_from_slice(name.as_bytes());
        let opening = files::Opening {
            oflags: if write { oflags::CREAT } else { 0 },
            rights: if write {
                rights::FD_WRITE
            } else {
                rights::FD_READ
            },
            fdflags: 0,
        };
        let length = u32::try_from(name.len()).expect("a short name");
        match files::path_open(host, &mut memory, fd, (100, length), opening, 8) {
            Ok(()) => Ok(memory::read_u32(&memory, 8).expect("the descriptor")),
            Err(Fail::Errno(number)) => Err(number),
            Err(Fail::Stop(stop)) => panic!("{stop}"),
        }
    }

    /// What a read of the whole of the file `fd` gives.
    fn contents(host: &mut Host, fd: u32) -> Vec<u8> {
        let Some(Descriptor::File(open)) = host.fds.get_mut(fd) else {
            panic!("{fd} is no file");
        };
        let mut buffer = vec![0_u8; 4096];
        let count = files::read(open, &mut buffer).unwrap_or_else(|_| panic!("a read"));
        buffer.truncate(count);
        buffer
    }

    /// A folder of the user's with a file `in.txt` that holds `hello`.
    fn notes() -> tempfile::TempDir {
        let folder = tempfile::tempdir().expect("a folder");
        std::fs::write(folder.path().join("in.txt"), "hello").expect("a file");
        folder
    }

    /// The threat: a program opens a file its idea doesn't grant. It gives an argument's place
    /// and never a path, and only the arguments the idea names are opened.
    #[test]
    fn only_the_arguments_the_idea_names_are_opened() {
        let folder = notes();
        std::fs::write(folder.path().join("other.txt"), "private").expect("a file");
        let mut host = host_with(
            &["reads the file its first argument names"],
            &["in.txt", "other.txt"],
            anywhere(folder.path(), &[]),
        );
        let fd = opened(&mut host, 1, mode::READ).expect("the first argument's file");
        assert_eq!(contents(&mut host, fd), b"hello");
        // The second argument, the program's own name, and an argument it doesn't have.
        assert_eq!(opened(&mut host, 2, mode::READ), Err(errno::NOTCAPABLE));
        assert_eq!(opened(&mut host, 0, mode::READ), Err(errno::NOTCAPABLE));
        assert_eq!(opened(&mut host, 3, mode::READ), Err(errno::NOTCAPABLE));
        // Nor the first argument as a folder.
        assert_eq!(folder_opened(&mut host, 1), Err(errno::NOTCAPABLE));
        // The run's report has what it opened and what lengua refused.
        let noted = &host.notes.opened;
        assert!(noted[0].problem.is_none() && noted[0].place.ends_with("in.txt"));
        assert!(noted[1].problem.as_deref().is_some_and(|why| why.contains("no `- Files:` line")));
        // Under a grant of every argument, each is opened, and one that isn't there is INVAL.
        let mut every = host_with(
            &["reads the files its arguments name"],
            &["in.txt", "other.txt", "missing.txt"],
            anywhere(folder.path(), &[]),
        );
        let second = opened(&mut every, 2, mode::READ).expect("the second argument's file");
        assert_eq!(contents(&mut every, second), b"private");
        assert_eq!(opened(&mut every, 3, mode::READ), Err(errno::NOENT));
        assert_eq!(opened(&mut every, 4, mode::READ), Err(errno::INVAL));
        assert_eq!(opened(&mut every, 0, mode::READ), Err(errno::NOTCAPABLE));
        // A host with no grant at all.
        let mut none = host_with(&[], &["in.txt"], anywhere(folder.path(), &[]));
        assert_eq!(opened(&mut none, 1, mode::READ), Err(errno::NOTCAPABLE));
    }

    /// The threat: a program writes where its idea says it reads. The mode is held to the
    /// grant before anything is opened.
    #[test]
    fn a_mode_the_grant_does_not_say_is_refused() {
        let folder = notes();
        let read = "reads the file its first argument names";
        let write = "writes the file its second argument names";
        let mut host = host_with(
            &[read, write],
            &["in.txt", "out.txt"],
            anywhere(folder.path(), &[]),
        );
        for asked in [
            mode::WRITE,
            mode::READ | mode::WRITE,
            mode::WRITE | mode::TRUNCATE,
            mode::WRITE | mode::CREATE,
            mode::WRITE | mode::APPEND,
        ] {
            assert_eq!(opened(&mut host, 1, asked), Err(errno::NOTCAPABLE), "{asked}");
        }
        assert_eq!(
            std::fs::read(folder.path().join("in.txt")).expect("the file"),
            b"hello"
        );
        // The second argument is written, and made, and not read.
        assert_eq!(opened(&mut host, 2, mode::READ), Err(errno::NOTCAPABLE));
        assert_eq!(opened(&mut host, 2, mode::WRITE), Err(errno::NOENT));
        let out = opened(&mut host, 2, mode::WRITE | mode::CREATE).expect("a new file");
        let Some(Descriptor::File(open)) = host.fds.get_mut(out) else {
            panic!("no file");
        };
        let wrote = files::write(open, &mut host.limits, b"copied");
        assert!(matches!(wrote, Ok(6)));
        assert_eq!(
            std::fs::read(folder.path().join("out.txt")).expect("the file"),
            b"copied"
        );
        // A mode that isn't one.
        for asked in [0, mode::CREATE, mode::READ | mode::TRUNCATE, 32, u32::MAX] {
            assert_eq!(opened(&mut host, 1, asked), Err(errno::INVAL), "{asked}");
        }
        assert_eq!(
            opened(&mut host, 2, mode::WRITE | mode::TRUNCATE | mode::APPEND),
            Err(errno::INVAL)
        );
    }

    /// The threat: a folder granted for reading is changed. Such a folder takes no file to
    /// write, no new file or folder, no removal and no new name.
    #[test]
    fn a_folder_granted_for_reading_takes_no_change() {
        let folder = notes();
        std::fs::create_dir(folder.path().join("sub")).expect("a folder");
        let mut host = host_with(
            &["reads the folder its first argument names"],
            &["."],
            anywhere(folder.path(), &[]),
        );
        let fd = folder_opened(&mut host, 1).expect("the folder");
        let file = path_opened(&mut host, fd, "in.txt", false).expect("a file in it");
        assert_eq!(contents(&mut host, file), b"hello");
        assert_eq!(path_opened(&mut host, fd, "in.txt", true), Err(errno::NOTCAPABLE));
        assert_eq!(path_opened(&mut host, fd, "new.txt", true), Err(errno::NOTCAPABLE));
        assert_eq!(path_opened(&mut host, fd, "../x", false), Err(errno::NOTCAPABLE));
        let mut memory = vec![0_u8; 65_536];
        memory[100..106].copy_from_slice(b"in.txt");
        memory[200..203].copy_from_slice(b"sub");
        memory[300..305].copy_from_slice(b"b.txt");
        let refused = Err(Fail::Errno(errno::NOTCAPABLE));
        assert_eq!(files::path_unlink_file(&mut host, &memory, fd, (100, 6)), refused);
        assert_eq!(files::path_remove_directory(&mut host, &memory, fd, (200, 3)), refused);
        assert_eq!(files::path_create_directory(&mut host, &memory, fd, (300, 5)), refused);
        assert_eq!(
            files::path_rename(&mut host, &memory, (fd, 100, 6), (fd, 300, 5)),
            refused
        );
        assert!(folder.path().join("in.txt").is_file() && folder.path().join("sub").is_dir());
        assert!(!folder.path().join("b.txt").exists() && !folder.path().join("new.txt").exists());
        // The same folder, granted for both, takes them; and the same argument opened again is
        // the same root.
        let mut both = host_with(
            &["reads and writes the folder its first argument names"],
            &["."],
            anywhere(folder.path(), &[]),
        );
        let fd = folder_opened(&mut both, 1).expect("the folder");
        let again = folder_opened(&mut both, 1).expect("the folder again");
        assert_ne!(fd, again);
        assert_eq!(both.named.len(), 1);
        assert!(path_opened(&mut both, again, "new.txt", true).is_ok());
        assert!(folder.path().join("new.txt").is_file());
        assert_eq!(
            files::path_rename(&mut both, &memory, (fd, 100, 6), (again, 300, 5)),
            Ok(())
        );
        assert!(folder.path().join("b.txt").is_file());
    }

    /// The threat: the user is led to name lengua's own folder, or the system's. A run opens
    /// no file there, and no folder that is one, holds one or lies in one.
    #[test]
    fn lenguas_own_folders_are_never_opened() {
        let home = tempfile::tempdir().expect("a folder");
        let data = home.path().join("data");
        let notes = home.path().join("notes");
        std::fs::create_dir_all(&data).expect("lengua's folder");
        std::fs::create_dir_all(&notes).expect("the user's folder");
        std::fs::write(data.join("key"), "the key").expect("the key");
        let key = data.join("key").to_string_lossy().into_owned();
        let home_text = home.path().to_string_lossy().into_owned();
        let data_text = data.to_string_lossy().into_owned();
        let own = data.canonicalize().expect("it is there");
        let mut host = host_with(
            &[
                "reads and writes the file its first argument names",
                "reads the folder its second argument names",
                "reads the folder its third argument names",
            ],
            &[&key, &home_text, &data_text],
            anywhere(&notes, &[own.as_path()]),
        );
        assert_eq!(opened(&mut host, 1, mode::READ), Err(errno::NOTCAPABLE));
        assert_eq!(
            opened(&mut host, 1, mode::WRITE | mode::TRUNCATE),
            Err(errno::NOTCAPABLE)
        );
        assert_eq!(folder_opened(&mut host, 2), Err(errno::NOTCAPABLE));
        assert_eq!(folder_opened(&mut host, 3), Err(errno::NOTCAPABLE));
        assert_eq!(std::fs::read(data.join("key")).expect("the key"), b"the key");
        assert!(
            host.notes
                .opened
                .iter()
                .all(|noted| noted.problem.as_deref().is_some_and(|why| why.contains("lengua's own")))
        );
    }

    /// The threat: a handed folder's file is a link to a private file. The last name is never
    /// a link, and a file with a second name isn't opened, before it is emptied.
    #[test]
    fn a_link_at_the_last_name_is_not_followed_and_two_names_are_not_opened() {
        let folder = notes();
        let private = tempfile::tempdir().expect("a folder");
        let secret = private.path().join("secret.txt");
        std::fs::write(&secret, "private").expect("a file");
        let grant = "reads and writes the files its arguments name";
        let ground = anywhere(folder.path(), &[]);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&secret, folder.path().join("link.txt")).expect("a link");
            let mut host = host_with(&[grant], &["link.txt"], ground.clone());
            assert_eq!(opened(&mut host, 1, mode::READ), Err(errno::ACCES));
            assert_eq!(
                opened(&mut host, 1, mode::WRITE | mode::TRUNCATE),
                Err(errno::ACCES)
            );
        }
        if std::fs::hard_link(&secret, folder.path().join("twin.txt")).is_ok() {
            let mut host = host_with(&[grant], &["twin.txt"], ground.clone());
            assert_eq!(opened(&mut host, 1, mode::READ), Err(errno::ACCES));
            assert_eq!(
                opened(&mut host, 1, mode::WRITE | mode::TRUNCATE),
                Err(errno::ACCES)
            );
        }
        assert_eq!(std::fs::read(&secret).expect("the file"), b"private");
        // A folder where a file is asked for, and a name that differs only in case.
        std::fs::create_dir(folder.path().join("sub")).expect("a folder");
        let mut host = host_with(&[grant], &["sub", "IN.TXT", "in.txt"], ground);
        assert_eq!(opened(&mut host, 1, mode::READ), Err(errno::ISDIR));
        assert_eq!(opened(&mut host, 2, mode::READ), Err(errno::NOENT));
        assert_eq!(
            opened(&mut host, 2, mode::WRITE | mode::CREATE),
            Err(errno::EXIST)
        );
        assert!(opened(&mut host, 3, mode::READ).is_ok());
    }

    /// The threat: a test's argument names a file of the user's. In a test, a named path stays
    /// beneath the test's own folder.
    #[test]
    fn a_tests_paths_stay_in_the_tests_folder() {
        let folder = notes();
        std::fs::create_dir(folder.path().join("sub")).expect("a folder");
        std::fs::write(folder.path().join("sub").join("deep.txt"), "deep").expect("a file");
        let mut host = host_with(
            &["reads and writes the files its arguments name"],
            &["in.txt", "sub/deep.txt", "/etc/passwd", "../in.txt", "sub/../../x"],
            Ground::Beneath(folder.path().to_path_buf()),
        );
        let fd = opened(&mut host, 1, mode::READ).expect("a file of the test's");
        assert_eq!(contents(&mut host, fd), b"hello");
        let deep = opened(&mut host, 2, mode::READ).expect("a file beneath");
        assert_eq!(contents(&mut host, deep), b"deep");
        for outside in [3, 4, 5] {
            assert_eq!(opened(&mut host, outside, mode::READ), Err(errno::NOTCAPABLE));
            assert_eq!(
                opened(&mut host, outside, mode::WRITE | mode::CREATE),
                Err(errno::NOTCAPABLE)
            );
        }
        // A run with no ground opens nothing.
        let mut nowhere = host_with(
            &["reads the files its arguments name"],
            &["in.txt"],
            Ground::Nowhere,
        );
        assert_eq!(opened(&mut nowhere, 1, mode::READ), Err(errno::NOTCAPABLE));
    }

    /// The threat: a program fills the disk through a file the user named. A named file is
    /// held to the run's file limits as a file in the working folder is.
    #[test]
    fn a_named_file_is_held_to_the_runs_file_limits() {
        let folder = notes();
        let mut host = host_with(
            &["writes the file its first argument names"],
            &["out.bin"],
            anywhere(folder.path(), &[]),
        );
        // The test's world lets a file hold 1 MiB.
        let out = opened(&mut host, 1, mode::WRITE | mode::CREATE).expect("a new file");
        let piece = vec![7_u8; 1 << 19];
        let mut total = 0;
        let mut last = Ok(0);
        for _ in 0..4 {
            let Some(Descriptor::File(open)) = host.fds.get_mut(out) else {
                panic!("no file");
            };
            last = files::write(open, &mut host.limits, &piece);
            total += last.unwrap_or(0);
        }
        assert_eq!(total, 1 << 20);
        assert_eq!(last, Err(Fail::Errno(errno::FBIG)));
        assert_eq!(
            std::fs::metadata(folder.path().join("out.bin")).expect("the file").len(),
            1 << 20
        );
    }
}
