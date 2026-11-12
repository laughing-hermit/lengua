//! `lengua-mcp lsp`: a first cut of a language server, over standard input and output, for an
//! editor that shows a program folder's idea.md and main.wat. It does one thing: when one of
//! the two files is opened, changed, saved or closed, it publishes the findings of both, each
//! on its line. The findings are `check`'s own for the text: idea.md's format and its tests
//! ([`crate::idea::parse`], [`crate::testfile::check`]), main.wat's build
//! ([`crate::wasm::text::build`]) and main.wat's rules ([`crate::checker::wasm::check`]).
//!
//! It runs no test and no proof, reads no lock, writes nothing, and holds a program to no part
//! it builds on: those stay `check`'s. It has no completion, no hover and no formatting.

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::checker::wasm::{self as rules, Context};
use crate::idea;
use crate::state::IDEA_FILE;
use crate::testfile;
use crate::wasm::{CODE_FILE, text};

/// The longest message the server reads.
const MAX_MESSAGE: usize = 16 * 1024 * 1024;

/// What each diagnostic names as its source.
const SOURCE: &str = "lengua";

/// JSON-RPC's code for a method the server doesn't have.
const METHOD_NOT_FOUND: i32 = -32601;

/// One finding of `check` in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The line it is on, from 1, when it belongs to one.
    pub line: Option<usize>,
    /// What is wrong, and how to fix it.
    pub message: String,
}

/// What `check` finds in idea.md's text and in main.wat's, as far as the two texts say it:
/// idea.md's findings, then main.wat's. main.wat is held to the rules with what the idea
/// grants, so an idea that can't be read leaves the rules unchecked, and main.wat's findings
/// say so.
#[must_use]
pub fn findings(idea_text: Option<&str>, code: Option<&str>) -> (Vec<Finding>, Vec<Finding>) {
    let mut of_idea = Vec::new();
    let mut of_code = Vec::new();
    let idea = idea_text.and_then(|text| match idea::parse(text) {
        Ok(idea) => {
            if let Err(problems) = testfile::check(&idea) {
                of_idea.extend(problems.into_iter().map(|problem| Finding {
                    line: problem.line,
                    message: problem.message,
                }));
            }
            Some(idea)
        }
        Err(problems) => {
            of_idea.extend(problems.into_iter().map(|problem| Finding {
                line: problem.line,
                message: problem.message,
            }));
            None
        }
    });
    // An idea of an outside module has no main.wat to hold to a rule.
    let outside = idea_text.is_some_and(|text| !matches!(idea::outside(text), Ok(None)));
    if let Some(code) = code {
        match (text::build(code), &idea) {
            (Err(problem), _) => of_code.push(Finding {
                line: problem.line,
                message: problem.message,
            }),
            (Ok(_), Some(_)) if outside => {}
            (Ok(built), Some(idea)) => {
                let context = Context {
                    uses: &idea.uses,
                    network: &idea.network,
                    secrets: &idea.secrets,
                };
                let report = rules::check(code, &built, &context);
                of_code.extend(report.violations.iter().map(|violation| Finding {
                    line: violation.line,
                    message: violation.to_string(),
                }));
            }
            (Ok(_), None) => of_code.push(Finding {
                line: None,
                message: format!(
                    "{CODE_FILE} builds; its rules aren't checked, since they take what \
                     {IDEA_FILE} grants, and {IDEA_FILE} is missing or has problems"
                ),
            }),
        }
    }
    (of_idea, of_code)
}

