//! main.wat's fields as its text writes them, for what it takes from the parts its idea builds
//! on (`- Builds on:`): a function taken from a part is marked `;; from: <part>` on a line above
//! its `(func`, and is the part's own function word for word, as are the constants and the
//! functions it names ([`fields`], [`names`]; `check` holds them in `src/server/parts.rs`). And
//! lengua's own library of parts, as the compile skill ships it ([`LIBRARY`]).

/// One of main.wat's own functions, as its text writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// Its name, without the `$`; empty when it has none.
    pub name: String,
    /// The line its `(func` is on, from 1.
    pub line: usize,
    /// The part it says it comes from, when a `;; from:` line above it says so.
    pub from: Option<String>,
    /// Its text from `(func` to its closing `)`, without comments, each run of white space
    /// outside its strings one space, and none just inside a parenthesis.
    pub body: String,
}

/// The index just past the string that opens with the `"` at `at`, counting its lines.
fn past_string(bytes: &[u8], at: usize, line: &mut usize) -> usize {
    let mut at = at + 1;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'\\' => at += 2,
            b'"' => return at + 1,
            b'\n' => {
                *line += 1;
                at += 1;
            }
            _ => at += 1,
        }
    }
    bytes.len()
}

/// The index just past the block comment `(; … ;)` that opens at `at`, the ones nested in it
/// too, counting its lines.
fn past_block_comment(bytes: &[u8], at: usize, line: &mut usize) -> usize {
    let mut at = at;
    let mut depth = 0_usize;
    while let Some(&byte) = bytes.get(at) {
        let next = bytes.get(at + 1).copied();
        if byte == b'(' && next == Some(b';') {
            depth += 1;
            at += 2;
        } else if byte == b';' && next == Some(b')') {
            depth = depth.saturating_sub(1);
            at += 2;
            if depth == 0 {
                return at;
            }
        } else {
            if byte == b'\n' {
                *line += 1;
            }
            at += 1;
        }
    }
    bytes.len()
}

/// The index of the end of the line comment `;;` at `at`: its line's newline, which isn't the
/// comment's, or the text's end.
fn past_line_comment(bytes: &[u8], at: usize) -> usize {
    bytes[at..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(bytes.len(), |end| at + end)
}

/// Whether the bytes at `at` open a line comment, `;;`.
fn line_comment(bytes: &[u8], at: usize) -> bool {
    bytes.get(at) == Some(&b';') && bytes.get(at + 1) == Some(&b';')
}

/// Whether the bytes at `at` open a block comment, `(;`.
fn block_comment(bytes: &[u8], at: usize) -> bool {
    bytes.get(at) == Some(&b'(') && bytes.get(at + 1) == Some(&b';')
}

/// The index just past the field that opens with the `(` at `at`: its matching `)`, past the
/// strings and comments in it, counting its lines.
fn past_field(bytes: &[u8], at: usize, line: &mut usize) -> usize {
    let mut at = at;
    let mut depth = 0_usize;
    while let Some(&byte) = bytes.get(at) {
        if line_comment(bytes, at) {
            at = past_line_comment(bytes, at);
            continue;
        }
        if block_comment(bytes, at) {
            at = past_block_comment(bytes, at, line);
            continue;
        }
        match byte {
            b'\n' => {
                *line += 1;
                at += 1;
            }
            b'"' => at = past_string(bytes, at, line),
            b'(' => {
                depth += 1;
                at += 1;
            }
            b')' => {
                at += 1;
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return at;
                }
            }
            _ => at += 1,
        }
    }
    bytes.len()
}

/// The keyword just after the `(` before `at`: `module`, `func`, `global` and the like.
fn keyword(text: &str, at: usize) -> &str {
    let rest = text.get(at..).unwrap_or_default().trim_start();
    let end = rest
        .find(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '.')
        })
        .unwrap_or(rest.len());
    &rest[..end]
}

