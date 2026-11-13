use crate::code::Code;
use crate::lex::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Below,
    Above,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Counter {
    name: String,
    side: Side,
}

pub(crate) struct Problem {
    pub at: usize,
    pub message: String,
}

const ENDLESS: [&str; 5] = ["repeat", "repeat_with", "from_fn", "successors", "cycle"];

const SHAPE: &str = "a `while` or a `loop` needs a counter that steps by 1 to a constant: `while i < LIMIT { ...; i += 1; }`, or `loop { if i >= LIMIT { break; } ...; i += 1; }`, with the step at the loop's top level, no other change to the counter and no `continue` before the step; or use `for` over a range or a collection";

fn constant(code: &Code, from: usize, to: usize) -> Option<String> {
    if to == from + 1 && code.kind(from) == Some(Kind::Integer) {
        return Some(code.text(from).to_string());
    }
    let (segments, end) = code.path(from);
    let last = segments.last()?;
    let upper = last.chars().any(|c| c.is_ascii_uppercase())
        && last
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    (end == to && upper).then(|| last.clone())
}

fn operator(code: &Code, from: usize, to: usize) -> Option<(usize, usize, Side, bool)> {
    let mut found = None;
    let mut at = from;
    while at < to {
        let less = code.is(at, "<");
        let arrow = at > 0 && (code.arrow_close(at) || code.joint(at - 1, "=", ">"));
        if (less || code.is(at, ">")) && !arrow {
            if found.is_some() || code.joint(at, "<", "<") || code.joint(at, ">", ">") {
                return None;
            }
            let inclusive = code.joint(at, code.text(at), "=");
            let side = if less { Side::Below } else { Side::Above };
            found = Some((at, if inclusive { 2 } else { 1 }, side, inclusive));
        }
        at = code.after(at);
    }
    found
}

fn compare(code: &Code, from: usize, to: usize) -> Option<Counter> {
    let (op, width, side, inclusive) = operator(code, from, to)?;
    let left = code
        .name(from)
        .filter(|_| op == from + 1)
        .zip(constant(code, op + width, to))
        .map(|(name, limit)| (name, limit, side));
    let flipped = if side == Side::Below {
        Side::Above
    } else {
        Side::Below
    };
    let right = || {
        code.name(op + width)
            .filter(|_| to == op + width + 1)
            .zip(constant(code, from, op))
            .map(|(name, limit)| (name, limit, flipped))
    };
    let (name, limit, side) = left.or_else(right)?;
    let extreme = (side == Side::Below && limit.ends_with("MAX"))
        || (side == Side::Above && limit.ends_with("MIN"));
    if inclusive && extreme {
        return None;
    }
    Some(Counter {
        name: name.to_string(),
        side,
    })
}

fn one(text: &str) -> bool {
    let cleaned = text.replace('_', "");
    let digits: String = cleaned.chars().take_while(char::is_ascii_digit).collect();
    let rest = &cleaned[digits.len()..];
    digits == "1" && (rest.is_empty() || rest.starts_with('u') || rest.starts_with('i'))
}

fn step(code: &Code, open: usize, close: usize, name: &str, sign: &str) -> Option<usize> {
    let mut at = open + 1;
    let mut start = true;
    while at < close {
        if start
            && code.name(at) == Some(name)
            && code.joint(at + 1, sign, "=")
            && code.kind(at + 3) == Some(Kind::Integer)
            && one(code.text(at + 3))
            && (code.is(at + 4, ";") || at + 4 == close)
        {
            return Some(at);
        }
        let group = code.open(at);
        let semicolon = code.is(at, ";");
        at = code.after(at);
        start = semicolon || group;
    }
    None
}

fn changes(code: &Code, at: usize) -> bool {
    let compound = ["+", "-", "*", "/", "%", "^", "&", "|"]
        .iter()
        .any(|op| code.joint(at + 1, op, "="));
    let shift = (code.joint(at + 1, "<", "<") || code.joint(at + 1, ">", ">"))
        && code.joint(at + 2, code.text(at + 2), "=");
    let borrowed = at >= 2 && code.is(at - 1, "mut") && code.is(at - 2, "&");
    let shadowed = at >= 1
        && (code.is(at - 1, "let")
            || (code.is(at - 1, "mut") && at >= 2 && code.is(at - 2, "let")));
    code.single_eq(at + 1) || compound || shift || borrowed || shadowed
}

