//! The names every system takes for files and folders. lengua holds a program on every target
//! to what the strictest of them allows, so a spec that works on one works on all of them:
//! Windows refuses some characters, device names and endings in a name, takes names that
//! differ only in case as one, and can open a file by a short name such as `REPORT~1.TXT`
//! that it gave a long one; macOS's APFS takes only UTF-8 names of characters Unicode 9.0
//! assigned, and takes names that differ only in case or in Unicode normalization as one; and
//! each caps a name's and a path's length. A program that asks for a name some system refuses
//! gets an error on every system, the same one.

mod unicode9;

/// The longest name a folder's entry may have, in UTF-8 bytes: every system's `NAME_MAX`.
pub const MAX_NAME: usize = 255;

/// The longest path a program may give, in bytes: macOS's `PATH_MAX` of 1,024, less its NUL.
pub const MAX_PATH: usize = 1023;

/// `EINVAL`: a name some system refuses outright.
pub const EINVAL: i64 = 22;
/// `ENAMETOOLONG`: a name or path past every system's length.
pub const ENAMETOOLONG: i64 = 36;
/// `EILSEQ`: a name that isn't UTF-8 text of characters Unicode 9.0 assigned.
pub const EILSEQ: i64 = 84;
/// `EEXIST`: a new name that differs only in case or normalization from one in its folder.
pub const EEXIST: i64 = 17;
/// `EBUSY`: a file renamed or removed while the program holds it open.
pub const EBUSY: i64 = 16;

/// A rule a name breaks: the error the program gets, and why, in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Broken {
    /// The error number the call returns, negated.
    pub errno: i64,
    /// Why, for a report.
    pub why: String,
}

impl Broken {
    fn new(errno: i64, why: impl Into<String>) -> Self {
        Self {
            errno,
            why: why.into(),
        }
    }
}

/// The characters Windows refuses in a name, besides the controls below 32.
const REFUSED: &[char] = &['<', '>', ':', '"', '\\', '|', '?', '*'];

/// The device names Windows reserves in every folder, alone or with any extension.
const DEVICES: &[&str] = &[
    "CON",
    "PRN",
    "AUX",
    "NUL",
    "COM1",
    "COM2",
    "COM3",
    "COM4",
    "COM5",
    "COM6",
    "COM7",
    "COM8",
    "COM9",
    "COM\u{b9}",
    "COM\u{b2}",
    "COM\u{b3}",
    "LPT1",
    "LPT2",
    "LPT3",
    "LPT4",
    "LPT5",
    "LPT6",
    "LPT7",
    "LPT8",
    "LPT9",
    "LPT\u{b9}",
    "LPT\u{b2}",
    "LPT\u{b3}",
];

/// The rule `path`, as a program gives it to a call, breaks, if any: each of its names, between
/// slashes, checked, and its whole length. `.` and `..` are the folder and its parent, which
/// lengua's confinement handles.
#[must_use]
pub fn broken(path: &[u8]) -> Option<Broken> {
    if path.len() > MAX_PATH {
        return Some(Broken::new(
            ENAMETOOLONG,
            format!("a path of {} bytes, past macOS's {MAX_PATH}", path.len()),
        ));
    }
    path.split(|byte| *byte == b'/')
        .filter(|name| !name.is_empty() && *name != b"." && *name != b"..")
        .find_map(broken_name)
}

/// The rule one name breaks, if any.
fn broken_name(name: &[u8]) -> Option<Broken> {
    let Ok(text) = std::str::from_utf8(name) else {
        return Some(Broken::new(
            EILSEQ,
            "a name that isn't UTF-8 text, which macOS and Windows refuse",
        ));
    };
    if name.len() > MAX_NAME {
        return Some(Broken::new(
            ENAMETOOLONG,
            format!(
                "a name of {} bytes, past every system's {MAX_NAME}",
                name.len()
            ),
        ));
    }
    if let Some(character) = text.chars().find(|character| !assigned(*character)) {
        return Some(Broken::new(
            EILSEQ,
            format!(
                "`{}` holds U+{:04X}, which Unicode 9.0 hadn't assigned, and macOS refuses it in \
                 a name",
                text.escape_debug(),
                u32::from(character)
            ),
        ));
    }
    if let Some(character) = text
        .chars()
        .find(|character| REFUSED.contains(character) || u32::from(*character) < 32)
    {
        return Some(Broken::new(
            EINVAL,
            format!(
                "`{}` holds {}, which Windows refuses in a name",
                text.escape_debug(),
                if u32::from(character) < 32 {
                    format!("the control character U+{:04X}", u32::from(character))
                } else {
                    format!("`{character}`")
                }
            ),
        ));
    }
    if text.ends_with(['.', ' ']) {
        return Some(Broken::new(
            EINVAL,
            format!(
                "`{}` ends with a dot or a space, which Windows refuses",
                text.escape_debug()
            ),
        ));
    }
    let stem = text.split('.').next().unwrap_or(text);
    if let Some(device) = DEVICES
        .iter()
        .find(|device| stem.to_uppercase() == **device)
    {
        return Some(Broken::new(
            EINVAL,
            format!("`{text}`: Windows reserves {device}, with any extension, in every folder"),
        ));
    }
    if short_name_shape(text) {
        return Some(Broken::new(
            EINVAL,
            format!(
                "`{text}` has the shape of a Windows short (8.3) name, which there can name \
                 another file, one Windows gave that short name too"
            ),
        ));
    }
    None
}

/// The longest stem and extension of a Windows short (8.3) name, in characters.
pub const SHORT_STEM: usize = 8;
/// See [`SHORT_STEM`].
pub const SHORT_EXTENSION: usize = 3;