/// `field` without its comments, each run of white space outside its strings one space, and
/// none just inside a parenthesis: two copies laid out differently read the same.
fn normalized(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut space = false;
    let mut lines = 0;
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if line_comment(bytes, at) {
            space = true;
            at = past_line_comment(bytes, at);
            continue;
        }
        if block_comment(bytes, at) {
            space = true;
            at = past_block_comment(bytes, at, &mut lines);
            continue;
        }
        if byte.is_ascii_whitespace() {
            space = true;
            at += 1;
            continue;
        }
        let next = if byte == b'"' {
            past_string(bytes, at, &mut lines)
        } else {
            at + 1
        };
        if space && byte != b')' && out.last().is_some_and(|last| *last != b'(') {
            out.push(b' ');
        }
        space = false;
        out.extend_from_slice(&bytes[at..next]);
        at = next;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One field of main.wat's module, as its text writes it: a function, a global, an import, the
/// memory, a data segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Its keyword: `func`, `global`, `import`, `memory`, `data` and the like.
    pub keyword: String,
    /// The name it gives what it defines, without the `$`: a function's, a global's, or what an
    /// import brings in; empty when it gives none.
    pub name: String,
    /// The line it opens on, from 1.
    pub line: usize,
    /// For a function, the part it says it comes from, when a `;; from:` line above it says so.
    pub from: Option<String>,
    /// Its text from its `(` to its closing `)`, without comments, each run of white space
    /// outside its strings one space, and none just inside a parenthesis.
    pub body: String,
}

impl Field {
    /// Whether it brings something in from outside the module, in either form: `(import …)`, or
    /// a function or a global with `(import …)` inside it.
    #[must_use]
    pub fn imports(&self) -> bool {
        self.keyword == "import" || self.body.contains("(import \"")
    }

    /// Whether it is a global the program can change: `(global $x (mut i32) …)`.
    #[must_use]
    pub fn changes(&self) -> bool {
        self.keyword == "global" && self.body.contains("(mut ")
    }
}

/// The name that opens `text`, without its `$`: empty when `text` doesn't open with `$`.
fn identifier(text: &str) -> String {
    text.strip_prefix('$').map_or_else(String::new, |name| {
        name.chars()
            .take_while(|character| !character.is_whitespace() && !"()\";".contains(*character))
            .collect()
    })
}

/// The name the field `normalized` gives what it defines, `keyword` being its keyword: the one
/// right after the keyword, or, for an import, the one of the function or global it brings in.
fn field_name(keyword: &str, normalized: &str) -> String {
    if keyword == "import" {
        return ["(func $", "(global $"]
            .iter()
            .find_map(|open| {
                let at = normalized.find(open)?;
                normalized.get(at + open.len() - 1..).map(identifier)
            })
            .unwrap_or_default();
    }
    let rest = normalized.trim_start_matches('(').trim_start();
    let rest = rest.strip_prefix(keyword).unwrap_or(rest).trim_start();
    identifier(rest)
}

/// main.wat's fields, in order: what its module holds, each function with the part a
/// `;; from:` line between the fields before it names.
#[must_use]
pub fn fields(text: &str) -> Vec<Field> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut depth = 0_usize;
    // The depth the module's fields open at: inside `(module …)`, or the text's own.
    let mut level: Option<usize> = None;
    let mut from = None;
    let mut line = 1;
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if line_comment(bytes, at) {
            let end = past_line_comment(bytes, at);
            if level == Some(depth)
                && let Some(part) = text
                    .get(at + 2..end)
                    .and_then(|comment| comment.trim().strip_prefix("from:"))
            {
                from = Some(part.trim().to_owned());
            }
            at = end;
            continue;
        }
        if block_comment(bytes, at) {
            at = past_block_comment(bytes, at, &mut line);
            continue;
        }
        match byte {
            b'\n' => {
                line += 1;
                at += 1;
            }
            b'"' => at = past_string(bytes, at, &mut line),
            b'(' => {
                let keyword = keyword(text, at + 1);
                let fields_at = *level.get_or_insert(usize::from(keyword == "module"));
                if depth == fields_at {
                    let first = line;
                    let end = past_field(bytes, at, &mut line);
                    let body = normalized(text.get(at..end).unwrap_or_default());
                    found.push(Field {
                        keyword: keyword.to_owned(),
                        name: field_name(keyword, &body),
                        line: first,
                        from: if keyword == "func" { from.take() } else { None },
                        body,
                    });
                    at = end;
                } else {
                    depth += 1;
                    at += 1;
                }
            }
            b')' => {
                depth = depth.saturating_sub(1);
                at += 1;
            }
            _ => at += 1,
        }
    }
    found
}

