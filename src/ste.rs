//! ASD-STE100 Simplified Technical English (STE) in idea files. STE is the controlled English
//! that ASD, the association of Europe's aerospace and defence industries, made for technical
//! text: simple words with one meaning each, short sentences with one idea each, and verbs in
//! the simple tenses, so that a reader for whom English is a second language, and a machine, read
//! a text one way. An idea is a program's source, so its prose is in STE, in the style of
//! behavior-driven development: each behavior a `## B1:` title in the simple present, then its
//! Given/When/Then steps, each a short STE sentence in lengua's step vocabulary.
//!
//! `check` holds an idea to what a computer can see without STE's dictionary, in the prose the
//! idea writes: its frontmatter's `description`, the paragraphs of `# Purpose`, each behavior's
//! title and every step. Code blocks are left out, and a value in backticks is one word.
//!
//! - A sentence has at most [`MAX_WORDS`] words, which are the tokens between spaces, a number
//!   among them. It ends at `.`, `!` or `?` before a space or the line's end, or where its
//!   paragraph, title or step ends.
//! - A paragraph, its lines up to a blank one, has at most [`MAX_SENTENCES`] sentences. Each item
//!   of a list is a paragraph of its own.
//! - A word of 5 letters or more that ends in `ing` is an -ing form, which STE allows only in
//!   technical names. lengua's list of those, and of words that aren't verbs, is `string`,
//!   `strings`, `nothing`, `something`, `anything`, `everything`, `during`, `spring`, `ceiling`,
//!   `morning`, `evening`, `building`, `heading`, `padding`, `encoding`, `encodings`, `rounding`,
//!   `bring`, `thing` and `things`, and `floating` just before `point`.
//! - A step uses none of lengua's old phrasings, such as ``standard input holding `…` ``: each is
//!   refused with the STE phrasing that replaces it, which lengua reads as the old one meant.
//!
//! Every problem starts with `STE:` and says what to write. The rest of STE (one meaning for each
//! word, the same word for the same thing every time, the active voice) is the write-idea
//! skill's to teach. ASD gives the standard away on its website; lengua states its rules in its
//! own words and copies nothing of STE's dictionary.

use std::collections::BTreeSet;

use crate::idea::Token;

/// The most words a sentence may have.
pub const MAX_WORDS: usize = 25;

/// The most sentences a paragraph may have.
pub const MAX_SENTENCES: usize = 6;

/// The words of 5 letters or more that end in `ing` and that an idea may still use: technical
/// names, and words that aren't a verb's -ing form. `floating` may come just before `point`.
const TECHNICAL_NAMES: &str = "string strings nothing something anything everything during \
     spring ceiling morning evening building heading padding encoding encodings rounding bring \
     thing things";

/// What a problem with an -ing form says to do.
const ING_FIX: &str = "STE allows -ing forms only in technical names: say it with a simple verb, \
     as in `it reads` for `reading`";

/// What a value in backticks becomes while STE's checks read a line: one word that is no letter,
/// no space and no stop.
const VALUE: char = '\u{1}';

/// In an old phrasing's marks, a value in backticks.
const ANY_VALUE: &str = "`";

/// One of lengua's old step phrasings, which STE refuses, and the STE phrasing that replaces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OldForm {
    /// The words that mark it in a step, in order, with `ANY_VALUE` for a value.
    marks: &'static [&'static str],
    /// The old phrasing, as a problem names it.
    pub old: &'static str,
    /// The STE phrasing to write instead, which lengua reads as the old one meant.
    pub new: &'static str,
}

impl OldForm {
    /// What to write instead, as a problem says it.
    #[must_use]
    pub fn fix(&self) -> String {
        format!("write `{}`, not `{}`", self.new, self.old)
    }

