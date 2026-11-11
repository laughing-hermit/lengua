//! lengua's library of parts, and programs built on them (`- Builds on:`), through the server,
//! the same on every system: the parts the compile skill ships are the ones the examples hold;
//! a part's contract is held to what the rules find in its main.wat; a program takes a part's
//! functions word for word, and `check` says which, records them in lengua.lock and runs the
//! program; a copy that isn't the part's, a constant that isn't, a function the part doesn't
//! offer, a part changed since it was checked and a function lent less room than its contract
//! states are each refused, and a part's own tests catch a change to the part.

mod common;

use std::path::PathBuf;

use common::{Program, Protocol, Session};
use lengua::hash::Sha256Hex;
use lengua::wasm::{contract, parts};
use serde_json::{Value, json};

/// A file of the repository.
fn file(path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A file of the example `name`.
fn example(name: &str, file_name: &str) -> String {
    file(&format!("examples/{name}/{file_name}"))
}

/// Writes the example `name`, its idea.md and its main.wat, into `program`.
fn write_example(program: &Program, name: &str) {
    for file_name in ["idea.md", "main.wat"] {
        program.write(file_name, &example(name, file_name));
    }
}

/// A reply's list `field`, as its lines.
fn lines(reply: &Value, field: &str) -> Vec<String> {
    reply[field]
        .as_array()
        .map(|lines| {
            lines
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// `text` with `from`, which it must hold, replaced by `to`.
fn changed(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "the text has no {from:?}");
    text.replacen(from, to, 1)
}

/// The function `name` of `code`, a main.wat laid out as the examples are: from its `(func`
/// line to the blank line after it.
fn function(code: &str, name: &str) -> String {
    let open = format!("  (func ${name} ");
    let start = code
        .find(&open)
        .unwrap_or_else(|| panic!("no function ${name}"));
    let rest = &code[start..];
    let end = rest.find("\n\n").map_or(rest.len(), |end| end + 1);
    rest[..end].to_owned()
}

/// `lengua-mcp check` of the program in `folder`, the attester, with no time for proofs: its
/// exit code, and what it printed.
fn attest(folder: &std::path::Path) -> (Option<i32>, String) {
    let attested = std::process::Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("check")
        .arg(folder)
        .env("LENGUA_PROOF_SECONDS", "0")
        .output()
        .expect("lengua-mcp runs");
    (
        attested.status.code(),
        String::from_utf8_lossy(&attested.stdout).into_owned(),
    )
}

/// The bundle of sum with its two parts beside it, the parts checked to done: sum's folder,
/// then lines' and decimal's.
async fn sum_with_its_parts(session: &Session) -> (Program, Program, Program) {
    let sum = Program::new();
    write_example(&sum, "sum");
    let lines_part = sum.beside("lines");
    write_example(&lines_part, "lines");
    let decimal = sum.beside("decimal");
    write_example(&decimal, "decimal");
    for part in [&decimal, &lines_part] {
        session.record(part).await;
        let done = session.settle(part).await;
        assert_eq!(done["state"], "done", "{done}");
    }
    (sum, lines_part, decimal)
}

/// The compile skill ships each part of the library exactly as the examples hold it, the server
/// knows each by those files, and each states a contract.
#[test]
fn the_skill_ships_the_library_s_parts_as_the_examples_hold_them() {
    let shipped =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugin/skills/compile/assets/parts");
    let mut folders: Vec<String> = std::fs::read_dir(&shipped)
        .expect("the skill's parts")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    folders.sort();
    let known: Vec<&str> = parts::LIBRARY.iter().map(|part| part.name).collect();
    assert_eq!(
        folders, known,
        "the library the server knows is the one the skill ships"
    );
    for part in parts::LIBRARY {
        let name = part.name;
        assert_eq!(part.idea, example(name, "idea.md"), "{name}/idea.md");
        assert_eq!(part.code, example(name, "main.wat"), "{name}/main.wat");
        let stated = contract::read(part.idea)
            .unwrap_or_else(|problems| panic!("{name}'s contract: {problems:#?}"))
            .unwrap_or_else(|| panic!("{name} states no contract"));
        assert!(!stated.offers.is_empty(), "{name} offers nothing");
        for offer in &stated.offers {
            assert!(
                offer.name == name || offer.name.starts_with(&format!("{name}_")),
                "{name} offers ${}, which doesn't carry the part's name",
                offer.name
            );
        }
    }
}

/// sum, an example built on the parts lines and decimal: it isn't checked before its parts are;
/// each part's reply states its contract as the rules hold it; sum's reply names what it takes
/// from each part, lengua.lock records the parts with their hashes, and sum runs.
#[tokio::test]
async fn a_program_builds_on_the_library_s_parts() {
    let session = Session::start(Protocol::Discover).await;
    let sum = Program::new();
    write_example(&sum, "sum");
    let lines_part = sum.beside("lines");
    write_example(&lines_part, "lines");
    let decimal = sum.beside("decimal");
    write_example(&decimal, "decimal");
    // Before its parts are checked on this computer.
    let report = session.record(&sum).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    let problems = report["problems"].to_string();
    for part in ["lines", "decimal"] {
        let unchecked =
            format!("the part [{part}](../{part}/idea.md) isn't checked on this computer");
        assert!(problems.contains(&unchecked), "{report}");
    }
    // decimal: a part states its contract, which the rules hold.
    session.record(&decimal).await;
    let done = session.settle(&decimal).await;
    assert_eq!(done["state"], "done", "{done}");
    assert_eq!(
        lines(&done, "contract"),
        [
            "$decimal_parse: reads 20 bytes at `$at`; writes 8 bytes at `$into`",
            "$decimal_format: writes 20 bytes at `$into`",
        ],
        "{done}"
    );
    assert!(done["parts"].is_null(), "{done}");
    // lines: a part built on a part.
    session.record(&lines_part).await;
    let done = session.settle(&lines_part).await;
    assert_eq!(done["state"], "done", "{done}");
    assert_eq!(
        lines(&done, "contract"),
        [
            "$lines_read: writes 4097 bytes at `$text`",
            "$lines_end: reads 4096 bytes at `$text`",
        ],
        "{done}"
    );
    let built_on = lines(&done, "parts");
    assert_eq!(built_on.len(), 1, "{done}");
    assert!(
        built_on[0].starts_with(
            "[decimal](../decimal/idea.md): main.wat takes $decimal_format from it, word for \
             word with the constants it names; lengua's library part `decimal`, unchanged; its \
             own "
        ),
        "{done}"
    );
    // sum: done, with what it takes from each part, in its idea's order.
    let done = session.settle(&sum).await;
    assert_eq!(done["state"], "done", "{done}");
    assert!(done["contract"].is_null(), "{done}");
    let built_on = lines(&done, "parts");
    assert_eq!(built_on.len(), 2, "{done}");
    assert!(
        built_on[0].starts_with(
            "[lines](../lines/idea.md): main.wat takes $lines_read and $lines_end from it, word \
             for word with the constants each names; lengua's library part `lines`, unchanged; "
        ),
        "{done}"
    );
    assert!(
        built_on[1].starts_with(
            "[decimal](../decimal/idea.md): main.wat takes $decimal_parse and $decimal_format \
             from it, word for word with the constants each names; lengua's library part \
             `decimal`, unchanged; "
        ),
        "{done}"
    );
    // The lock records each part: its idea, its main.wat and what main.wat takes from it.
    let lock = sum.read("lengua.lock").expect("the lock");
    let recorded = |part: &str, functions: &str| {
        format!(
            "{{ part = \"../{part}\", idea = \"{}\", code = \"{}\", functions = [{functions}] }}",
            lengua::trust::idea_hash(example(part, "idea.md").as_bytes()),
            Sha256Hex::of(example(part, "main.wat").as_bytes()),
        )
    };
    let parts_line = format!(
        "\nparts = [{}, {}]\n",
        recorded("lines", "\"lines_read\", \"lines_end\""),
        recorded("decimal", "\"decimal_parse\", \"decimal_format\""),
    );
    assert!(lock.contains(&parts_line), "{lock}\nwanted: {parts_line}");
    // And it runs.
    let mut arguments = sum.arguments();
    arguments["stdin"] = json!("2\n3\n-4");
    let ran = session.call_ok("run", arguments).await;
    assert_eq!(ran["stdout"], "1\n", "{ran}");
    assert_eq!(ran["exit_code"], 0, "{ran}");
    session.service.cancel().await.expect("close the session");
}

/// main.wat is held to its parts at every check: a copy that isn't the part's function, a
/// constant that isn't the part's, a part the idea doesn't build on, a part nothing is taken
/// from, a function the part's contract doesn't offer, and a part changed since it was checked
/// are each refused; a change to a part is caught by the part's own tests.
#[tokio::test]
async fn a_copy_that_isn_t_the_part_s_is_refused() {
    let session = Session::start(Protocol::Discover).await;
    let (sum, _lines_part, decimal) = sum_with_its_parts(&session).await;
    let code = example("sum", "main.wat");
    session.record(&sum).await;
    let refused = async |code: &str, wanted: &[&str]| {
        sum.write("main.wat", code);
        let report = session.check(&sum).await;
        assert_eq!(report["state"], "code_problems", "{report}");
        let problems = report["problems"].to_string();
        for wanted in wanted {
            assert!(problems.contains(wanted), "{wanted:?} isn't in {report}");
        }
    };
    // One operator of a taken function changed.
    let tampered = changed(
        &code,
        "(br_if $invalid (i32.gt_u (local.get $c) (global.get $DECIMAL_NINE)))",
        "(br_if $invalid (i32.ge_u (local.get $c) (global.get $DECIMAL_NINE)))",
    );
    refused(
        &tampered,
        &[
            "main.wat's $decimal_parse differs from",
            "copy it again word for word",
        ],
    )
    .await;
    // A constant a taken function reads, changed.
    let constant = changed(
        &code,
        "(global $DECIMAL_TEN i64 (i64.const 10))",
        "(global $DECIMAL_TEN i64 (i64.const 9))",
    );
    refused(
        &constant,
        &[
            "reads `$DECIMAL_TEN`, which main.wat must define as the part does: `(global \
             $DECIMAL_TEN i64 (i64.const 10))`",
        ],
    )
    .await;
    // A function that says it comes from a part the idea doesn't build on.
    let elsewhere = changed(
        &code,
        "  ;; from: ../decimal\n  (func $decimal_parse ",
        "  ;; from: ../numbers\n  (func $decimal_parse ",
    );
    refused(&elsewhere, &["a part idea.md doesn't build on"]).await;
    // A part the idea builds on, with nothing taken from it.
    let own = code.replace("  ;; from: ../decimal\n", "");
    refused(
        &own,
        &["builds on [decimal](../decimal/idea.md), but main.wat takes nothing from it"],
    )
    .await;
    // A function of the part's that its contract doesn't offer.
    let unoffered = format!(
        "{}  (global $SCAN i32 (i32.const 21))\n\n  ;; from: ../decimal\n{})\n",
        code.trim_end()
            .strip_suffix(')')
            .expect("main.wat ends its module"),
        function(&example("decimal", "main.wat"), "text_length")
    );
    refused(
        &unoffered,
        &[
            "doesn't offer $text_length",
            "($decimal_parse and $decimal_format)",
        ],
    )
    .await;
    // The untouched main.wat is done.
    sum.write("main.wat", &code);
    let done = session.settle(&sum).await;
    assert_eq!(done["state"], "done", "{done}");
    // The part changed after that: main.wat is refused until the part is checked again, and
    // the part's own tests catch the change.
    let part_code = example("decimal", "main.wat");
    let mutant = changed(
        &part_code,
        "(global $DECIMAL_NINE i32 (i32.const 9))",
        "(global $DECIMAL_NINE i32 (i32.const 8))",
    );
    decimal.write("main.wat", &mutant);
    let report = session.check(&sum).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        report["problems"]
            .to_string()
            .contains("the part [decimal](../decimal/idea.md) isn't checked on this computer"),
        "{report}"
    );
    let caught = session.settle(&decimal).await;
    assert_eq!(caught["state"], "tests_failed", "{caught}");
    let report = session.check(&sum).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    // The part as it was: checked again, and sum is done again.
    decimal.write("main.wat", &part_code);
    let done = session.settle(&decimal).await;
    assert_eq!(done["state"], "done", "{done}");
    let done = session.settle(&sum).await;
    assert_eq!(done["state"], "done", "{done}");
    session.service.cancel().await.expect("close the session");
}

/// A taken function used outside its contract is refused by the memory rules, which hold each
/// call to what the function reaches through the addresses it is lent: a region with less room
/// than the contract states, at its end or at the place the program lends, and a read-only
/// region lent to a function that writes.
#[tokio::test]
async fn a_part_used_outside_its_contract_is_refused() {
    let session = Session::start(Protocol::Discover).await;
    let (sum, _lines_part, _decimal) = sum_with_its_parts(&session).await;
    let code = example("sum", "main.wat");
    session.record(&sum).await;
    let refused = async |code: &str, wanted: &[&str]| {
        sum.write("main.wat", code);
        let report = session.check(&sum).await;
        assert_eq!(report["state"], "code_problems", "{report}");
        let problems = report["problems"].to_string();
        for wanted in wanted {
            assert!(problems.contains(wanted), "{wanted:?} isn't in {report}");
        }
    };
    // $decimal_parse writes 8 bytes at $into: a region of 4 is too small.
    let small = changed(
        &code,
        "(global $number.size i32 (i32.const 8))",
        "(global $number.size i32 (i32.const 4))",
    );
    refused(
        &small,
        &[
            "what $decimal_parse reaches through argument 3",
            "of $number, which has 4 bytes",
        ],
    )
    .await;
    // $decimal_parse reads 20 bytes at $at: lent a line's start, $text needs that room after
    // the last place a line can start.
    let short = changed(
        &code,
        "(global $text.size i32 (i32.const 4160))",
        "(global $text.size i32 (i32.const 4100))",
    );
    refused(
        &short,
        &[
            "what $decimal_parse reaches through argument 1",
            "of $text, which has 4100 bytes",
        ],
    )
    .await;
    // $decimal_format writes at $into: a read-only region can't be lent to it.
    let read_only = changed(
        &code,
        "(call $decimal_format (local.get $sum) (global.get $out))",
        "(call $decimal_format (local.get $sum) (global.get $nan))",
    );
    refused(&read_only, &["writes to $nan, a read-only region"]).await;
    session.service.cancel().await.expect("close the session");
}

/// A part's contract is held to its main.wat: a clause that states another reach than the
/// rules find, and an offer of a function no program could take, are problems of the part.
#[tokio::test]
async fn a_part_s_contract_is_held_to_its_main_wat() {
    let session = Session::start(Protocol::Discover).await;
    let part = Program::new();
    let idea = example("decimal", "idea.md");
    part.write("main.wat", &example("decimal", "main.wat"));
    // A reach the function goes past.
    let less = changed(
        &idea,
        "- `$decimal_parse`: reads 20 bytes at `$at`;",
        "- `$decimal_parse`: reads 19 bytes at `$at`;",
    );
    part.write("idea.md", &less);
    let report = session.record(&part).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        report["problems"].to_string().contains(
            "`# Contract` says `$decimal_parse`: reads 19 bytes at `$at`; writes 8 bytes at \
             `$into`, but in main.wat it reads 20 bytes at `$at`; writes 8 bytes at `$into`"
        ),
        "{report}"
    );
    // A function that names the part's own regions.
    let own = changed(
        &idea,
        "- `$decimal_format`: writes 20 bytes at `$into`.",
        "- `$main`: uses no memory. It is the program.\n- `$decimal_format`: writes 20 bytes at \
         `$into`.",
    );
    part.write("idea.md", &own);
    let report = session.record(&part).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    let problems = report["problems"].to_string();
    assert!(
        problems.contains("no program can take what `# Contract` offers here")
            && problems.contains("`$main` names the region"),
        "{report}"
    );
    // An item lengua can't read.
    let unread = changed(
        &idea,
        "- `$decimal_format`: writes 20 bytes at `$into`.",
        "- `$decimal_format`: fills 20 bytes at `$into`.",
    );
    part.write("idea.md", &unread);
    let report = session.record(&part).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        report["problems"]
            .to_string()
            .contains("lengua can't read the clause `fills 20 bytes at `$into``"),
        "{report}"
    );
    // The contract as it is: done.
    part.write("idea.md", &idea);
    session.record(&part).await;
    let done = session.settle(&part).await;
    assert_eq!(done["state"], "done", "{done}");
    session.service.cancel().await.expect("close the session");
    // The attester holds the contract too: it says each function that holds, and refuses a
    // section that says another reach.
    let (code, said) = attest(part.path());
    assert_eq!(code, Some(0), "{said}");
    assert!(
        said.contains(
            "\nidea.md: its contract holds: $decimal_parse: reads 20 bytes at `$at`; writes 8 \
             bytes at `$into`\n"
        ),
        "{said}"
    );
    part.write("idea.md", &less);
    let (code, said) = attest(part.path());
    assert_eq!(code, Some(1), "{said}");
    assert!(
        said.contains("but in main.wat it reads 20 bytes at `$at`"),
        "{said}"
    );
    assert_eq!(said.lines().last(), Some("not attested"), "{said}");
}