/// Whether `name` has the shape of a Windows short name, such as `REPORT~1.TXT` or `progra~1`:
/// at most eight characters ending in `~` and digits, then, if any, one dot and one to three
/// more. Where short names are on, Windows gives each long name one of that shape, and opens the
/// long-named file by it, so such a name there may reach another file: refused on every system.
#[must_use]
pub fn short_name_shape(name: &str) -> bool {
    let (stem, extension) = match name.split_once('.') {
        Some((stem, extension)) => (stem, Some(extension)),
        None => (name, None),
    };
    if extension.is_some_and(|extension| {
        extension.is_empty()
            || extension.contains('.')
            || extension.chars().count() > SHORT_EXTENSION
    }) {
        return false;
    }
    stem.chars().count() <= SHORT_STEM
        && stem.rsplit_once('~').is_some_and(|(_, digits)| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
}

/// Whether macOS's APFS takes `character` in a name: Unicode 9.0 assigned it.
fn assigned(character: char) -> bool {
    let point = u32::from(character);
    unicode9::ASSIGNED
        .binary_search_by(|(first, last)| {
            if point < *first {
                std::cmp::Ordering::Greater
            } else if point > *last {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// The form two names share when some system takes them as one: Unicode's decomposed
/// normalization, then each character's case mapped up and back down, then decomposed again,
/// so `Readme.TXT` and `README.txt`, or `é` as one character and as `e` and an accent, meet.
#[must_use]
pub fn folded(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    let decomposed: String = name.nfd().collect();
    decomposed
        .to_uppercase()
        .to_lowercase()
        .nfd()
        .collect::<String>()
}

/// Whether `name` would be a second name for `other` on a system that takes names differing
/// only in case or normalization as one: the same folded, but not the same bytes.
#[must_use]
pub fn clashes(name: &str, other: &str) -> bool {
    if name == other {
        return false;
    }
    // Two ASCII names meet when they differ only in case, which is all folding does to them; the
    // broker asks this of every entry of a folder for every name a program makes.
    if name.is_ascii() && other.is_ascii() {
        return name.eq_ignore_ascii_case(other);
    }
    folded(name) == folded(other)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn errno(path: &str) -> Option<i64> {
        broken(path.as_bytes()).map(|broken| broken.errno)
    }

    #[test]
    fn names_every_system_takes_pass() {
        for path in [
            "notes.txt",
            "archive/2026/notes.txt",
            ".hidden",
            "a b",
            "caf\u{e9}.txt",
            "\u{65e5}\u{672c}.md",
            "CONSOLE.txt",
            "COM10",
            "./a",
            "../a",
        ] {
            assert_eq!(broken(path.as_bytes()), None, "{path}");
        }
    }

    #[test]
    fn windows_refuses_characters_endings_and_devices() {
        for path in [
            "a:b", "a<b", "a>b", "a\"b", "a\\b", "a|b", "a?b", "a*b", "a\tb",
        ] {
            assert_eq!(errno(path), Some(EINVAL), "{path}");
        }
        for path in ["a.", "a ", "folder./x"] {
            assert_eq!(errno(path), Some(EINVAL), "{path}");
        }
        for path in [
            "CON",
            "con",
            "Nul.txt",
            "aux.tar.gz",
            "COM1",
            "lpt9.log",
            "COM\u{b9}",
        ] {
            assert_eq!(errno(path), Some(EINVAL), "{path}");
        }
    }

    #[test]
    fn windows_short_names_are_refused_everywhere() {
        for path in [
            "REPORT~1.TXT",
            "progra~1",
            "a~12",
            "~1",
            "folder/DATA~3.CSV",
            "AB12~1.tx",
        ] {
            assert_eq!(errno(path), Some(EINVAL), "{path}");
        }
        for path in [
            "backup~.txt",
            "notes~old.txt",
            "longername~1.txt",
            "a~1.text",
            "a~1.b.c",
            "a1~",
            "report1.txt",
        ] {
            assert_eq!(errno(path), None, "{path}");
        }
    }

    #[test]
    fn macos_takes_only_utf8_of_unicode_9() {
        assert_eq!(broken(b"a\xffb").map(|broken| broken.errno), Some(EILSEQ));
        // U+1F970, the smiling face with hearts, came in Unicode 11.
        assert_eq!(errno("\u{1f970}.txt"), Some(EILSEQ));
        // U+1F600, the grinning face, came in Unicode 6.1.
        assert_eq!(errno("\u{1f600}.txt"), None);
        // A noncharacter is never assigned.
        assert_eq!(errno("a\u{fdd0}"), Some(EILSEQ));
        assert_eq!(errno("a\u{ffff}"), Some(EILSEQ));
    }

    #[test]
    fn names_and_paths_have_every_systems_length() {
        assert_eq!(errno(&"a".repeat(MAX_NAME)), None);
        assert_eq!(errno(&"a".repeat(MAX_NAME + 1)), Some(ENAMETOOLONG));
        let deep = ["a"; 512].join("/");
        assert_eq!(deep.len(), MAX_PATH);
        assert_eq!(errno(&deep), None);
        assert_eq!(errno(&format!("{deep}b")), Some(ENAMETOOLONG));
    }

    #[test]
    fn names_that_differ_only_in_case_or_normalization_clash() {
        assert!(clashes("Readme.TXT", "README.txt"));
        assert!(clashes("caf\u{e9}", "cafe\u{301}"));
        assert!(clashes("Stra\u{df}e", "STRASSE"));
        assert!(!clashes("a.txt", "a.txt"), "the same name is the same file");
        assert!(!clashes("a.txt", "b.txt"));
    }
}