    /// Whether `tokens` start with its marks.
    fn starts(&self, tokens: &[Token]) -> bool {
        if tokens.len() < self.marks.len() {
            return false;
        }
        for (mark, token) in self.marks.iter().zip(tokens) {
            let same = match token {
                Token::Value(_) => *mark == ANY_VALUE,
                Token::Word(word) => word.as_str() == *mark,
            };
            if !same {
                return false;
            }
        }
        true
    }
}

/// lengua's old step phrasings, the longer first where two start alike.
const OLD_FORMS: &[OldForm] = &[
    OldForm {
        marks: &["standard", "input", "holding"],
        old: "standard input holding `…`",
        new: "standard input that contains `…`",
    },
    OldForm {
        marks: &["file", "named", ANY_VALUE, "holding"],
        old: "a file named `name` holding `…`",
        new: "a file `name` that contains `…`",
    },
    OldForm {
        marks: &["file", ANY_VALUE, "holding"],
        old: "a file `name` holding `…`",
        new: "a file `name` that contains `…`",
    },
    OldForm {
        marks: &["variable", ANY_VALUE, "set", "to"],
        old: "the environment variable `NAME` set to `…`",
        new: "the environment variable `NAME` with the value `…`",
    },
    OldForm {
        marks: &["text", "matching"],
        old: "text matching `rule`",
        new: "text that matches `rule`",
    },
    OldForm {
        marks: &["file", ANY_VALUE, "holds"],
        old: "the file `name` holds `…`",
        new: "the file `name` contains `…`",
    },
    OldForm {
        marks: &["client", "gets", "back"],
        old: "the client gets back `…`",
        new: "the client receives `…`",
    },
    OldForm {
        marks: &["keeps", "serving"],
        old: "it keeps serving",
        new: "it does not stop",
    },
    OldForm {
        marks: &["nothing", "listening"],
        old: "nothing listening on port `…`",
        new: "no server on port `…`",
    },
    OldForm {
        marks: &["followed", "by", "a", "newline"],
        old: "`…` followed by a newline",
        new: "`…` and a newline",
    },
    OldForm {
        marks: &["followed", "by"],
        old: "`…` followed by `…`",
        new: "`…` then `…`",
    },
    OldForm {
        marks: &["repeated", ANY_VALUE, "times"],
        old: "`…` repeated `N` times",
        new: "`…` `N` times",
    },
];

/// The old phrasings in a step's words and values, in order, each once.
#[must_use]
pub fn old_forms(tokens: &[Token]) -> Vec<&'static OldForm> {
    let mut found: Vec<&'static OldForm> = Vec::new();
    let mut at = 0;
    while at < tokens.len() {
        let rest = &tokens[at..];
        match OLD_FORMS.iter().find(|form| form.starts(rest)) {
            Some(form) => {
                if !found.contains(&form) {
                    found.push(form);
                }
                at += form.marks.len();
            }
            None => at += 1,
        }
    }
    found
}

/// What STE's checks find in an idea's prose, as `idea::parse` reads it line by line: the
/// frontmatter's description, the paragraphs of `# Purpose`, each behavior's title and every
/// step.
#[derive(Debug, Default)]
pub struct Prose {
    /// The paragraph or step being read: each of its lines, with its number.
    open: Vec<(usize, String)>,
    /// What the open lines are.
    unit: Unit,
    /// The problems found so far, each with its line.
    found: Vec<(usize, String)>,
}

/// What a piece of prose is, which decides what STE's checks ask of it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Unit {
    /// A paragraph, whose sentences are counted too.
    #[default]
    Paragraph,
    /// A behavior's title.
    Title,
    /// A step, which may use none of lengua's old phrasings.
    Step,
}

impl Prose {
    /// A line of a paragraph: it goes on with the open paragraph, or starts one.
    pub fn paragraph(&mut self, number: usize, text: &str) {
        if self.unit != Unit::Paragraph {
            self.end();
        }
        self.open.push((number, text.to_owned()));
    }