/// main.wat's own functions, in order: the `(func …)` fields of its module, each with the part a
/// `;; from:` line between the fields before it names.
#[must_use]
pub fn functions(text: &str) -> Vec<Function> {
    fields(text)
        .into_iter()
        .filter(|field| field.keyword == "func")
        .map(|field| Function {
            name: field.name,
            line: field.line,
            from: field.from,
            body: field.body,
        })
        .collect()
}

/// The words of a field's text, as [`Field::body`] holds it: what stands between its
/// parentheses and spaces, a string being one word.
fn words(body: &str) -> Vec<&str> {
    let bytes = body.as_bytes();
    let mut found = Vec::new();
    let mut lines = 0;
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if byte == b'(' || byte == b')' || byte.is_ascii_whitespace() {
            at += 1;
            continue;
        }
        let mut end = at;
        while let Some(&next) = bytes.get(end) {
            if next == b'(' || next == b')' || next.is_ascii_whitespace() {
                break;
            }
            end = if next == b'"' {
                past_string(bytes, end, &mut lines)
            } else {
                end + 1
            };
        }
        let end = end.min(bytes.len());
        if let Some(word) = body.get(at..end) {
            found.push(word);
        }
        at = end.max(at + 1);
    }
    found
}

/// Every name a field's text uses, without the `$`, each once, in order: its own, its locals'
/// and labels', and those of the functions and globals it names.
#[must_use]
pub fn names(body: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for word in words(body) {
        let name = identifier(word);
        if !name.is_empty() && !found.contains(&name) {
            found.push(name);
        }
    }
    found
}

/// The first function or global a field's text names by its number, not its name, as
/// `call 3` does: the instruction and the number.
#[must_use]
pub fn numbered(body: &str) -> Option<String> {
    let words = words(body);
    words.windows(2).find_map(|pair| {
        let by_number = matches!(pair[0], "call" | "global.get" | "global.set" | "ref.func")
            && !pair[1].starts_with('$');
        by_number.then(|| format!("{} {}", pair[0], pair[1]))
    })
}

/// A part of lengua's library, as the compile skill ships it in its `assets/parts` folder: its
/// folder's name, its idea.md and its main.wat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryPart {
    /// Its folder's name.
    pub name: &'static str,
    /// Its idea.md.
    pub idea: &'static str,
    /// Its main.wat.
    pub code: &'static str,
}

/// One part of the library, by its folder's name under the compile skill's `assets/parts`.
macro_rules! library_part {
    ($name:literal) => {
        LibraryPart {
            name: $name,
            idea: include_str!(concat!(
                "../../plugin/skills/compile/assets/parts/",
                $name,
                "/idea.md"
            )),
            code: include_str!(concat!(
                "../../plugin/skills/compile/assets/parts/",
                $name,
                "/main.wat"
            )),
        }
    };
}

/// lengua's library of parts, in the order of their names.
pub const LIBRARY: [LibraryPart; 3] = [
    library_part!("crc32"),
    library_part!("decimal"),
    library_part!("lines"),
];