fn other_change(code: &Code, open: usize, close: usize, name: &str, step: usize) -> Option<usize> {
    (open + 1..close).find(|at| {
        *at != step
            && code.name(*at) == Some(name)
            && !code.after_dot(*at)
            && !code.after_path(*at)
            && changes(code, *at)
    })
}

fn loop_body(code: &Code, at: usize, to: usize) -> Option<usize> {
    match code.ident(at) {
        Some("loop") => code.is(at + 1, "{").then_some(at + 1),
        Some("while") => code.find(at + 1, to, "{"),
        Some("for") if !code.is(at + 1, "<") => code.find(at + 1, to, "{"),
        _ => None,
    }
}

fn early_continue(
    code: &Code,
    open: usize,
    close: usize,
    label: Option<&str>,
    limit: usize,
) -> Option<usize> {
    let nested: Vec<(usize, usize)> = (open + 1..close)
        .filter_map(|at| loop_body(code, at, close))
        .map(|brace| (brace, code.end(brace)))
        .collect();
    (open + 1..limit.min(close)).find(|at| {
        if !code.is(*at, "continue") {
            return false;
        }
        if code.kind(at + 1) == Some(Kind::Lifetime) {
            return label == Some(code.text(at + 1));
        }
        !nested.iter().any(|(from, to)| from < at && at < to)
    })
}

fn counted(
    code: &Code,
    open: usize,
    counter: &Counter,
    sign: &str,
    label: Option<&str>,
    check: usize,
) -> Result<(), String> {
    let close = code.end(open);
    let name = counter.name.as_str();
    let Some(step) = step(code, open, close, name, sign) else {
        return Err(format!(
            "the counter `{name}` has no step `{name} {sign}= 1;` at the loop's top level; {SHAPE}"
        ));
    };
    if let Some(at) = other_change(code, open, close, name, step) {
        return Err(format!(
            "the counter `{name}` changes at line {} too; {SHAPE}",
            code.line(at)
        ));
    }
    if let Some(at) = early_continue(code, open, close, label, step.max(check)) {
        return Err(format!(
            "the `continue` at line {} can skip the step of `{name}`; {SHAPE}",
            code.line(at)
        ));
    }
    Ok(())
}

fn label(code: &Code, at: usize) -> Option<&str> {
    (at >= 2 && code.is(at - 1, ":") && code.kind(at - 2) == Some(Kind::Lifetime))
        .then(|| code.text(at - 2))
}

fn while_loop(code: &Code, at: usize, to: usize) -> Result<(), String> {
    if code.is(at + 1, "let") {
        return Err(format!("a `while let` has no counter; {SHAPE}"));
    }
    let brace = code
        .find(at + 1, to, "{")
        .ok_or_else(|| format!("a `while` with no body; {SHAPE}"))?;
    if code.split(at + 1, brace, "|", "|").len() > 1 {
        return Err(format!(
            "a `while` whose condition has `||` can go on past its counter; {SHAPE}"
        ));
    }
    let counter = code
        .split(at + 1, brace, "&", "&")
        .into_iter()
        .find_map(|(from, end)| compare(code, from, end))
        .ok_or_else(|| {
            format!("the condition of this `while` compares no counter with a constant; {SHAPE}")
        })?;
    let sign = if counter.side == Side::Below {
        "+"
    } else {
        "-"
    };
    counted(code, brace, &counter, sign, label(code, at), brace)
}

fn exits(code: &Code, open: usize) -> Vec<(Counter, usize)> {
    let close = code.end(open);
    let mut out = Vec::new();
    let mut at = open + 1;
    let mut start = true;
    while at < close {
        if start
            && code.is(at, "if")
            && let Some(block) = code.find(at + 1, close, "{")
            && (code.is(block + 1, "break") || code.is(block + 1, "return"))
            && !code.is(code.after(block), "else")
        {
            for (from, end) in code.split(at + 1, block, "|", "|") {
                if let Some(counter) = compare(code, from, end) {
                    out.push((counter, at));
                }
            }
        }
        let group = code.open(at);
        let semicolon = code.is(at, ";");
        at = code.after(at);
        start = semicolon || group;
    }
    out
}

fn endless_loop(code: &Code, at: usize) -> Result<(), String> {
    let open = at + 1;
    if !code.is(open, "{") {
        return Err(format!("a `loop` with no body; {SHAPE}"));
    }
    let mut last = format!(
        "this `loop` has no `if <counter> >= <constant> {{ break; }}` at its top level; {SHAPE}"
    );
    for (counter, check) in exits(code, open) {
        let sign = if counter.side == Side::Above {
            "+"
        } else {
            "-"
        };
        match counted(code, open, &counter, sign, label(code, at), check) {
            Ok(()) => return Ok(()),
            Err(message) => last = message,
        }
    }
    Err(last)
}

