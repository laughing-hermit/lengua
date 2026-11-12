//! The parts a program builds on (`- Builds on:`), as `check` holds main.wat to them, and a
//! part's own contract.
//!
//! A part is a program folder like any other, with a `# Contract` section in its idea.md that
//! lists the functions a program may take from it ([`crate::wasm::contract`]). A program takes a
//! function by copying it into its main.wat under a `;; from: <part>` line. `check` then holds
//! the copy to the part ([`taken`]): the part is checked on this computer, with no proof that
//! found it wrong; its contract offers the function; the copy is the part's function word for
//! word; and everything the function names outside itself is the part's too, word for word: the
//! constants it reads, the functions of the part it calls, taken the same way, and the host
//! functions it imports. A taken function names no region and no global that changes, so what
//! it does depends only on its arguments, the bytes it is lent and the host: it means in the
//! program what it means in the part. main.wat stays the one text lengua builds, checks, tests,
//! proves and runs, so the rules, the secret rule among them, and the proofs go through a taken
//! function as through any other.

use std::path::PathBuf;

use super::state_of;
use crate::checker::wasm::Lends;
use crate::data_dir::DataDir;
use crate::hash::Sha256Hex;
use crate::idea;
use crate::lock::{self, Key, PartStamp, ProofStamp};
use crate::prove::contract::Claim;
use crate::state::{CODE_FILE, IDEA_FILE, ProgramFolder, State};
use crate::trust;
use crate::wasm::contract::{self, Contract};
use crate::wasm::parts::{self, Field};

/// A part as main.wat builds on it.
#[derive(Debug, Clone)]
pub(super) struct Taken {
    /// The part, as the idea's `- Builds on:` line links to it.
    part: idea::Part,
    /// Its folder.
    shown: String,
    /// Its main.wat's fields.
    fields: Vec<Field>,
    /// Its contract.
    contract: Contract,
    /// lengua's hash of its idea.md.
    idea: Sha256Hex,
    /// The SHA-256 of its main.wat.
    code: Sha256Hex,
    /// The part of lengua's library it is, when its two files are that part's.
    library: Option<&'static str>,
    /// What its own lock records of its proofs.
    proofs: Option<Box<ProofStamp>>,
    /// The functions main.wat takes from it, without the `$`, in main.wat's order.
    functions: Vec<String>,
}

/// The ids in `ids`, as a reply lists them: `B1`, `B1 and B2`, `B1, B2 and B3`.
fn listed(ids: &[String]) -> String {
    match ids {
        [] => String::new(),
        [one] => one.clone(),
        [most @ .., last] => format!("{} and {last}", most.join(", ")),
    }
}

impl Taken {
    /// What its own lock records of its proofs, in words.
    fn proofs_line(&self) -> String {
        let Some(proofs) = &self.proofs else {
            return format!(
                "its own proofs aren't recorded yet: call check with path {} to see them",
                self.shown
            );
        };
        let mut said = Vec::new();
        if !proofs.proven.is_empty() {
            said.push(format!(
                "{} proven for every input in range",
                listed(&proofs.proven)
            ));
        }
        for bounded in &proofs.bounded {
            said.push(format!(
                "{} proven for every input with text up to {} of {} bytes",
                bounded.id, bounded.bytes, bounded.of
            ));
        }
        let unproven: Vec<String> = proofs.unproven.iter().map(|(id, _)| id.clone()).collect();
        if !unproven.is_empty() {
            said.push(format!("{} tested, not proven", listed(&unproven)));
        }
        if said.is_empty() {
            "its own idea has no generated test to prove".to_owned()
        } else {
            format!("its own proofs: {}", said.join("; "))
        }
    }

    /// The part in one line of `check`'s reply.
    fn line(&self) -> String {
        let functions: Vec<String> = self
            .functions
            .iter()
            .map(|name| format!("${name}"))
            .collect();
        let whose = self.library.map_or_else(
            || "a part of this bundle".to_owned(),
            |name| format!("lengua's library part `{name}`, unchanged"),
        );
        format!(
            "[{}]({}): {CODE_FILE} takes {} from it, word for word with the constants {} names; \
             {whose}; {}. This program's own tests and proofs go through {} again, as it calls \
             {}",
            self.part.name,
            self.part.idea,
            listed(&functions),
            if functions.len() == 1 { "it" } else { "each" },
            self.proofs_line(),
            if functions.len() == 1 { "it" } else { "them" },
            if functions.len() == 1 { "it" } else { "them" },
        )
    }

