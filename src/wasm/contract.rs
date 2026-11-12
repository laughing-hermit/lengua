//! A part's contract: the functions a program may take from it, each with what it does through
//! the addresses it is lent, as the part's idea.md states them under `# Contract`.
//!
//! ```markdown
//! # Contract
//! - `$decimal_parse`: reads 20 bytes at `$at`; writes 8 bytes at `$into`. It gives 0, or 28
//!   when the text is not a number.
//! - `$crc32_start`: uses no memory. It gives the value a CRC-32 starts from.
//! ```
//!
//! An item is a function's name in backticks, a colon, and its clauses up to the first full
//! stop, with `;` between them: `reads N bytes at` a parameter, `writes N bytes at` a parameter
//! (which may read them too), or `uses no memory`. What follows the full stop is the idea's own
//! words: what the function gives, and what its arguments must be. The clauses are what the
//! memory rules find in main.wat ([`crate::checker::wasm::Lends`]), so `check` holds the two to
//! each other ([`Contract::problems`]): a part states exactly how far each function reaches, and
//! a program that takes the function lends it regions with that room, or `check` refuses it.
//!
//! An item may also state what its function gives, as a formula of its arguments, with more
//! clauses before the full stop:
//!
//! ```markdown
//! - `$crc32_update`: reads 4096 bytes at `$at`; takes `$crc` from 0 to 0; takes `$length` from
//!   0 to 256; `$at` holds `$length` bytes; gives `crc32(at)`. Its arguments are …
//! ```
//!
//! `takes` a parameter `from` its least value `to` its most; an address `holds` as many bytes
//! as a parameter says, and the formula names those bytes by the address's parameter; `gives`
//! the formula, in the language of the idea's tests, between backticks. `check` proves the
//! formula of the function, for every argument in the ranges and every byte, and the proofs of
//! a program that takes the function use it in place of the function's body
//! ([`crate::prove::contract`]).

use crate::checker::wasm::Lends;
use crate::expr::{self, Expr};
use crate::idea::heading;

/// The section's name, after `# `.
pub const HEADING: &str = "Contract";

/// How to write an item, for the problems that name one.
const FORM: &str = "write each item as `` - `$name`: reads 20 bytes at `$at`; writes 8 bytes at \
                    `$into`. What it gives. ``, or `` - `$name`: uses no memory. What it gives. ``";

/// What a function does through one address it is lent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lent {
    /// The parameter that holds the address, without the `$`.
    pub param: String,
    /// Whether the function writes through it.
    pub written: bool,
    /// How many bytes from the address it may touch.
    pub bytes: u64,
}

impl Lent {
    /// The clause that states it.
    fn clause(&self) -> String {
        format!(
            "{} {} byte{} at `${}`",
            if self.written { "writes" } else { "reads" },
            self.bytes,
            if self.bytes == 1 { "" } else { "s" },
            self.param
        )
    }
}

/// One function a part offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// Its name, without the `$`.
    pub name: String,
    /// The line of idea.md its item starts on, from 1.
    pub line: usize,
    /// What it does through each address it is lent, in the item's order.
    pub lent: Vec<Lent>,
    /// What it gives, when the item states it.
    pub gives: Option<Gives>,
}

impl Offer {
    /// The item's clauses, as the contract states them.
    #[must_use]
    pub fn clauses(&self) -> String {
        if self.lent.is_empty() {
            "uses no memory".to_owned()
        } else {
            let clauses: Vec<String> = self.lent.iter().map(Lent::clause).collect();
            clauses.join("; ")
        }
    }

    /// The item in one line of a reply: its clauses, and what it gives where it states it.
    #[must_use]
    pub fn line(&self) -> String {
        let clauses = self.clauses();
        match &self.gives {
            None => format!("${}: {clauses}", self.name),
            Some(gives) => format!("${}: {clauses}; {}", self.name, gives.stated()),
        }
    }
}