fn open_range(code: &Code, from: usize, to: usize) -> bool {
    (from..to).any(|at| {
        code.joint(at, ".", ".")
            && !code.is(at + 2, "=")
            && !code.is(at + 2, ".")
            && (at + 2 >= to || matches!(code.text(at + 2), ")" | "]" | "," | ";"))
    })
}

fn for_loop(code: &Code, at: usize, to: usize) -> Result<(), String> {
    let Some(inside) = code.find(at + 1, to, "in") else {
        return Ok(());
    };
    let Some(brace) = code.find(inside + 1, to, "{") else {
        return Ok(());
    };
    let endless = (inside + 1..brace).any(|k| code.name(k).is_some_and(|n| ENDLESS.contains(&n)))
        || open_range(code, inside + 1, brace);
    let bounded = (inside + 1..brace)
        .any(|k| code.after_dot(k) && matches!(code.name(k), Some("take" | "zip")));
    if endless && !bounded {
        return Err("this `for` goes over an iterator with no end (an open range `a..`, `repeat`, `repeat_with`, `from_fn`, `successors` or `cycle`); add `.take(n)`, or go over a range with an end or a collection".to_string());
    }
    Ok(())
}

pub(crate) fn check(code: &Code, from: usize, to: usize) -> Vec<Problem> {
    let mut out = Vec::new();
    for at in from..to {
        let result = match code.ident(at) {
            Some("for") if !code.is(at + 1, "<") => for_loop(code, at, to),
            Some("while") => while_loop(code, at, to),
            Some("loop") => endless_loop(code, at),
            _ => continue,
        };
        if let Err(message) = result {
            out.push(Problem { at, message });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{check, one};
    use crate::code::read;

    fn problems(body: &str) -> usize {
        let code = read(body).expect("code");
        check(&code, 0, code.len()).len()
    }

    #[test]
    fn counted_loops_pass() {
        assert_eq!(problems("while i < 10 { work(); i += 1; }"), 0);
        assert_eq!(problems("while LIMIT > i && ok { i += 1 }"), 0);
        assert_eq!(problems("while n > 0 { n -= 1; }"), 0);
        assert_eq!(
            problems("loop { if i >= MAX_ROUNDS { break; } i += 1; }"),
            0
        );
        assert_eq!(
            problems(
                "'outer: loop { if done || i > 9 { return; } i += 1; for x in v { continue 'inner; } }"
            ),
            0
        );
        assert_eq!(problems("while i < 3 { i += 1; if a { continue; } }"), 0);
        assert_eq!(
            problems(
                "for x in 0..n {} for y in v.iter().map(|z| { z }) {} for w in (0..).take(3) {}"
            ),
            0
        );
    }

    #[test]
    fn other_loops_are_problems() {
        assert_eq!(problems("loop { work(); }"), 1);
        assert_eq!(problems("while let Some(x) = v.pop() {}"), 1);
        assert_eq!(problems("while i < 10 {}"), 1);
        assert_eq!(problems("while i < 10 { if a { i += 1; } }"), 1);
        assert_eq!(problems("while i < 10 { i += 1; i = 0; }"), 1);
        assert_eq!(problems("while i < 10 { i += 2; }"), 1);
        assert_eq!(problems("while i < 10 { if a { continue; } i += 1; }"), 1);
        assert_eq!(problems("while i < 10 || a { i += 1; }"), 1);
        assert_eq!(problems("while i <= u32::MAX { i += 1; }"), 1);
        assert_eq!(problems("while i < n { i += 1; }"), 1);
        assert_eq!(problems("while i < 10 { i -= 1; }"), 1);
        assert_eq!(problems("while i < 10 { take(&mut i); i += 1; }"), 1);
        assert_eq!(
            problems(
                "for x in 0.. {} for y in std::iter::repeat(1) {} for z in v.iter().cycle() {}"
            ),
            3
        );
    }

    #[test]
    fn a_step_is_one() {
        assert!(one("1"));
        assert!(one("1_u32"));
        assert!(one("1usize"));
        assert!(!one("10"));
        assert!(!one("1_0"));
        assert!(!one("0x1"));
    }
}