    /// What lengua.lock records of it.
    fn stamp(&self) -> PartStamp {
        PartStamp {
            part: self.part.folder(),
            idea: self.idea.clone(),
            code: self.code.clone(),
            functions: self.functions.clone(),
        }
    }
}

/// Each part in a line of `check`'s reply.
pub(super) fn lines(taken: &[Taken]) -> Vec<String> {
    taken.iter().map(Taken::line).collect()
}

/// What lengua.lock records of the parts.
pub(super) fn stamps(taken: &[Taken]) -> Vec<PartStamp> {
    taken.iter().map(Taken::stamp).collect()
}

/// The part `part` of the program in `folder`, ready to take functions from: checked on this
/// computer, with its main.wat, its contract and no proof that found it wrong; or why not.
fn open(data_dir: &DataDir, folder: &ProgramFolder, part: &idea::Part) -> Result<Taken, String> {
    // The part's own path, `..` resolved, as a program folder takes it.
    let joined = folder.path().join(part.folder());
    let part_folder: PathBuf = std::fs::canonicalize(&joined).unwrap_or(joined);
    let shown = part_folder.display().to_string();
    let (name, link) = (&part.name, &part.idea);
    let ready = part_folder
        .to_str()
        .and_then(|path| super::folder_of(path).ok())
        .and_then(|part_folder| state_of(data_dir, &part_folder).ok())
        .is_some_and(|state| state == State::Ready);
    if !ready {
        return Err(format!(
            "the part [{name}]({link}) isn't checked on this computer: call check with path \
             {shown} until it's done, then check this program"
        ));
    }
    let Ok(Some(code)) = crate::state::read_text(&part_folder.join(CODE_FILE)) else {
        return Err(format!(
            "the part [{name}]({link}) has no {CODE_FILE}: {CODE_FILE} takes functions only \
             from parts in WebAssembly; make the part an Attested Computation compiled to \
             {CODE_FILE} and call check with path {shown} until it's done, or write the \
             functions in {CODE_FILE} yourself, without their `;; from:` lines, and drop the \
             part from `- Builds on:`"
        ));
    };
    let Ok(Some(idea_text)) = crate::state::read_text(&part_folder.join(IDEA_FILE)) else {
        return Err(format!(
            "the part [{name}]({link}) has no {IDEA_FILE} lengua can read: call check with \
             path {shown}"
        ));
    };
    let contract = match contract::read(&idea_text) {
        Ok(Some(contract)) => contract,
        Ok(None) => {
            return Err(format!(
                "the part [{name}]({link}) states no contract: its {IDEA_FILE} has no `# {}` \
                 section, which lists each function a program may take, with what it reads and \
                 writes through the addresses it is lent. Add the section to {shown}/{IDEA_FILE} \
                 and call check with path {shown} until it's done; or write the functions in \
                 {CODE_FILE} yourself, without their `;; from:` lines, and drop the part from \
                 `- Builds on:`",
                contract::HEADING
            ));
        }
        Err(_) => {
            return Err(format!(
                "the part [{name}]({link}) has problems in its `# {}` section: call check with \
                 path {shown} and fix them first",
                contract::HEADING
            ));
        }
    };
    let proofs = Key::load(data_dir)
        .ok()
        .flatten()
        .and_then(|key| lock::read(&part_folder, Some(&key)).ok().flatten())
        .and_then(|lock| lock.proofs);
    if let Some(found) = proofs.as_ref().and_then(|proofs| proofs.wrong.first()) {
        return Err(format!(
            "a proof found the part [{name}]({link}) wrong (generated {}: {}): fix the part \
             first, call check with path {shown} until it's done, then copy its functions \
             again",
            found.0, found.1
        ));
    }
    Ok(Taken {
        part: part.clone(),
        shown,
        fields: parts::fields(&code),
        contract,
        idea: trust::idea_hash(idea_text.as_bytes()),
        code: Sha256Hex::of(code.as_bytes()),
        library: parts::library_part(&idea_text, &code).map(|part| part.name),
        proofs,
        functions: Vec::new(),
    })
}