/// What a function gives, as its item states it: a formula of its arguments, which holds for
/// every argument in the ranges the item takes them in, and every byte it is lent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gives {
    /// The formula, as the item writes it between backticks.
    pub formula: String,
    /// The formula, read.
    pub expr: Expr,
    /// Each parameter the item takes in a range: its name without the `$`, its least value and
    /// its most.
    pub takes: Vec<(String, i128, i128)>,
    /// Each address whose bytes the formula names: the parameter that holds it, and the
    /// parameter that says how many bytes, both without the `$`.
    pub holds: Vec<(String, String)>,
}

impl Gives {
    /// The clauses that state it, as the item writes them.
    #[must_use]
    pub fn stated(&self) -> String {
        let mut clauses: Vec<String> = self
            .takes
            .iter()
            .map(|(name, low, high)| format!("takes `${name}` from {low} to {high}"))
            .collect();
        clauses.extend(
            self.holds
                .iter()
                .map(|(pointer, length)| format!("`${pointer}` holds `${length}` bytes")),
        );
        clauses.push(format!("gives `{}`", self.formula));
        clauses.join("; ")
    }
}

/// One clause of an item.
enum Clause {
    /// What the function does through an address.
    Lent(Lent),
    /// `uses no memory`.
    NoMemory,
    /// `takes` a parameter from its least value to its most.
    Takes(String, i128, i128),
    /// An address `holds` as many bytes as a parameter says.
    Holds(String, String),
    /// `gives` a formula.
    Gives(String),
}

/// The parameter's name that opens `text`, written `` `$name` ``, and what follows it.
fn named(text: &str) -> Option<(&str, &str)> {
    let rest = text.trim_start().strip_prefix("`$")?;
    let (name, rest) = rest.split_once('`')?;
    (!name.is_empty() && !name.contains(char::is_whitespace)).then_some((name, rest))
}

/// The clause `clause`, or a problem's words when lengua can't read it.
fn clause_of(clause: &str) -> Result<Clause, String> {
    let clause = clause.trim();
    let unread = || format!("lengua can't read the clause `{clause}`");
    if let Some(rest) = clause.strip_prefix("gives ") {
        let formula = rest
            .trim()
            .strip_prefix('`')
            .and_then(|rest| rest.strip_suffix('`'))
            .filter(|formula| !formula.is_empty() && !formula.contains('`'))
            .ok_or_else(unread)?;
        return Ok(Clause::Gives(formula.to_owned()));
    }
    if let Some(rest) = clause.strip_prefix("takes ") {
        let (name, rest) = named(rest).ok_or_else(unread)?;
        let rest = rest.trim().strip_prefix("from ").ok_or_else(unread)?;
        let (low, high) = rest.split_once(" to ").ok_or_else(unread)?;
        let low: i128 = low.trim().parse().map_err(|_| unread())?;
        let high: i128 = high.trim().parse().map_err(|_| unread())?;
        return Ok(Clause::Takes(name.to_owned(), low, high));
    }
    if let Some((pointer, rest)) = named(clause)
        && let Some(rest) = rest.trim().strip_prefix("holds ")
    {
        let (length, rest) = named(rest).ok_or_else(unread)?;
        if !matches!(rest.trim(), "bytes" | "byte") {
            return Err(unread());
        }
        return Ok(Clause::Holds(pointer.to_owned(), length.to_owned()));
    }
    lent(clause).map(|found| found.map_or(Clause::NoMemory, Clause::Lent))
}

/// How to write what a function gives, for the problems that name it.
const GIVES_FORM: &str = "write `` - `$name`: reads 64 bytes at `$at`; takes `$length` from 0 to \
                          64; `$at` holds `$length` bytes; gives `crc32(at)`. `` or, for a \
                          function of numbers, `` - `$name`: uses no memory; takes `$n` from 0 to \
                          255; gives `n * 2`. ``";

