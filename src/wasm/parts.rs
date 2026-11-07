//! main.wat's own functions as its text writes them, for what it takes from the parts its idea
//! builds on (`- Builds on:`): a function taken from a part is marked `;; from: <part>` on a line
//! above its `(func`, and is the part's own function word for word.

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

/// The name a `(func …)` field gives its function, without the `$`; empty when it gives none.
fn name_of(field: &str) -> String {
    let rest = field.trim_start_matches('(').trim_start();
    let rest = rest.strip_prefix("func").unwrap_or(rest).trim_start();
    rest.strip_prefix('$').map_or_else(String::new, |name| {
        name.chars()
            .take_while(|character| !character.is_whitespace() && !"()\";".contains(*character))
            .collect()
    })
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

/// main.wat's own functions, in order: the `(func …)` fields of its module, each with the part a
/// `;; from:` line between the fields before it names.
#[must_use]
pub fn functions(text: &str) -> Vec<Function> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut depth = 0_usize;
    // The depth the module's fields open at: inside `(module …)`, or the text's own.
    let mut fields: Option<usize> = None;
    let mut from = None;
    let mut line = 1;
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if line_comment(bytes, at) {
            let end = past_line_comment(bytes, at);
            if fields == Some(depth)
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
                let level = *fields.get_or_insert(usize::from(keyword == "module"));
                if depth == level && keyword == "func" {
                    let first = line;
                    let end = past_field(bytes, at, &mut line);
                    let field = text.get(at..end).unwrap_or_default();
                    found.push(Function {
                        name: name_of(field),
                        line: first,
                        from: from.take(),
                        body: normalized(field),
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
}