    /// The first line of a step, without its list marker: it ends what was open.
    pub fn step(&mut self, number: usize, text: &str) {
        self.end();
        self.unit = Unit::Step;
        self.open.push((number, text.to_owned()));
    }

    /// A line that goes on with the open step; nothing when no step is open.
    pub fn continuation(&mut self, number: usize, text: &str) {
        if self.unit == Unit::Step {
            self.open.push((number, text.to_owned()));
        }
    }

    /// A behavior's title, the text after `B1:`, which is a line of its own.
    pub fn title(&mut self, number: usize, text: &str) {
        self.end();
        self.check(&[(number, text.to_owned())], Unit::Title);
    }

    /// A text of its own, such as the frontmatter's description: paragraphs, all on `number`.
    pub fn text(&mut self, number: usize, text: &str) {
        self.end();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                self.end();
            } else {
                self.paragraph(number, line);
            }
        }
        self.end();
    }

    /// Ends the open paragraph or step, as a blank line, a heading, a table or a code block does.
    pub fn end(&mut self) {
        let open = std::mem::take(&mut self.open);
        let unit = std::mem::take(&mut self.unit);
        self.check(&open, unit);
    }

    /// Every problem found, each with its line, after the last line.
    #[must_use]
    pub fn finish(mut self) -> Vec<(usize, String)> {
        self.end();
        self.found
    }

    /// Checks one paragraph, title or step, from its lines.
    fn check(&mut self, lines: &[(usize, String)], unit: Unit) {
        let Some(&(first, _)) = lines.first() else {
            return;
        };
        let mut sentences = 0;
        let mut words = 0;
        let mut start = first;
        for (number, text) in lines {
            for word in masked(text).split_whitespace() {
                if words == 0 {
                    start = *number;
                }
                words += 1;
                if word.ends_with(['.', '!', '?']) {
                    self.sentence(start, words);
                    sentences += 1;
                    words = 0;
                }
            }
        }
        if words > 0 {
            self.sentence(start, words);
            sentences += 1;
        }
        if unit == Unit::Paragraph && sentences > MAX_SENTENCES {
            let message = format!(
                "STE: this paragraph has {sentences} sentences, more than the {MAX_SENTENCES} \
                 STE allows: give each topic a paragraph of its own, or make a list"
            );
            self.problem(first, message);
        }
        self.forms(first, lines, unit);
    }

    /// A sentence of `words` words, which starts on `line`.
    fn sentence(&mut self, line: usize, words: usize) {
        if words > MAX_WORDS {
            let message = format!(
                "STE: this sentence has {words} words, more than the {MAX_WORDS} STE allows: put \
                 one idea in each sentence, and make two or more sentences of this one"
            );
            self.problem(line, message);
        }
    }

    /// The -ing forms in a paragraph, title or step, and a step's old phrasings.
    fn forms(&mut self, first: usize, lines: &[(usize, String)], unit: Unit) {
        let mut numbers = Vec::new();
        let mut tokens = Vec::new();
        for (number, text) in lines {
            for token in tokens_of(text) {
                numbers.push(*number);
                tokens.push(token);
            }
        }
        // The -ing word of an old phrasing is that phrasing's problem, not one of its own.
        let mut covered: Vec<&str> = Vec::new();
        if unit == Unit::Step {
            for form in old_forms(&tokens) {
                self.problem(first, format!("STE: {}", form.fix()));
                covered.extend(form.marks.iter().filter(|mark| mark.ends_with("ing")));
            }
        }
        let mut reported = BTreeSet::new();
        for (index, token) in tokens.iter().enumerate() {
            let Token::Word(word) = token else {
                continue;
            };
            let after = tokens.get(index + 1);
            let point = matches!(after, Some(Token::Word(after)) if after == "point");
            let technical = covered.contains(&word.as_str()) || (word == "floating" && point);
            if technical || !ing_form(word) {
                continue;
            }
            if reported.insert((numbers[index], word.as_str())) {
                let message = format!("STE: `{word}` is an -ing form. {ING_FIX}");
                self.problem(numbers[index], message);
            }
        }
    }

    fn problem(&mut self, line: usize, message: String) {
        self.found.push((line, message));
    }
}