/// What an item's clauses say its function gives: the formula `formula` with the ranges `takes`
/// and the bytes `holds`, held to the item's memory clauses, `lent`; or what is wrong with them.
fn gives_of(
    formula: Option<String>,
    takes: Vec<(String, i128, i128)>,
    holds: Vec<(String, String)>,
    lent: &[Lent],
) -> Result<Option<Gives>, String> {
    let Some(formula) = formula else {
        if takes.is_empty() && holds.is_empty() {
            return Ok(None);
        }
        return Err(format!(
            "a `takes` or a `holds` clause goes with a `gives` clause, which states the formula \
             they are for: {GIVES_FORM}"
        ));
    };
    let expr = expr::parse(&formula)
        .map_err(|error| format!("lengua can't read the formula `{formula}`: {error}"))?;
    if let Some((name, ..)) = takes.iter().find(|(_, low, high)| low > high) {
        return Err(format!(
            "`${name}` is taken from a value above the one it is taken to"
        ));
    }
    if let Some(written) = lent.iter().find(|lent| lent.written) {
        return Err(format!(
            "a function with a `gives` clause writes nothing, and this one writes at `${}`: the \
             formula states what a function gives back, not what it leaves in memory",
            written.param
        ));
    }
    for (pointer, length) in &holds {
        let Some(read) = lent.iter().find(|lent| lent.param == *pointer) else {
            return Err(format!(
                "`${pointer}` holds bytes, so the item says how far the function reads there: \
                 add `reads N bytes at `${pointer}``"
            ));
        };
        let room = i128::from(read.bytes);
        let fits = takes
            .iter()
            .any(|(name, low, high)| name == length && *low >= 0 && *high <= room);
        if !fits {
            return Err(format!(
                "`${pointer}` holds `${length}` bytes, so the item takes `${length}` from 0 or \
                 more to at most {room}, the bytes the function reads there: add `takes \
                 `${length}` from 0 to {room}`, or a smaller range"
            ));
        }
    }
    if let Some(unheld) = lent
        .iter()
        .find(|lent| !holds.iter().any(|(pointer, _)| *pointer == lent.param))
    {
        return Err(format!(
            "the function reads at `${}`, so the item says which of the bytes there the formula \
             is about: add `` `${}` holds `$length` bytes ``, with the parameter that says how \
             many",
            unheld.param, unheld.param
        ));
    }
    for name in expr::variables_of(&expr) {
        let known = takes.iter().any(|(taken, ..)| *taken == name)
            || holds.iter().any(|(pointer, _)| *pointer == name);
        if !known {
            return Err(format!(
                "the formula `{formula}` names `{name}`, which the item neither takes in a range \
                 nor says holds bytes: {GIVES_FORM}"
            ));
        }
    }
    Ok(Some(Gives {
        formula,
        expr,
        takes,
        holds,
    }))
}

/// A part's contract.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contract {
    /// The functions it offers, in the section's order.
    pub offers: Vec<Offer>,
}

/// The items of idea.md's `# Contract` section, each with its first line's number: a top-level
/// list item and the indented lines that go on with it, as one line. `None` when the idea has no
/// such section.
fn items(idea_text: &str) -> Option<Vec<(usize, String)>> {
    let mut found: Option<Vec<(usize, String)>> = None;
    let mut inside = false;
    let mut fenced = false;
    let mut frontmatter = false;
    for (index, line) in idea_text.lines().enumerate() {
        let number = index + 1;
        // The frontmatter is YAML, where a `#` opens a comment.
        if number == 1 && line.trim_end() == "---" {
            frontmatter = true;
            continue;
        }
        if frontmatter {
            frontmatter = line.trim_end() != "---";
            continue;
        }
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(name) = heading(line, 1) {
            inside = name.eq_ignore_ascii_case(HEADING);
            if inside && found.is_none() {
                found = Some(Vec::new());
            }
            continue;
        }
        if !inside {
            continue;
        }
        let Some(items) = found.as_mut() else {
            continue;
        };
        if let Some(item) = line.strip_prefix("- ") {
            items.push((number, item.trim().to_owned()));
        } else if line.starts_with([' ', '\t'])
            && !line.trim().is_empty()
            && let Some((_, item)) = items.last_mut()
        {
            item.push(' ');
            item.push_str(line.trim());
        }
    }
    found
}