/// Why a run can't go on the parts lengua.lock records, `stamps`, for the program in `folder`:
/// a part whose idea.md or main.wat is no longer what it was when `check` held main.wat to it,
/// or isn't there. `None` when every part is as recorded, and for a program that takes from no
/// part. A run reads each part's two files and nothing else of it.
pub(super) fn changed_since(folder: &ProgramFolder, stamps: &[PartStamp]) -> Option<String> {
    stamps.iter().find_map(|stamp| {
        let part_folder = folder.path().join(&stamp.part);
        let read = |name: &str| {
            crate::state::read_text(&part_folder.join(name))
                .ok()
                .flatten()
        };
        let same = read(IDEA_FILE)
            .is_some_and(|idea| trust::idea_hash(idea.as_bytes()) == stamp.idea)
            && read(CODE_FILE).is_some_and(|code| Sha256Hex::of(code.as_bytes()) == stamp.code);
        (!same).then(|| {
            format!(
                "{CODE_FILE} takes functions from the part {}, and that part isn't what it was \
                 when check held {CODE_FILE} to it, so the program won't run. Call check with \
                 path {folder}: it says whether the part must be checked again and its \
                 functions copied again.",
                stamp.part
            )
        })
    })
}

/// The global `name` among `fields`, `name` without its `$`.
fn global<'f>(fields: &'f [Field], name: &str) -> Option<&'f Field> {
    fields
        .iter()
        .find(|field| field.keyword == "global" && field.name == name)
}

/// The function `name` among `fields`, defined there or imported, `name` without its `$`.
fn function<'f>(fields: &'f [Field], name: &str) -> Option<&'f Field> {
    fields.iter().find(|field| {
        field.name == name
            && (field.keyword == "func"
                || (field.keyword == "import" && field.body.contains("(func ")))
    })
}

/// Whether the global `name` of `fields` is a region's address or its size: `$x` beside
/// `$x.size` or `$x.rosize`, or one of those two.
fn of_a_region(fields: &[Field], name: &str) -> bool {
    name.ends_with(".size")
        || name.ends_with(".rosize")
        || global(fields, &format!("{name}.size")).is_some()
        || global(fields, &format!("{name}.rosize")).is_some()
}

/// Why no program can take the function `taken` of a part whose main.wat's fields are `theirs`:
/// it names a function or a global by its number, a region of the part's, or a global that
/// changes. `None` when a program can.
fn keeps_to_itself(taken: &Field, theirs: &[Field]) -> Option<String> {
    let name = &taken.name;
    if let Some(numbered) = parts::numbered(&taken.body) {
        return Some(format!(
            "`${name}` has `{numbered}`, which names a function or a global by its number: a \
             number means another thing in another program, so name it with `$`"
        ));
    }
    for used in parts::globals(&taken.body) {
        let Some(field) = global(theirs, &used) else {
            continue;
        };
        if of_a_region(theirs, &used) {
            return Some(format!(
                "`${name}` names the region `${used}`: a function a program takes works only \
                 through the addresses its parameters lend it, so pass the region in, as an \
                 address in a parameter"
            ));
        }
        if field.changes() {
            return Some(format!(
                "`${name}` names `${used}`, a global that changes: a function a program takes \
                 keeps nothing between calls but in the memory it is lent"
            ));
        }
    }
    None
}