/// The part of lengua's library that `idea` and `code` are, a folder's idea.md and main.wat:
/// the same main.wat byte for byte, and the same idea.md but for the `verified` and `generated`
/// keys, which `check` writes and lengua's hash of an idea leaves out.
#[must_use]
pub fn library_part(idea: &str, code: &str) -> Option<&'static LibraryPart> {
    let hash = crate::trust::idea_hash(idea.as_bytes());
    LIBRARY
        .iter()
        .find(|part| part.code == code && crate::trust::idea_hash(part.idea.as_bytes()) == hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A main.wat that takes `$parse` from the part `../numbers`.
    const TEXT: &str = "(module\n  ;; from: ../numbers\n  (func $parse (param i32) (result i32)  \
                        ;; returns: result\n    (local.get 0))\n  (func $main (export \
                        \"_start\")\n    (drop (call $parse (i32.const 1)))))\n";

    #[test]
    fn finds_each_function_and_the_part_it_comes_from() {
        let found = functions(TEXT);
        assert_eq!(found.len(), 2, "{found:#?}");
        assert_eq!(found[0].name, "parse");
        assert_eq!(found[0].line, 3);
        assert_eq!(found[0].from.as_deref(), Some("../numbers"));
        assert_eq!(
            found[0].body,
            "(func $parse (param i32) (result i32) (local.get 0))"
        );
        assert_eq!(found[1].name, "main");
        assert_eq!(found[1].line, 5);
        assert_eq!(found[1].from, None);
    }

    #[test]
    fn a_copy_laid_out_otherwise_reads_the_same() {
        let copy =
            "(module (func $parse   (param i32)\n\n (result i32) (; why ;) (local.get 0) ) )";
        assert_eq!(functions(copy)[0].body, functions(TEXT)[0].body);
        let changed = "(module (func $parse (param i32) (result i32) (local.get 1)))";
        assert_ne!(functions(changed)[0].body, functions(TEXT)[0].body);
        // A string keeps what looks like a comment or a parenthesis, and its spaces.
        let strings = "(module (data (i32.const 1024) \"a ;; ( b\") (func $f (nop)))";
        let found = functions(strings);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].body, "(func $f (nop))");
        // A `;; from:` inside a function is that function's own comment.
        let inside = "(module (func $f\n ;; from: ../x\n (nop)) (func $g))";
        assert!(
            functions(inside)
                .iter()
                .all(|function| function.from.is_none())
        );
        // Fields written without `(module …)` are the text's own.
        assert_eq!(functions("(func $a) (func $b)").len(), 2);
    }

    #[test]
    fn finds_each_field_and_the_names_a_function_uses() {
        let text = "(module\n  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $proc_exit \
                    (param i32)))\n  (memory (export \"memory\") 1 1)\n  (global $TEN i64 \
                    (i64.const 10))\n  (global $seen (mut i32) (i32.const 0))\n  (data (i32.const \
                    1024) \"$not a name\")\n  ;; from: ../decimal\n  (func $double (param $n i64) \
                    (result i64)\n    (i64.mul (local.get $n) (global.get $TEN)))\n  (func $main \
                    (export \"_start\")\n    (drop (call $double (i64.const 1)))\n    (call 0 \
                    (i32.const 0))))\n";
        let found = fields(text);
        let shown: Vec<(&str, &str)> = found
            .iter()
            .map(|field| (field.keyword.as_str(), field.name.as_str()))
            .collect();
        assert_eq!(
            shown,
            [
                ("import", "proc_exit"),
                ("memory", ""),
                ("global", "TEN"),
                ("global", "seen"),
                ("data", ""),
                ("func", "double"),
                ("func", "main"),
            ]
        );
        assert!(found[0].imports() && !found[2].imports(), "{found:#?}");
        assert!(found[3].changes() && !found[2].changes(), "{found:#?}");
        assert_eq!(found[2].body, "(global $TEN i64 (i64.const 10))");
        assert_eq!(found[5].from.as_deref(), Some("../decimal"));
        assert_eq!(found[6].from, None);
        // A function's names: its own, its locals', and what it calls and reads. A string's `$`
        // names nothing.
        assert_eq!(names(&found[5].body), ["double", "n", "TEN"]);
        assert_eq!(names(&found[4].body), Vec::<String>::new());
        // A function or a global named by its number.
        assert_eq!(numbered(&found[5].body), None);
        assert_eq!(numbered(&found[6].body).as_deref(), Some("call 0"));
    }

    #[test]
    fn the_library_s_parts_are_known_by_their_files() {
        for part in LIBRARY {
            assert_eq!(
                library_part(part.idea, part.code).map(|found| found.name),
                Some(part.name)
            );
            // lengua's own entry in `verified`, which check writes, changes nothing.
            let verified = part.idea.replacen(
                "\n---\n",
                "\nverified:\n  - { by: someone, at: 2026-11-11 }\n---\n",
                1,
            );
            assert_ne!(verified, part.idea, "{}", part.name);
            assert_eq!(
                library_part(&verified, part.code).map(|found| found.name),
                Some(part.name)
            );
            let changed = part.code.replacen("(module", "(module ", 1);
            assert_eq!(library_part(part.idea, &changed), None, "{}", part.name);
        }
        assert_eq!(library_part("", ""), None);
    }
}