/// Whether `word`, in lowercase, is an -ing form STE refuses: 5 letters or more that end in
/// `ing`, and none of lengua's technical names.
fn ing_form(word: &str) -> bool {
    word.ends_with("ing")
        && word.chars().count() >= 5
        && !TECHNICAL_NAMES.split_whitespace().any(|name| name == word)
}

/// The words of `line` as STE's checks read them: each run of letters, in lowercase, and each
/// value in backticks.
fn tokens_of(line: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    for character in masked(line).chars() {
        if character.is_alphabetic() {
            word.extend(character.to_lowercase());
            continue;
        }
        if !word.is_empty() {
            tokens.push(Token::Word(std::mem::take(&mut word)));
        }
        if character == VALUE {
            tokens.push(Token::Value(String::new()));
        }
    }
    if !word.is_empty() {
        tokens.push(Token::Word(word));
    }
    tokens
}

/// `line` with each value in backticks, from a run of backticks to the next run of as many, made
/// one `VALUE`. A run with no such run after it is the line's own text.
fn masked(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let run = after.len() - after.trim_start_matches('`').len();
        let body = &after[run..];
        match exact_run(body, run) {
            Some(end) => {
                out.push(VALUE);
                rest = &body[end + run..];
            }
            None => {
                out.push_str(&after[..run]);
                rest = body;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Where the first run of exactly `run` backticks in `text` starts.
fn exact_run(text: &str, run: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'`' {
            let length = bytes[index..]
                .iter()
                .take_while(|&&byte| byte == b'`')
                .count();
            if length == run {
                return Some(index);
            }
            index += length;
        } else {
            index += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An idea in STE, its steps in lengua's STE phrasings.
    const IDEA: &str = r#"---
type: lengua/program-idea
title: add
description: Adds two whole numbers and prints the sum.
---
# Purpose
Adds two whole numbers between `-1000` and `1000`, which it gets as arguments.
It prints the sum on standard output.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` and a newline
- And it exits with code `0`

### For any two numbers
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` and a newline
- And it exits with code `0`

| a | b |
|---|---|
| `int(-1000, 1000)` | `int(-1000, 1000)` |

## B2: Shows the usage without arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` and a newline to standard error
- And it exits with code `1`

# Constraints
- Uses: nothing
"#;

    /// The paragraph line of [`IDEA`] that tests change, line 8.
    const LINE: &str = "It prints the sum on standard output.";

    /// The STE problems `idea::parse` finds in `text`, each as `line N: STE: …`.
    fn ste(text: &str) -> Vec<String> {
        match crate::idea::parse(text) {
            Ok(_) => Vec::new(),
            Err(problems) => problems
                .iter()
                .filter(|problem| problem.message.starts_with("STE:"))
                .map(ToString::to_string)
                .collect(),
        }
    }

    fn assert_ste(text: &str, needle: &str) {
        let found = ste(text);
        assert!(
            found.iter().any(|problem| problem.contains(needle)),
            "no STE problem containing {needle:?} in {found:#?}"
        );
    }

    fn assert_clean(text: &str) {
        let found = ste(text);
        assert!(found.is_empty(), "{found:#?}");
    }

    /// A sentence of `words` words.
    fn sentence(words: usize) -> String {
        format!("{}end.", "word ".repeat(words - 1))
    }

    #[test]
    fn an_idea_in_ste_has_no_problems() {
        let idea = crate::idea::parse(IDEA).unwrap_or_else(|problems| panic!("{problems:#?}"));
        assert_eq!(idea.behaviors.len(), 2);
        assert!(idea.behaviors.iter().all(|behavior| !behavior.serves));
    }

    /// A sentence has at most 25 words, and a value in backticks is one of them, however many it
    /// holds. Code blocks aren't prose.
    #[test]
    fn sentences_have_at_most_25_words() {
        assert_clean(&IDEA.replace(LINE, &sentence(25)));
        let long = IDEA.replace(LINE, &sentence(26));
        assert_ste(&long, "line 8: STE: this sentence has 26 words");
        let valued = format!("{}`a b c d e f g h i j`.", "word ".repeat(24));
        assert_clean(&IDEA.replace(LINE, &valued));
        // A sentence goes on over the lines of its paragraph, up to its stop.
        let lines = format!("{}\n{}", "word ".repeat(13), sentence(13));
        let wrapped = IDEA.replace(LINE, &lines);
        assert_ste(&wrapped, "line 8: STE: this sentence has 26 words");
        // The description, a behavior's title and a step are sentences too.
        let many = "word ".repeat(26);
        let described = IDEA.replace("prints the sum.", &format!("prints the sum {many}."));
        assert_ste(&described, "line 4: STE: this sentence has 35 words");
        let titled = IDEA.replace("## B1: Adds two numbers", &format!("## B1: {many}"));
        assert_ste(&titled, "line 11: STE: this sentence has 26 words");
        let stepped = IDEA.replace("- Given no arguments", &format!("- Given no {many}"));
        assert_ste(&stepped, "line 28: STE: this sentence has 28 words");
        // A step's next lines are the step's too.
        let continued = IDEA.replace("- Given no arguments", &format!("- Given no\n  {many}"));
        assert_ste(&continued, "line 28: STE: this sentence has 28 words");
        let block = IDEA.replace(LINE, &format!("{LINE}\n\n```text\n{many}\n```"));
        assert_clean(&block);
        // Prose under the generated tests' heading, and in other sections, is the idea's own.
        let heading = format!("### For any\n{many}\n");
        let outline = IDEA.replace("### For any two numbers\n", &heading);
        assert_clean(&outline);
        let notes = format!("{IDEA}\n# Notes\n{many}\n");
        assert_clean(&notes);
    }

    /// A paragraph has at most six sentences, and each item of a list is a paragraph of its own.
    #[test]
    fn paragraphs_have_at_most_six_sentences() {
        assert_clean(&IDEA.replace(LINE, &"It adds. ".repeat(5)));
        let long = IDEA.replace(LINE, &"It adds. ".repeat(6));
        assert_ste(&long, "line 7: STE: this paragraph has 7 sentences");
        let list = format!("{LINE}\n{}", "\n- It adds.".repeat(7));
        assert_clean(&IDEA.replace(LINE, &list));
        let two = format!("{}\n\n{}", "It adds. ".repeat(5), "It adds. ".repeat(6));
        assert_clean(&IDEA.replace(LINE, &two));
        // A step is no paragraph: its sentences only count words.
        let step = format!("- Given {}", "it. ".repeat(7));
        assert_clean(&IDEA.replace("- Given no arguments", &step));
    }

    /// An -ing form is refused in prose, but a technical name passes, and so does a value.
    #[test]
    fn ing_forms_are_refused_but_technical_names() {
        let titled = IDEA.replace("Shows the usage without arguments", "Missing arguments");
        assert_ste(&titled, "line 27: STE: `missing` is an -ing form");
        assert!(ste(&titled)[0].ends_with(ING_FIX));
        let reading = IDEA.replace("which it gets", "reading them");
        assert_ste(&reading, "line 7: STE: `reading` is an -ing form");
        let described = IDEA.replace("and prints the sum.", "and printing the sum.");
        assert_ste(&described, "line 4: STE: `printing` is an -ing form");
        let running = IDEA.replacen("the program runs", "the program is running", 1);
        assert_ste(&running, "line 13: STE: `running` is an -ing form");
        let names = "Nothing in its string has padding, a floating-point encoding or `reading`.";
        assert_clean(&IDEA.replace(LINE, names));
        let short = "It prints a king, a ring and a thing.";
        assert_clean(&IDEA.replace(LINE, short));
        let floating = IDEA.replace(LINE, "It prints a floating sum.");
        assert_ste(&floating, "line 8: STE: `floating` is an -ing form");
        let twice = IDEA.replace(LINE, "It prints, adding and adding again.");
        assert_eq!(ste(&twice).len(), 1, "{:#?}", ste(&twice));
    }

    /// Each of lengua's old step phrasings is refused with the STE phrasing that replaces it, and
    /// its own -ing word is no second problem.
    #[test]
    fn old_step_phrasings_are_refused_with_their_ste_phrasings() {
        let refused = |step: &str, form: usize| {
            let text = IDEA.replacen(
                "- When the program runs\n",
                &format!("- When the program runs\n- And {step}\n"),
                1,
            );
            let found = ste(&text);
            let fix = format!("line 14: STE: {}", OLD_FORMS[form].fix());
            assert!(found.contains(&fix), "{step}: {found:#?}");
            assert!(
                !found.iter().any(|problem| problem.contains("-ing form")),
                "{step}: {found:#?}"
            );
        };
        refused("standard input holding `x`", 0);
        refused("a file named `in.txt` holding `x`", 1);
        refused("a file `in.txt` holding `x`", 2);
        refused("the environment variable `NAME` set to `Ada`", 3);
        refused("it prints text matching `rolled {any(1, 6)}`", 4);
        refused("the file `out.txt` holds text matching `{any(1, 9)}`", 4);
        refused("the file `out.txt` holds `x`", 5);
        refused("the client gets back `x`", 6);
        refused("it keeps serving", 7);
        refused("nothing listening on port `8080`", 8);
        refused("it prints `5` followed by a newline", 9);
        refused("it prints `5` followed by `6`", 10);
        refused("standard input that contains `a` repeated `3` times", 11);
        let fix = OLD_FORMS[0].fix();
        assert_eq!(
            fix,
            "write `standard input that contains `…``, not `standard input holding `…``"
        );
    }

    /// Old phrasings are a step's: in a value, or in the prose of `# Purpose`, they're words.
    #[test]
    fn old_phrasings_count_only_in_steps() {
        let valued = IDEA.replace("prints `5` and", "prints `5 followed by 6` and");
        assert_clean(&valued);
        let prose = IDEA.replace(LINE, "It prints the sum, followed by a newline.");
        assert_clean(&prose);
    }

    #[test]
    fn finds_the_old_forms_in_a_steps_words() {
        let forms = |step: &str| old_forms(&tokens_of(step));
        let newline = forms("it prints `x` followed by a newline");
        assert_eq!(newline, [&OLD_FORMS[9]]);
        let joined = forms("it prints `x` followed by `y`");
        assert_eq!(joined, [&OLD_FORMS[10]]);
        let several = forms("a file named `x` holding `a` repeated `5` times followed by `b`");
        assert_eq!(several, [&OLD_FORMS[1], &OLD_FORMS[11], &OLD_FORMS[10]]);
        assert!(forms("it prints `x` and a newline then `y` `2` times").is_empty());
        assert!(forms("it prints `followed by a newline`").is_empty());
        let twice = forms("it prints `a` followed by `b` followed by `c`");
        assert_eq!(twice, [&OLD_FORMS[10]]);
    }

    #[test]
    fn reads_values_as_one_word() {
        assert_eq!(masked("a `b c` d"), "a \u{1} d");
        assert_eq!(masked("``a`b`` c"), "\u{1} c");
        assert_eq!(masked("an `open one"), "an `open one");
        let word = |text: &str| Token::Word(text.to_owned());
        let value = Token::Value(String::new());
        let expected = [word("it"), word("s"), value, word("a"), word("b")];
        assert_eq!(tokens_of("It's `x`, A-b."), expected);
    }
}