/// What keeps `mine`, a function of main.wat's that says it comes from the part `source`, from
/// being the part's own: each problem in a line. `fields` are main.wat's, and `from` is what the
/// function's `;; from:` line says.
fn held_to(mine: &Field, from: &str, fields: &[Field], source: &Taken) -> Vec<String> {
    let name = &mine.name;
    let shown = &source.shown;
    let Some(theirs) = source
        .fields
        .iter()
        .find(|field| field.keyword == "func" && !field.imports() && field.name == *name)
    else {
        return vec![format!(
            "{CODE_FILE}'s ${name} says `;; from: {from}`, but {shown}/{CODE_FILE} has no \
             ${name}: take it from the part under the name it has there"
        )];
    };
    if theirs.body != mine.body {
        return vec![format!(
            "{CODE_FILE}'s ${name} differs from {shown}/{CODE_FILE}'s, where it comes from: \
             copy it again word for word, keeping its `;; from: {from}` line"
        )];
    }
    if source.contract.offer(name).is_none() {
        let offered: Vec<String> = source
            .contract
            .offers
            .iter()
            .map(|offer| format!("${}", offer.name))
            .collect();
        return vec![format!(
            "{CODE_FILE}'s ${name} says `;; from: {from}`, but the contract in \
             {shown}/{IDEA_FILE} doesn't offer ${name}: a program takes only the functions a \
             part's `# {}` lists ({})",
            contract::HEADING,
            listed(&offered)
        )];
    }
    if let Some(problem) = keeps_to_itself(theirs, &source.fields) {
        return vec![format!(
            "{CODE_FILE} can't take ${name} from {shown}: {problem}"
        )];
    }
    let mut problems = Vec::new();
    // Each constant the function reads: the program's must be the part's.
    for used in parts::globals(&mine.body) {
        if let Some(constant) = global(&source.fields, &used)
            && global(fields, &used).is_none_or(|own| own.body != constant.body)
        {
            problems.push(format!(
                "{CODE_FILE}'s ${name}, from {from}, reads `${used}`, which {CODE_FILE} must \
                 define as the part does: `{}`",
                constant.body
            ));
        }
    }
    // Each function it calls: the part's own, taken too, or a host function, imported alike.
    // lengua's library's functions are neither's to define.
    for used in parts::calls(&mine.body) {
        if used == *name {
            continue;
        }
        if let Some(called) = function(&source.fields, &used) {
            let own = function(fields, &used);
            if called.imports() {
                // A host function the part imports: the same import, under the same name.
                if own.is_none_or(|own| own.body != called.body) {
                    problems.push(format!(
                        "{CODE_FILE}'s ${name}, from {from}, calls `${used}`, which {CODE_FILE} \
                         must import as the part does: `{}`",
                        called.body
                    ));
                }
            } else {
                // A function of the part's own: taken from the part too.
                let taken_too = own.is_some_and(|own| {
                    own.from
                        .as_deref()
                        .is_some_and(|theirs| idea::folder_of_link(theirs) == source.part.folder())
                });
                if !taken_too {
                    problems.push(format!(
                        "{CODE_FILE}'s ${name}, from {from}, calls `${used}`, the part's own: \
                         copy `${used}` from {shown}/{CODE_FILE} too, with a `;; from: {from}` \
                         line above it"
                    ));
                }
            }
        }
    }
    problems
}

/// The parts main.wat, `code`, builds on, `builds_on` being the parts its idea links to; or
/// what keeps it from building on them, each problem in a line: a part that isn't checked on
/// this computer, has no main.wat, states no contract or was found wrong by a proof; a function
/// that names a part the idea doesn't build on, that the part doesn't have or doesn't offer, or
/// that isn't the part's word for word, with the constants, functions and imports it names; and
/// a part the idea builds on that main.wat takes nothing from.
pub(super) fn taken(
    data_dir: &DataDir,
    folder: &ProgramFolder,
    builds_on: &[idea::Part],
    code: &str,
) -> Result<Vec<Taken>, Vec<String>> {
    let mut problems = Vec::new();
    let fields = parts::fields(code);
    let mut used: Vec<Taken> = Vec::new();
    // The parts that can't be taken from, each said once.
    let mut refused: Vec<&idea::Part> = Vec::new();
    for mine in fields.iter().filter(|field| field.keyword == "func") {
        let Some(from) = &mine.from else {
            continue;
        };
        let wanted = idea::folder_of_link(from);
        let Some(part) = builds_on.iter().find(|part| part.folder() == wanted) else {
            problems.push(format!(
                "{CODE_FILE}'s ${} says `;; from: {from}`, a part {IDEA_FILE} doesn't build \
                 on: add `[{wanted}]({wanted}/idea.md)` to its `- Builds on:` line, or drop the \
                 line",
                mine.name
            ));
            continue;
        };
        if refused.contains(&part) {
            continue;
        }
        let at = match used.iter().position(|taken| taken.part == *part) {
            Some(at) => at,
            None => match open(data_dir, folder, part) {
                Ok(opened) => {
                    used.push(opened);
                    used.len() - 1
                }
                Err(problem) => {
                    problems.push(problem);
                    refused.push(part);
                    continue;
                }
            },
        };
        let Some(source) = used.get_mut(at) else {
            continue;
        };
        problems.extend(held_to(mine, from, &fields, source));
        if !source.functions.contains(&mine.name) {
            source.functions.push(mine.name.clone());
        }
    }
    for part in builds_on {
        if !refused.contains(&part) && !used.iter().any(|taken| taken.part == *part) {
            problems.push(format!(
                "{IDEA_FILE} builds on [{}]({}), but {CODE_FILE} takes nothing from it: copy \
                 each function it uses with a `;; from: {}` line above it, or drop the part from \
                 `- Builds on:`",
                part.name,
                part.idea,
                part.folder()
            ));
        }
    }
    if problems.is_empty() {
        // The parts in the idea's order, whatever main.wat's is.
        used.sort_by_key(|taken| builds_on.iter().position(|part| *part == taken.part));
        Ok(used)
    } else {
        Err(problems)
    }
}