/// The secret rule goes through a taken function as through any other: bytes with its input
/// named secret is refused, since `$lines_end` and `$bytes_equal`, each word for word its
/// part's, branch on the bytes they read; without the `- Secret:` line the same main.wat is
/// done.
#[tokio::test]
async fn the_secret_rule_goes_through_a_taken_function() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/bytes");
    session
        .settle_parts(&program, &folder)
        .await
        .expect("the parts bytes builds on are done");
    let idea = example("bytes", "idea.md");
    let secret = changed(
        &idea,
        "- Uses: standard input\n",
        "- Uses: standard input\n- Secret: standard input\n",
    );
    program.write("idea.md", &secret);
    program.write("main.wat", &example("bytes", "main.wat"));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        report["problems"]
            .to_string()
            .contains("so a secret decides"),
        "{report}"
    );
    program.write("idea.md", &idea);
    session.record(&program).await;
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    assert_eq!(
        lines(&done, "contract"),
        ["$bytes_equal: reads 4096 bytes at `$a`; reads 4096 bytes at `$b`"],
        "{done}"
    );
    session.service.cancel().await.expect("close the session");
}

/// crc32's `$crc32_update` with each byte's step written as the eight columns of the CRC's
/// table, one for each bit of the byte xored into the CRC, in place of eight shifts: the same
/// function of its arguments, in another form, to measure which form the solvers prove further.
const CRC32_BY_COLUMNS: &str = "  (func $crc32_update (param $crc i32) (param $at i32) (param \
$length i32) (result i32)
    (local $i i32)
    (local $c i32)
    (local $m i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $CRC32_TEXT))
      (then (call $assert_fail (i32.const 831)) unreachable))
    (local.set $c (i32.xor (local.get $crc) (global.get $CRC32_ALL_ONES)))
    (block $done
      (local.set $left (global.get $CRC32_TEXT))
      (loop $byte  ;; bound: 4096
        (br_if $done (i32.ge_u (local.get $i) (local.get $length)))
        (if (i32.ge_u (local.get $i) (global.get $CRC32_TEXT))
          (then (call $assert_fail (i32.const 832)) unreachable))
        (local.set $m
          (i32.xor (local.get $c) (i32.load8_u (i32.add (local.get $at) (local.get $i)))))
        (local.set $c
          (i32.xor (i32.shr_u (local.get $c) (i32.const 8))
            (i32.xor
              (i32.xor
                (i32.xor
                  (i32.and (global.get $CRC32_BIT0)
                    (i32.sub (i32.const 0) (i32.and (local.get $m) (i32.const 1))))
                  (i32.and (global.get $CRC32_BIT1)
                    (i32.sub (i32.const 0)
                      (i32.and (i32.shr_u (local.get $m) (i32.const 1)) (i32.const 1)))))
                (i32.xor
                  (i32.and (global.get $CRC32_BIT2)
                    (i32.sub (i32.const 0)
                      (i32.and (i32.shr_u (local.get $m) (i32.const 2)) (i32.const 1))))
                  (i32.and (global.get $CRC32_BIT3)
                    (i32.sub (i32.const 0)
                      (i32.and (i32.shr_u (local.get $m) (i32.const 3)) (i32.const 1))))))
              (i32.xor
                (i32.xor
                  (i32.and (global.get $CRC32_BIT4)
                    (i32.sub (i32.const 0)
                      (i32.and (i32.shr_u (local.get $m) (i32.const 4)) (i32.const 1))))
                  (i32.and (global.get $CRC32_BIT5)
                    (i32.sub (i32.const 0)
                      (i32.and (i32.shr_u (local.get $m) (i32.const 5)) (i32.const 1)))))
                (i32.xor
                  (i32.and (global.get $CRC32_BIT6)
                    (i32.sub (i32.const 0)
                      (i32.and (i32.shr_u (local.get $m) (i32.const 6)) (i32.const 1))))
                  (i32.and (global.get $CRC32_BIT7)
                    (i32.sub (i32.const 0)
                      (i32.and (i32.shr_u (local.get $m) (i32.const 7)) (i32.const 1)))))))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (i32.xor (local.get $c) (global.get $CRC32_ALL_ONES)))
";

/// The eight columns' constants, in place of the polynomial and the count of a byte's bits.
const CRC32_COLUMNS: &str = "  (global $CRC32_BIT0 i32 (i32.const 0x77073096))
  (global $CRC32_BIT1 i32 (i32.const 0xee0e612c))
  (global $CRC32_BIT2 i32 (i32.const 0x076dc419))
  (global $CRC32_BIT3 i32 (i32.const 0x0edb8832))
  (global $CRC32_BIT4 i32 (i32.const 0x1db71064))
  (global $CRC32_BIT5 i32 (i32.const 0x3b6e20c8))
  (global $CRC32_BIT6 i32 (i32.const 0x76dc4190))
  (global $CRC32_BIT7 i32 (i32.const 0xedb88320))
";

/// How far crc32's proofs go with a build's full five minutes, for the part as it ships and for
/// the same function by columns: each proof's line is printed, for a person to read. Neither
/// form is asserted proven; both must be done, their tests passed.
#[tokio::test]
#[ignore = "measures crc32's proofs over five minutes a form: by hand, or on CI's dispatch"]
async fn crc32_s_proof_with_its_full_time() {
    let session = Session::proving_for(Protocol::Discover, 300).await;
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/crc32");
    let shipped = example("crc32", "main.wat");
    let columns = changed(
        &changed(
            &changed(
                &shipped,
                &function(&shipped, "crc32_update"),
                CRC32_BY_COLUMNS,
            ),
            "  (global $CRC32_POLY i32 (i32.const 0xedb88320))\n",
            CRC32_COLUMNS,
        ),
        "  (global $CRC32_BITS i32 (i32.const 8))\n",
        "",
    );
    for (name, code) in [("as shipped", &shipped), ("by columns", &columns)] {
        let program = Program::new();
        session
            .settle_parts(&program, &folder)
            .await
            .expect("decimal, which crc32 builds on, is done");
        program.write("idea.md", &example("crc32", "idea.md"));
        program.write("main.wat", code);
        session.record(&program).await;
        let report = session.settle(&program).await;
        println!("CRC {name}: state {}", report["state"]);
        for field in ["problems", "proofs"] {
            for line in lines(&report, field) {
                println!("CRC {name}: {field}: {line}");
            }
        }
        assert_eq!(report["state"], "done", "{name}: {report}");
    }
    session.service.cancel().await.expect("close the session");
}

/// How far sort64's proofs go with a build's full five minutes: each proof's line is printed,
/// for a person to read. None is asserted proven; the part must be done, its tests passed.
#[tokio::test]
#[ignore = "measures sort64's proofs over five minutes: by hand, or on CI's dispatch"]
async fn sort64_s_proof_with_its_full_time() {
    let session = Session::proving_for(Protocol::Discover, 300).await;
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sort64");
    let program = Program::new();
    session
        .settle_parts(&program, &folder)
        .await
        .expect("lines and decimal, which sort64 builds on, are done");
    write_example(&program, "sort64");
    session.record(&program).await;
    let report = session.settle(&program).await;
    println!("SORT state {}", report["state"]);
    for field in ["problems", "proofs"] {
        for line in lines(&report, field) {
            println!("SORT {field}: {line}");
        }
    }
    assert_eq!(report["state"], "done", "{report}");
    session.service.cancel().await.expect("close the session");
}
