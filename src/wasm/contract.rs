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

use crate::checker::wasm::Lends;
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
    for clause in rest.get(..end).unwrap_or_default().split(';') {
        match lent(clause) {
            Ok(None) => {}
            Ok(Some(one)) => {
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
    Ok(Offer {
        name: name.to_owned(),
        line,
        lent: lends,
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
            .map(|offer| format!("${}: {}", offer.name, offer.clauses()))
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
}