/// The clause `clause` as what a function does through an address, or `None` for `uses no
/// memory`; a problem's words when it is neither.
fn lent(clause: &str) -> Result<Option<Lent>, String> {
    let clause = clause.trim();
    if clause.eq_ignore_ascii_case("uses no memory") {
        return Ok(None);
    }
    let unread = || format!("lengua can't read the clause `{clause}`");
    let (written, rest) = if let Some(rest) = clause.strip_prefix("reads ") {
        (false, rest)
    } else if let Some(rest) = clause.strip_prefix("writes ") {
        (true, rest)
    } else {
        return Err(unread());
    };
    let (count, rest) = rest.split_once(' ').ok_or_else(unread)?;
    let bytes: u64 = count.parse().map_err(|_| unread())?;
    let rest = rest
        .strip_prefix("bytes at ")
        .or_else(|| rest.strip_prefix("byte at "))
        .ok_or_else(unread)?;
    let param = rest
        .trim()
        .strip_prefix("`$")
        .and_then(|rest| rest.strip_suffix('`'))
        .filter(|name| !name.is_empty() && !name.contains(['`', ' ']))
        .ok_or_else(unread)?;
    if bytes == 0 {
        return Err(format!(
            "the clause `{clause}` names no bytes: leave it out, or write `uses no memory`"
        ));
    }
    Ok(Some(Lent {
        param: param.to_owned(),
        written,
        bytes,
    }))
}