/// What is wrong with a part's own contract, `idea_text` being its idea.md, `code` its main.wat
/// and `lends` what the memory rules found of main.wat's functions: an item lengua can't read,
/// a clause that isn't what main.wat does, and a function no program could take, since it names
/// a region, a global that changes or a function the contract doesn't offer. Each problem names
/// idea.md's line. An idea with no `# Contract` section has none: it is no part.
pub(super) fn contract_problems(idea_text: &str, code: &str, lends: &[Lends]) -> Vec<String> {
    let contract = match contract::read(idea_text) {
        Ok(None) => return Vec::new(),
        Ok(Some(contract)) => contract,
        Err(problems) => {
            return problems
                .iter()
                .map(|problem| format!("{IDEA_FILE} {problem}"))
                .collect();
        }
    };
    let mut problems: Vec<String> = contract
        .problems(lends)
        .iter()
        .map(|problem| format!("{IDEA_FILE} {problem}"))
        .collect();
    let fields = parts::fields(code);
    for offer in &contract.offers {
        // What the item says the function gives, held to the function's text.
        if let Some(Err(problem)) = Claim::of(offer, &fields) {
            problems.push(format!("{IDEA_FILE} line {}: {problem}", offer.line));
        }
        let Some(offered) = fields
            .iter()
            .find(|field| field.keyword == "func" && field.name == offer.name)
        else {
            continue;
        };
        let line = offer.line;
        if offered.imports() || offered.from.is_some() {
            problems.push(format!(
                "{IDEA_FILE} line {line}: `# {}` offers `${}`, which {CODE_FILE} imports or \
                 takes from another part: a part offers only functions it defines itself",
                contract::HEADING,
                offer.name
            ));
            continue;
        }
        if let Some(problem) = keeps_to_itself(offered, &fields) {
            problems.push(format!(
                "{IDEA_FILE} line {line}: no program can take what `# {}` offers here: {problem}",
                contract::HEADING
            ));
            continue;
        }
        for used in parts::calls(&offered.body) {
            let calls_unoffered = used != offer.name
                && contract.offer(&used).is_none()
                && fields
                    .iter()
                    .any(|field| field.keyword == "func" && !field.imports() && field.name == used);
            if calls_unoffered {
                problems.push(format!(
                    "{IDEA_FILE} line {line}: `${}` calls `${used}`, which `# {}` doesn't \
                     offer: a program that takes a function takes what it calls too, so list \
                     `${used}` with its own clauses",
                    offer.name,
                    contract::HEADING
                ));
            }
        }
    }
    problems
}

/// What the contracts of the parts in `taken` say the functions main.wat, `code`, takes from
/// them give, each held to the function's text as main.wat has it, which is the part's word for
/// word: the claims `check` proves of each function alone, and the program's proofs then take
/// at its calls. A function whose item states no formula has none.
pub(super) fn taken_claims(taken: &[Taken], code: &str) -> Vec<Claim> {
    let fields = parts::fields(code);
    let mut claims = Vec::new();
    for source in taken {
        for name in &source.functions {
            let claim = source
                .contract
                .offer(name)
                .and_then(|offer| Claim::of(offer, &fields));
            if let Some(Ok(mut claim)) = claim {
                claim.part = Some(source.part.name.clone());
                claims.push(claim);
            }
        }
    }
    claims
}