/// `text` with each `%XX` as the byte it writes.
fn percent_decoded(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(byte) = bytes.get(at) {
        if *byte == b'%' {
            let pair = text.get(at + 1..at + 3)?;
            decoded.push(u8::from_str_radix(pair, 16).ok()?);
            at += 3;
        } else {
            decoded.push(*byte);
            at += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

/// The file a `file:` URI names.
fn path_of(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let path = if rest.starts_with('/') {
        rest
    } else {
        rest.get(rest.find('/')?..)?
    };
    let decoded = percent_decoded(path)?;
    // Windows writes `C:\x` as `/C:/x`.
    let drive = decoded.as_bytes();
    let windows = drive.len() >= 3
        && drive.first() == Some(&b'/')
        && drive.get(1).is_some_and(u8::is_ascii_alphabetic)
        && drive.get(2) == Some(&b':');
    let path = if windows {
        decoded.get(1..)?
    } else {
        decoded.as_str()
    };
    Some(PathBuf::from(path))
}

/// The findings as the diagnostics of one file: each on the whole of its line, or on the
/// file's first line when it has none.
fn diagnostics(findings: &[Finding]) -> Value {
    Value::Array(
        findings
            .iter()
            .map(|finding| {
                let line = finding.line.map_or(0, |line| line.saturating_sub(1));
                json!({
                    "range": {
                        "start": { "line": line, "character": 0 },
                        "end": { "line": line + 1, "character": 0 },
                    },
                    "severity": 1,
                    "source": SOURCE,
                    "message": finding.message,
                })
            })
            .collect(),
    )
}

/// Reads one message: its headers, then as many bytes as `Content-Length` says. None at the
/// input's end.
fn read_message(input: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let length = length
        .filter(|length| *length <= MAX_MESSAGE)
        .ok_or_else(|| io::Error::other("a message with no length, or one too long"))?;
    let mut body = vec![0_u8; length];
    input.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(io::Error::other)
}

/// Writes one message, its length first.
fn write_message(output: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message).map_err(io::Error::other)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()
}

/// Publishes the findings of the folder of `uri`, when `uri` names an idea.md or a main.wat:
/// for each of the two files that is there, its findings, or none. A file's text is the one
/// the editor has open, else the one on disk.
fn publish(output: &mut impl Write, open: &BTreeMap<String, String>, uri: &str) -> io::Result<()> {
    let Some((base, name)) = uri.rsplit_once('/') else {
        return Ok(());
    };
    if name != IDEA_FILE && name != CODE_FILE {
        return Ok(());
    }
    let text_of = |file: &str| {
        let uri = format!("{base}/{file}");
        open.get(&uri)
            .cloned()
            .or_else(|| path_of(&uri).and_then(|path| std::fs::read_to_string(path).ok()))
    };
    let idea_text = text_of(IDEA_FILE);
    let code = text_of(CODE_FILE);
    let (of_idea, of_code) = findings(idea_text.as_deref(), code.as_deref());
    let files = [
        (IDEA_FILE, of_idea, idea_text.is_some()),
        (CODE_FILE, of_code, code.is_some()),
    ];
    for (file, found, there) in files {
        if there {
            let message = json!({
                "jsonrpc": "2.0",
                "method": "textDocument/publishDiagnostics",
                "params": {
                    "uri": format!("{base}/{file}"),
                    "diagnostics": diagnostics(&found),
                },
            });
            write_message(output, &message)?;
        }
    }
    Ok(())
}

/// Serves an editor on `input` and `output` until it says `exit` or its input ends. Gives
/// whether the editor said `shutdown` first, as the protocol asks.
///
/// # Errors
///
/// When a message can't be read or written.
pub fn serve(input: &mut impl BufRead, output: &mut impl Write) -> io::Result<bool> {
    let mut open: BTreeMap<String, String> = BTreeMap::new();
    let mut shut = false;
    while let Some(message) = read_message(input)? {
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let id = message.get("id").cloned();
        let document = &message["params"]["textDocument"];
        let uri = document["uri"].as_str().unwrap_or_default().to_owned();
        match method {
            "initialize" => {
                let result = json!({
                    "capabilities": {
                        "textDocumentSync": { "openClose": true, "change": 1, "save": {} },
                    },
                    "serverInfo": { "name": "lengua-mcp", "version": env!("CARGO_PKG_VERSION") },
                });
                write_message(
                    output,
                    &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                )?;
            }
            "shutdown" => {
                shut = true;
                write_message(
                    output,
                    &json!({ "jsonrpc": "2.0", "id": id, "result": null }),
                )?;
            }
            "exit" => return Ok(shut),
            "textDocument/didOpen" => {
                if let Some(text) = document["text"].as_str() {
                    open.insert(uri.clone(), text.to_owned());
                }
                publish(output, &open, &uri)?;
            }
            "textDocument/didChange" => {
                // The whole text each time, as the server asked for.
                let text = message["params"]["contentChanges"]
                    .as_array()
                    .and_then(|changes| changes.last())
                    .and_then(|change| change["text"].as_str());
                if let Some(text) = text {
                    open.insert(uri.clone(), text.to_owned());
                }
                publish(output, &open, &uri)?;
            }
            "textDocument/didSave" => publish(output, &open, &uri)?,
            "textDocument/didClose" => {
                open.remove(&uri);
                publish(output, &open, &uri)?;
            }
            // Another request gets an answer, so the editor doesn't wait; a notification and
            // the editor's own answers get none.
            _ => {
                if let Some(id) = id.filter(|_| !method.is_empty()) {
                    let error = json!({
                        "code": METHOD_NOT_FOUND,
                        "message": format!("lengua's language server has no {method}"),
                    });
                    write_message(
                        output,
                        &json!({ "jsonrpc": "2.0", "id": id, "error": error }),
                    )?;
                }
            }
        }
    }
    Ok(shut)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The add idea and its code.
    const IDEA: &str = include_str!("../tests/fixtures/add/idea.md");
    const CODE: &str = include_str!("../tests/fixtures/add/main.wat");

    /// `messages` as an editor sends them.
    fn framed(messages: &[Value]) -> Vec<u8> {
        let mut wire = Vec::new();
        for message in messages {
            write_message(&mut wire, message).expect("written");
        }
        wire
    }

    /// What the server wrote, message by message.
    fn unframed(wire: &[u8]) -> Vec<Value> {
        let mut reader = wire;
        let mut messages = Vec::new();
        while let Some(message) = read_message(&mut reader).expect("read") {
            messages.push(message);
        }
        messages
    }

    /// The diagnostics the server published for the file `name`, the last time it did.
    fn published(messages: &[Value], name: &str) -> Option<Vec<Value>> {
        messages
            .iter()
            .rev()
            .find(|message| {
                message["method"] == "textDocument/publishDiagnostics"
                    && message["params"]["uri"]
                        .as_str()
                        .is_some_and(|uri| uri.ends_with(name))
            })
            .and_then(|message| message["params"]["diagnostics"].as_array().cloned())
    }

    /// The checked add program has no finding; a main.wat that doesn't build has the builder's
    /// on its line; one that breaks a rule has the rule's; and an idea with a problem has it on
    /// its line, with main.wat's rules left unchecked.
    #[test]
    fn the_findings_are_checks_own_on_their_lines() {
        let (of_idea, of_code) = findings(Some(IDEA), Some(CODE));
        assert!(of_idea.is_empty(), "{of_idea:#?}");
        assert!(of_code.is_empty(), "{of_code:#?}");

        let broken = CODE.replacen("(local.set $err", "(local.sett $err", 1);
        let line = CODE
            .lines()
            .position(|line| line.contains("(local.set $err"))
            .expect("the line")
            + 1;
        let (_, of_code) = findings(Some(IDEA), Some(&broken));
        assert_eq!(of_code.len(), 1, "{of_code:#?}");
        assert_eq!(of_code[0].line, Some(line), "{of_code:#?}");

        // A loop with no bound breaks a rule of the dialect.
        let unbounded = CODE.replacen("(loop $digit  ;; bound: 20", "(loop $digit", 1);
        assert_ne!(unbounded, CODE);
        let (_, of_code) = findings(Some(IDEA), Some(&unbounded));
        assert!(
            of_code
                .iter()
                .any(|finding| finding.line.is_some() && finding.message.contains("rule")),
            "{of_code:#?}"
        );

        let no_purpose = IDEA.replace("# Purpose", "# Porpoise");
        assert_ne!(no_purpose, IDEA);
        let (of_idea, of_code) = findings(Some(&no_purpose), Some(CODE));
        assert!(!of_idea.is_empty());
        assert_eq!(of_code.len(), 1, "{of_code:#?}");
        assert!(
            of_code[0].message.contains("aren't checked"),
            "{of_code:#?}"
        );
    }

    /// An editor opens a folder's main.wat with text that doesn't build, then changes it to
    /// text that does: the server publishes the build's finding on its line, then none, for the
    /// file's own URI, and answers what the protocol asks an answer for.
    #[test]
    fn an_editor_gets_the_findings_of_what_it_opens_and_changes() {
        let folder = tempfile::tempdir().expect("a folder");
        std::fs::write(folder.path().join("idea.md"), IDEA).expect("idea.md");
        std::fs::write(folder.path().join("main.wat"), CODE).expect("main.wat");
        let base = format!(
            "file://{}{}",
            if folder.path().to_string_lossy().starts_with('/') {
                ""
            } else {
                "/"
            },
            folder.path().to_string_lossy().replace('\\', "/")
        );
        let uri = format!("{base}/main.wat");
        let broken = CODE.replacen("(local.set $err", "(local.sett $err", 1);
        let wire = framed(&[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
            json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }),
            json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
                "textDocument": { "uri": uri, "text": broken } } }),
        ]);
        let mut said = Vec::new();
        let shut = serve(&mut wire.as_slice(), &mut said).expect("served");
        assert!(!shut, "the input ended with no shutdown");
        let messages = unframed(&said);
        assert_eq!(messages[0]["id"], 1);
        assert_eq!(
            messages[0]["result"]["capabilities"]["textDocumentSync"]["change"],
            1
        );
        let of_code = published(&messages, "/main.wat").expect("main.wat's");
        assert_eq!(of_code.len(), 1, "{messages:#?}");
        assert_eq!(of_code[0]["source"], "lengua");
        assert!(of_code[0]["range"]["start"]["line"].as_u64() > Some(0));
        // idea.md is read from the folder, and has no finding.
        let of_idea = published(&messages, "/idea.md").expect("idea.md's");
        assert!(of_idea.is_empty(), "{messages:#?}");

        let wire = framed(&[
            json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
                "textDocument": { "uri": uri, "text": broken } } }),
            json!({ "jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
                "textDocument": { "uri": uri },
                "contentChanges": [{ "text": CODE }] } }),
            json!({ "jsonrpc": "2.0", "id": 7, "method": "textDocument/hover", "params": {} }),
            json!({ "jsonrpc": "2.0", "id": 8, "method": "shutdown" }),
            json!({ "jsonrpc": "2.0", "method": "exit" }),
        ]);
        let mut said = Vec::new();
        let shut = serve(&mut wire.as_slice(), &mut said).expect("served");
        assert!(shut);
        let messages = unframed(&said);
        let of_code = published(&messages, "/main.wat").expect("main.wat's");
        assert!(of_code.is_empty(), "{messages:#?}");
        let hover = messages
            .iter()
            .find(|message| message["id"] == 7)
            .expect("an answer");
        assert_eq!(hover["error"]["code"], METHOD_NOT_FOUND);
        assert!(messages.iter().any(|message| message["id"] == 8));
        // A file that is neither of the two gets nothing.
        let other = format!("{base}/notes.txt");
        let wire = framed(&[
            json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
                "textDocument": { "uri": other, "text": "words" } } }),
        ]);
        let mut said = Vec::new();
        serve(&mut wire.as_slice(), &mut said).expect("served");
        assert!(said.is_empty());
    }

    #[test]
    fn a_file_uri_names_its_path() {
        assert_eq!(
            path_of("file:///home/me/a%20b/main.wat"),
            Some(PathBuf::from("/home/me/a b/main.wat"))
        );
        assert_eq!(
            path_of("file:///C:/me/main.wat"),
            Some(PathBuf::from("C:/me/main.wat"))
        );
        assert_eq!(path_of("https://example.org/main.wat"), None);
        assert_eq!(path_of("file:///a%zz"), None);
    }
}