/// Where the first sentence of `text` ends: at its first full stop that ends the text or has
/// white space after it. A name such as `$x.size` holds none.
fn sentence_end(text: &str) -> Option<usize> {
    text.match_indices('.').map(|(at, _)| at).find(|at| {
        text.get(at + 1..)
            .is_none_or(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
    })
}

/// One item as a function's offer, or what is wrong with it.
fn offer(line: usize, item: &str) -> Result<Offer, String> {
    let named = item
        .strip_prefix("`$")
        .and_then(|rest| rest.split_once('`'))
        .filter(|(name, _)| !name.is_empty() && !name.contains(char::is_whitespace));
    let Some((name, rest)) = named else {
        return Err(format!(
            "line {line}: an item of `# {HEADING}` starts with a function's name in backticks: \
             {FORM}"
        ));
    };
    let Some(rest) = rest.trim_start().strip_prefix(':') else {
        return Err(format!(
            "line {line}: a colon comes after `${name}`, then what it does through the \
             addresses it is lent: {FORM}"
        ));
    };
    let Some(end) = sentence_end(rest) else {
        return Err(format!(
            "line {line}: `${name}`'s clauses end with a full stop: {FORM}"
        ));
    };
    let mut lends: Vec<Lent> = Vec::new();
    let mut takes: Vec<(String, i128, i128)> = Vec::new();
    let mut holds: Vec<(String, String)> = Vec::new();
    let mut formula: Option<String> = None;
    for clause in rest.get(..end).unwrap_or_default().split(';') {
        match clause_of(clause) {
            Ok(Clause::NoMemory) => {}
            Ok(Clause::Takes(param, low, high)) => {
                if takes.iter().any(|(known, ..)| *known == param) {
                    return Err(format!(
                        "line {line}: `${name}` takes `${param}` twice: write one range"
                    ));
                }
                takes.push((param, low, high));
            }
            Ok(Clause::Holds(pointer, length)) => {
                if holds.iter().any(|(known, _)| *known == pointer) {
                    return Err(format!(
                        "line {line}: `${name}` has two `holds` clauses for `${pointer}`: \
                         write one"
                    ));
                }
                holds.push((pointer, length));
            }
            Ok(Clause::Gives(stated)) => {
                if formula.is_some() {
                    return Err(format!(
                        "line {line}: `${name}` has two `gives` clauses: write one formula"
                    ));
                }
                formula = Some(stated);
            }
            Ok(Clause::Lent(one)) => {
                if lends.iter().any(|known| known.param == one.param) {
                    return Err(format!(
                        "line {line}: `${name}` has two clauses for `${}`: write one, `writes` \
                         when the function writes there at all, with the farthest it reaches",
                        one.param
                    ));
                }
                lends.push(one);
            }
            Err(problem) => return Err(format!("line {line}: `${name}`: {problem}: {FORM}")),
        }
    }
    let gives = gives_of(formula, takes, holds, &lends)
        .map_err(|problem| format!("line {line}: `${name}`: {problem}"))?;
    Ok(Offer {
        name: name.to_owned(),
        line,
        lent: lends,
        gives,
    })
}

/// The contract idea.md states, when it has a `# Contract` section: a part's. Its problems are
/// each on its line of idea.md.
///
/// # Errors
///
/// When the section holds an item lengua can't read, the same function twice, or no function.
pub fn read(idea_text: &str) -> Result<Option<Contract>, Vec<String>> {
    let Some(items) = items(idea_text) else {
        return Ok(None);
    };
    let mut contract = Contract::default();
    let mut problems = Vec::new();
    for (line, item) in &items {
        match offer(*line, item) {
            Ok(offer) => {
                if contract.offers.iter().any(|known| known.name == offer.name) {
                    problems.push(format!(
                        "line {line}: `# {HEADING}` lists `${}` twice",
                        offer.name
                    ));
                } else {
                    contract.offers.push(offer);
                }
            }
            Err(problem) => problems.push(problem),
        }
    }
    if items.is_empty() {
        problems.push(format!(
            "`# {HEADING}` lists no function: {FORM}; or take the section out, when no program \
             builds on this one"
        ));
    }
    if problems.is_empty() {
        Ok(Some(contract))
    } else {
        Err(problems)
    }
}

impl Contract {
    /// The function `name`'s offer, `name` without its `$`.
    #[must_use]
    pub fn offer(&self, name: &str) -> Option<&Offer> {
        self.offers.iter().find(|offer| offer.name == name)
    }

    /// Where the contract and main.wat differ, `lends` being what the memory rules found each
    /// of main.wat's functions does through the addresses it is lent: a function the contract
    /// offers and main.wat doesn't have, an address the rules can't bound, and a clause that
    /// says another parameter, another number of bytes, or reads where main.wat writes.
    #[must_use]
    pub fn problems(&self, lends: &[Lends]) -> Vec<String> {
        let mut problems = Vec::new();
        for offer in &self.offers {
            let name = &offer.name;
            let line = offer.line;
            let Some(found) = lends.iter().find(|lends| lends.function == *name) else {
                problems.push(format!(
                    "line {line}: `# {HEADING}` offers `${name}`, but main.wat has no function \
                     of that name"
                ));
                continue;
            };
            let mut held: Vec<Lent> = Vec::new();
            let mut unbounded = false;
            for (param, written, reach) in &found.borrows {
                match reach {
                    Some(0) => {}
                    Some(bytes) => held.push(Lent {
                        param: param.clone(),
                        written: *written,
                        bytes: *bytes,
                    }),
                    None => {
                        unbounded = true;
                        problems.push(format!(
                            "line {line}: the rules can't say how far main.wat's `${name}` \
                             reaches through `${param}`, so no contract can state it: check \
                             each index against a named constant before the access, and each \
                             length against one before a copy or a call that takes it"
                        ));
                    }
                }
            }
            if unbounded {
                continue;
            }
            let same =
                held.len() == offer.lent.len() && held.iter().all(|lent| offer.lent.contains(lent));
            if !same {
                let rules = Offer {
                    lent: held,
                    ..offer.clone()
                };
                problems.push(format!(
                    "line {line}: `# {HEADING}` says `${name}`: {}, but in main.wat it {}: \
                     write `` `${name}`: {}. `` when main.wat is right, or fix main.wat",
                    offer.clauses(),
                    rules.clauses(),
                    rules.clauses()
                ));
            }
        }
        problems
    }

    /// Each function it offers, in a line: what a reply shows of a part.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.offers
            .iter()
            .map(Offer::line)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An idea's text with a contract of two functions, the first on two lines.
    const IDEA: &str = "---\ntype: Attested Computation\n# Contract\n---\n# Purpose\nText.\n\n# \
                        Contract\nA program takes these.\n\n- `$parse`: reads 20 bytes at `$at`; \
                        writes 8 bytes at `$into`. It\n  gives 0, or 28.\n- `$start`: uses no \
                        memory. It gives -1.\n\n# Constraints\n- Uses: nothing\n";

    fn lends(function: &str, borrows: &[(&str, bool, Option<u64>)]) -> Lends {
        Lends {
            function: function.to_owned(),
            borrows: borrows
                .iter()
                .map(|(param, written, reach)| ((*param).to_owned(), *written, *reach))
                .collect(),
        }
    }

    #[test]
    fn reads_each_function_s_clauses() {
        let contract = read(IDEA).expect("no problem").expect("a contract");
        assert_eq!(contract.offers.len(), 2, "{contract:#?}");
        let parse = contract.offer("parse").expect("parse");
        assert_eq!(parse.line, 11);
        assert_eq!(
            parse.clauses(),
            "reads 20 bytes at `$at`; writes 8 bytes at `$into`"
        );
        assert_eq!(
            contract.offer("start").expect("start").clauses(),
            "uses no memory"
        );
        assert_eq!(
            contract.lines(),
            [
                "$parse: reads 20 bytes at `$at`; writes 8 bytes at `$into`",
                "$start: uses no memory"
            ]
        );
        // An idea with no such section is no part, and the frontmatter's comment is no heading.
        assert_eq!(read("---\n# Contract\n---\n# Purpose\nText.\n"), Ok(None));
    }

    #[test]
    fn an_item_lengua_can_t_read_is_a_problem_on_its_line() {
        let problems = |section: &str| {
            read(&format!("# Contract\n{section}\n"))
                .expect_err("a problem")
                .join("\n")
        };
        assert!(problems("").contains("lists no function"));
        assert!(problems("- parse: reads 2 bytes at `$at`.").contains("line 2: an item of"));
        assert!(problems("- `$parse` reads 2 bytes at `$at`.").contains("a colon comes after"));
        assert!(problems("- `$parse`: reads 2 bytes at `$at`").contains("end with a full stop"));
        assert!(problems("- `$parse`: takes 2 bytes at `$at`.").contains("can't read the clause"));
        assert!(problems("- `$parse`: reads two bytes at `$at`.").contains("can't read the"));
        assert!(problems("- `$parse`: reads 0 bytes at `$at`.").contains("names no bytes"));
        assert!(
            problems("- `$parse`: reads 2 bytes at `$at`; writes 1 byte at `$at`.")
                .contains("two clauses for `$at`")
        );
        assert!(
            problems("- `$parse`: uses no memory.\n- `$parse`: uses no memory.")
                .contains("line 3: `# Contract` lists `$parse` twice")
        );
    }

    #[test]
    fn the_contract_is_held_to_what_the_rules_find() {
        let contract = read(IDEA).expect("no problem").expect("a contract");
        let rules = [
            lends("parse", &[("at", false, Some(20)), ("into", true, Some(8))]),
            lends("start", &[]),
            lends("other", &[("x", true, None)]),
        ];
        assert_eq!(contract.problems(&rules), Vec::<String>::new());
        // A function that reaches further, writes where the contract reads, or isn't there.
        let further = [
            lends("parse", &[("at", false, Some(21)), ("into", true, Some(8))]),
            lends("start", &[]),
        ];
        let problems = contract.problems(&further).join("\n");
        assert!(
            problems.contains("in main.wat it reads 21 bytes at `$at`; writes 8 bytes at `$into`"),
            "{problems}"
        );
        let writes = [
            lends("parse", &[("at", true, Some(20)), ("into", true, Some(8))]),
            lends("start", &[("at", false, Some(1))]),
        ];
        let problems = contract.problems(&writes);
        assert_eq!(problems.len(), 2, "{problems:#?}");
        assert!(
            problems[1].contains(
                "says `$start`: uses no memory, but in main.wat it reads 1 byte at `$at`"
            )
        );
        let unbounded = [lends("parse", &[("at", false, None)])];
        let problems = contract.problems(&unbounded).join("\n");
        assert!(problems.contains("can't say how far"), "{problems}");
        assert!(
            problems.contains("offers `$start`, but main.wat has no function"),
            "{problems}"
        );
    }

    #[test]
    fn reads_what_a_function_gives() {
        let section = "# Contract\n- `$crc`: reads 64 bytes at `$at`; takes `$start` from 0 to 0; \
                       takes `$length` from 0 to 64;\n  `$at` holds `$length` bytes; gives \
                       `crc32(at)`. Its arguments.\n- `$twice`: uses no memory; takes `$n` from \
                       -5 to 5; gives `n * 2`. Twice.\n";
        let contract = read(section).expect("no problem").expect("a contract");
        let crc = contract.offer("crc").expect("crc");
        assert_eq!(crc.clauses(), "reads 64 bytes at `$at`");
        let gives = crc.gives.as_ref().expect("what it gives");
        assert_eq!(gives.formula, "crc32(at)");
        assert_eq!(
            gives.takes,
            [("start".to_owned(), 0, 0), ("length".to_owned(), 0, 64)]
        );
        assert_eq!(gives.holds, [("at".to_owned(), "length".to_owned())]);
        assert_eq!(
            contract.lines(),
            [
                "$crc: reads 64 bytes at `$at`; takes `$start` from 0 to 0; takes `$length` from \
                 0 to 64; `$at` holds `$length` bytes; gives `crc32(at)`",
                "$twice: uses no memory; takes `$n` from -5 to 5; gives `n * 2`"
            ]
        );
        // The memory clauses are still what the rules are held to.
        let rules = [
            lends("crc", &[("at", false, Some(64))]),
            lends("twice", &[]),
        ];
        assert_eq!(contract.problems(&rules), Vec::<String>::new());
    }

    #[test]
    fn what_a_function_gives_is_held_to_its_other_clauses() {
        let problems = |item: &str| {
            read(&format!("# Contract\n- {item}\n"))
                .expect_err("a problem")
                .join("\n")
        };
        // A formula lengua can't read, and one that names what the item doesn't state.
        assert!(
            problems("`$f`: uses no memory; takes `$n` from 0 to 9; gives `n +`.")
                .contains("can't read the formula")
        );
        assert!(
            problems("`$f`: uses no memory; takes `$n` from 0 to 9; gives `n + m`.")
                .contains("names `m`")
        );
        // A range upside down, a range or bytes with no formula, and two formulas.
        assert!(
            problems("`$f`: uses no memory; takes `$n` from 9 to 0; gives `n`.")
                .contains("above the one")
        );
        assert!(
            problems("`$f`: uses no memory; takes `$n` from 0 to 9.")
                .contains("goes with a `gives` clause")
        );
        assert!(
            problems("`$f`: uses no memory; takes `$n` from 0 to 9; gives `n`; gives `n`.")
                .contains("two `gives` clauses")
        );
        // Bytes the item doesn't say the function reads, more bytes than it reads, bytes read
        // that the formula is not about, and a function that writes.
        assert!(
            problems("`$f`: uses no memory; takes `$n` from 0 to 9; `$at` holds `$n` bytes; gives `len(at)`.")
                .contains("how far the function reads there")
        );
        assert!(
            problems("`$f`: reads 8 bytes at `$at`; takes `$n` from 0 to 9; `$at` holds `$n` bytes; gives `len(at)`.")
                .contains("to at most 8")
        );
        assert!(
            problems("`$f`: reads 8 bytes at `$at`; takes `$n` from 0 to 8; gives `n`.")
                .contains("which of the bytes there")
        );
        assert!(
            problems("`$f`: writes 8 bytes at `$at`; takes `$n` from 0 to 8; `$at` holds `$n` bytes; gives `n`.")
                .contains("writes nothing")
        );
    }
}