/// What a part's own contract says its functions give, `idea_text` being its idea.md and `code`
/// its main.wat: the claims `check` proves of each function alone. None for an idea with no
/// `# Contract` section, and for an item that states no formula.
pub(super) fn own_claims(idea_text: &str, code: &str) -> Vec<Claim> {
    let Ok(Some(contract)) = contract::read(idea_text) else {
        return Vec::new();
    };
    let fields = parts::fields(code);
    contract
        .offers
        .iter()
        .filter_map(|offer| Claim::of(offer, &fields))
        .filter_map(Result::ok)
        .collect()
}

/// A part's contract in lines of `check`'s reply, `idea_text` being its idea.md: each function
/// it offers, with what it does through the addresses it is lent. None for an idea with no
/// `# Contract` section.
pub(super) fn contract_lines(idea_text: &str) -> Vec<String> {
    contract::read(idea_text)
        .ok()
        .flatten()
        .map(|contract| contract.lines())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A part's main.wat: two functions it could offer, one that names a region and one that
    /// names a global that changes.
    const PART: &str = "(module\n  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func \
                        $proc_exit (param i32)))\n  (memory (export \"memory\") 1 1)\n  (global \
                        $TEN i32 (i32.const 10))\n  (global $calls (mut i32) (i32.const 0))\n  \
                        (global $out i32 (i32.const 1024))\n  (global $out.size i32 (i32.const \
                        8))\n  (func $main (export \"_start\")\n    (drop (call $scale \
                        (i32.const 1)))\n    (call $proc_exit (i32.const 0)))\n  (func $scale \
                        (param $n i32) (result i32)\n    (call $times (local.get $n)))\n  (func \
                        $times (param $n i32) (result i32)\n    (i32.mul (local.get $n) \
                        (global.get $TEN)))\n  (func $put (param $n i32)\n    (i32.store \
                        (global.get $out) (local.get $n)))\n  (func $count (result i32)\n    \
                        (global.set $calls (i32.const 1))\n    (global.get $calls)))\n";

    fn contract(items: &str) -> String {
        format!("# Purpose\nText.\n\n# Contract\n{items}\n")
    }

    fn lends() -> Vec<Lends> {
        ["main", "scale", "times", "put", "count"]
            .iter()
            .map(|name| Lends {
                function: (*name).to_owned(),
                borrows: Vec::new(),
            })
            .collect()
    }

    #[test]
    fn a_part_offers_only_what_a_program_can_take() {
        let both = contract("- `$scale`: uses no memory. Ten times.\n- `$times`: uses no memory.");
        assert_eq!(
            contract_problems(&both, PART, &lends()),
            Vec::<String>::new()
        );
        assert_eq!(
            contract_lines(&both),
            ["$scale: uses no memory", "$times: uses no memory"]
        );
        // No section, no part.
        assert_eq!(
            contract_problems("# Purpose\nText.\n", PART, &lends()),
            Vec::<String>::new()
        );
        let problems = |items: &str| contract_problems(&contract(items), PART, &lends()).join("\n");
        assert!(
            problems("- `$scale`: uses no memory.").contains(
                "idea.md line 5: `$scale` calls `$times`, which `# Contract` doesn't offer"
            ),
            "{}",
            problems("- `$scale`: uses no memory.")
        );
        assert!(problems("- `$put`: uses no memory.").contains("names the region `$out`"));
        assert!(problems("- `$count`: uses no memory.").contains("a global that changes"));
        assert!(problems("- `$gone`: uses no memory.").contains("main.wat has no function"));
        assert!(problems("- gone").contains("idea.md line 5: an item of"));
    }

    #[test]
    fn a_taken_function_is_held_to_the_part_with_what_it_names() {
        let idea_text = contract("- `$scale`: uses no memory.\n- `$times`: uses no memory.");
        let source = Taken {
            part: idea::Part {
                name: "numbers".to_owned(),
                idea: "../numbers/idea.md".to_owned(),
            },
            shown: "/bundle/numbers".to_owned(),
            fields: parts::fields(PART),
            contract: contract::read(&idea_text)
                .expect("no problem")
                .expect("a contract"),
            idea: Sha256Hex::of(b"idea"),
            code: Sha256Hex::of(b"code"),
            library: None,
            proofs: None,
            functions: Vec::new(),
        };
        let held = |code: &str| {
            let fields = parts::fields(code);
            fields
                .iter()
                .filter(|field| field.from.is_some())
                .flat_map(|field| held_to(field, "../numbers", &fields, &source))
                .collect::<Vec<String>>()
                .join("\n")
        };
        let times = "  ;; from: ../numbers\n  (func $times (param $n i32) (result i32)\n    \
                     (i32.mul (local.get $n) (global.get $TEN)))\n";
        let scale = "  ;; from: ../numbers\n  (func $scale (param $n i32) (result i32)\n    (call \
                     $times (local.get $n)))\n";
        let whole = format!("(module\n  (global $TEN i32 (i32.const 10))\n{times}{scale})\n");
        assert_eq!(held(&whole), "");
        // The constant it reads, missing or another.
        let other = whole.replace("(i32.const 10))\n", "(i32.const 9))\n");
        assert!(
            held(&other).contains("reads `$TEN`, which main.wat must define as the part does: `(global $TEN i32 (i32.const 10))`"),
            "{}",
            held(&other)
        );
        assert!(held(&format!("(module\n{times})\n")).contains("reads `$TEN`"));
        // The part's function it calls, missing or the program's own.
        let alone = format!("(module\n{scale})\n");
        assert!(
            held(&alone).contains("calls `$times`, the part's own"),
            "{}",
            held(&alone)
        );
        let own = format!(
            "(module\n  (global $TEN i32 (i32.const 10))\n{scale}{})\n",
            times.replace("  ;; from: ../numbers\n", "")
        );
        assert!(
            held(&own).contains("calls `$times`, the part's own"),
            "{}",
            held(&own)
        );
        // A copy that differs, a function the part doesn't have, and one it doesn't offer.
        let differs = whole.replace("i32.mul", "i32.add");
        assert!(held(&differs).contains("differs from /bundle/numbers/main.wat's"));
        let unknown = "(module\n  ;; from: ../numbers\n  (func $half))\n";
        assert!(held(unknown).contains("has no $half"));
        let unoffered = "(module\n  ;; from: ../numbers\n  (func $put (param $n i32)\n    \
                         (i32.store (global.get $out) (local.get $n))))\n";
        assert!(
            held(unoffered).contains("doesn't offer $put"),
            "{}",
            held(unoffered)
        );
    }

    #[test]
    fn a_part_s_line_says_what_is_taken_and_what_is_proven() {
        let mut taken = Taken {
            part: idea::Part {
                name: "decimal".to_owned(),
                idea: "../decimal/idea.md".to_owned(),
            },
            shown: "/bundle/decimal".to_owned(),
            fields: Vec::new(),
            contract: Contract::default(),
            idea: Sha256Hex::of(b"idea"),
            code: Sha256Hex::of(b"code"),
            library: Some("decimal"),
            proofs: None,
            functions: vec!["decimal_parse".to_owned(), "decimal_format".to_owned()],
        };
        let line = taken.line();
        assert!(
            line.starts_with(
                "[decimal](../decimal/idea.md): main.wat takes $decimal_parse and \
                 $decimal_format from it, word for word with the constants each names; lengua's \
                 library part `decimal`, unchanged; its own proofs aren't recorded yet"
            ),
            "{line}"
        );
        taken.library = None;
        taken.functions.pop();
        taken.proofs = Some(Box::new(ProofStamp {
            module_sha256: Sha256Hex::of(b"module"),
            proven: vec!["B1".to_owned(), "B4".to_owned()],
            bounded: vec![lock::Bounded {
                id: "B2".to_owned(),
                bytes: 12,
                of: 4096,
                more: true,
            }],
            solvers: "z3 and cvc5".to_owned(),
            wrong: Vec::new(),
            unproven: vec![("B3".to_owned(), "time".to_owned())],
        }));
        let line = taken.line();
        assert!(
            line.contains(
                "takes $decimal_parse from it, word for word with the constants it names; a \
                 part of this bundle; its own proofs: B1 and B4 proven for every input in \
                 range; B2 proven for every input with text up to 12 of 4096 bytes; B3 tested, \
                 not proven. This program's own tests and proofs go through it again, as it \
                 calls it"
            ),
            "{line}"
        );
        assert_eq!(
            stamps(&[taken]),
            [PartStamp {
                part: "../decimal".to_owned(),
                idea: Sha256Hex::of(b"idea"),
                code: Sha256Hex::of(b"code"),
                functions: vec!["decimal_parse".to_owned()],
            }]
        );
    }
}
