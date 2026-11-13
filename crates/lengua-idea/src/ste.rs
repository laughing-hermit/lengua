use std::collections::HashMap;

use crate::Sink;
use crate::text::{Kind, Token, sentences};

const WORDS: &str = include_str!("ste-words.txt");
const REPLACE: &str = include_str!("ste-replace.txt");

const NOUN: u16 = 1;
const VERB: u16 = 2;
const ADJECTIVE: u16 = 4;
const ADVERB: u16 = 8;
const PREPOSITION: u16 = 16;
const CONJUNCTION: u16 = 32;
const PRONOUN: u16 = 64;
const DETERMINER: u16 = 128;
const PARTICIPLE: u16 = 256;
const MODAL: u16 = 512;
const NUMBER: u16 = 1024;

pub(crate) const PROCEDURE_WORDS: usize = 20;
pub(crate) const DESCRIPTION_WORDS: usize = 25;
pub(crate) const PARAGRAPH_SENTENCES: usize = 6;
pub(crate) const NOUN_CLUSTER: usize = 3;

const BE: [&str; 8] = ["is", "are", "was", "were", "be", "been", "being", "am"];
const HAVE: [&str; 3] = ["has", "have", "had"];

pub(crate) struct Dictionary {
    words: HashMap<&'static str, u16>,
    replace: HashMap<&'static str, &'static str>,
}

enum Look {
    Found(u16),
    Replace(&'static str),
    Ing,
    Unknown,
}

fn tags(text: &str) -> u16 {
    text.chars()
        .map(|c| match c {
            'n' => NOUN,
            'v' => VERB,
            'a' => ADJECTIVE,
            'r' => ADVERB,
            'p' => PREPOSITION,
            'c' => CONJUNCTION,
            'o' => PRONOUN,
            'd' => DETERMINER,
            'x' => PARTICIPLE,
            'm' => MODAL,
            'q' => NUMBER,
            _ => 0,
        })
        .fold(0, |all, tag| all | tag)
}

fn technical(word: &str, at_start: bool) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return true;
    };
    let inner_capital = chars.any(char::is_uppercase);
    let digits = word.chars().any(|c| c.is_ascii_digit() || c == '_');
    digits || inner_capital || (first.is_uppercase() && !at_start)
}

fn undouble(stem: &str) -> Option<String> {
    let mut chars = stem.chars().rev();
    let last = chars.next()?;
    let before = chars.next()?;
    (last == before && !"aeiou".contains(last))
        .then(|| stem[..stem.len() - last.len_utf8()].to_string())
}

impl Dictionary {
    pub(crate) fn load() -> Self {
        let mut words = HashMap::new();
        for line in WORDS.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(word), Some(kinds)) = (parts.next(), parts.next()) {
                *words.entry(word).or_insert(0) |= tags(kinds);
            }
        }
        let mut replace = HashMap::new();
        for line in REPLACE.lines() {
            if let Some((word, with)) = line.split_once(' ') {
                replace.insert(word, with.trim());
            }
        }
        Self { words, replace }
    }

    fn exact(&self, word: &str) -> Option<u16> {
        self.words.get(word).copied()
    }

    fn inflected(&self, word: &str) -> Option<u16> {
        let plural = [
            word.strip_suffix("ies").map(|s| format!("{s}y")),
            word.strip_suffix("es").map(str::to_string),
            word.strip_suffix('s').map(str::to_string),
        ];
        for stem in plural.into_iter().flatten() {
            if let Some(found) = self.exact(&stem)
                && found & (NOUN | VERB) != 0
            {
                return Some(found & (NOUN | VERB));
            }
        }
        let past = [
            word.strip_suffix("ied").map(|s| format!("{s}y")),
            word.strip_suffix("ed").map(str::to_string),
            word.strip_suffix('d').map(str::to_string),
            word.strip_suffix("ed").and_then(undouble),
        ];
        for stem in past.into_iter().flatten() {
            if self.exact(&stem).is_some_and(|found| found & VERB != 0) {
                return Some(VERB | PARTICIPLE | ADJECTIVE);
            }
        }
        None
    }

    fn verb_stem(&self, stem: &str) -> bool {
        let forms = [
            Some(stem.to_string()),
            Some(format!("{stem}e")),
            undouble(stem),
        ];
        forms
            .into_iter()
            .flatten()
            .any(|form| self.exact(&form).is_some_and(|found| found & VERB != 0))
    }

    fn look(&self, word: &str) -> Look {
        if let Some(with) = self.replace.get(word) {
            return Look::Replace(*with);
        }
        if let Some(found) = self.exact(word).or_else(|| self.inflected(word)) {
            return Look::Found(found);
        }
        if let Some(stem) = word.strip_suffix("ing")
            && self.verb_stem(stem)
        {
            return Look::Ing;
        }
        Look::Unknown
    }

    fn base_verb(&self, word: &str) -> bool {
        self.exact(word).is_some_and(|found| found & VERB != 0)
    }

    fn participle(&self, word: &str) -> bool {
        match self.exact(word) {
            Some(found) => found & PARTICIPLE != 0,
            None => self
                .inflected(word)
                .is_some_and(|found| found & PARTICIPLE != 0),
        }
    }

    fn only_noun(&self, word: &str) -> bool {
        self.exact(word)
            .or_else(|| self.inflected(word))
            .is_some_and(|found| found == NOUN)
    }

    pub(crate) fn words(&self, tokens: &[Token], sink: &mut Sink) {
        for (i, token) in tokens.iter().enumerate() {
            if token.kind != Kind::Word {
                continue;
            }
            for part in token.text.split(['-', '\'', '\u{2019}']) {
                self.word(part, i == 0, token.line, sink);
            }
        }
    }

    fn word(&self, word: &str, at_start: bool, line: usize, sink: &mut Sink) {
        if word.is_empty() || word == "s" || technical(word, at_start) {
            return;
        }
        match self.look(&word.to_lowercase()) {
            Look::Found(_) => {}
            Look::Replace(with) => sink.add(
                line,
                "ste-word-unapproved",
                format!("`{word}` is not an approved word; use `{with}`"),
            ),
            Look::Ing => sink.add(
                line,
                "ste-word-ing",
                format!(
                    "`{word}` is the -ing form of a verb; write the sentence in a simple tense"
                ),
            ),
            Look::Unknown => sink.add(
                line,
                "ste-word",
                format!(
                    "`{word}` is not in lengua's STE dictionary; use an approved word, or write a technical name in backticks"
                ),
            ),
        }
    }

    pub(crate) fn description(&self, tokens: &[Token], sink: &mut Sink) {
        for sentence in sentences(tokens) {
            self.sentence(sentence, sink);
        }
    }

    pub(crate) fn paragraph(&self, tokens: &[Token], sink: &mut Sink) {
        let all = sentences(tokens);
        if all.len() > PARAGRAPH_SENTENCES
            && let Some(first) = tokens.first()
        {
            sink.add(
                first.line,
                "ste-paragraph-length",
                format!(
                    "this paragraph has {} sentences; STE allows {PARAGRAPH_SENTENCES} in a paragraph",
                    all.len()
                ),
            );
        }
        for sentence in all {
            self.sentence(sentence, sink);
        }
    }

    fn sentence(&self, sentence: &[Token], sink: &mut Sink) {
        let counted: Vec<&Token> = sentence.iter().filter(|t| t.counts()).collect();
        let Some(first) = counted.first() else {
            return;
        };
        let procedural = sentence.first().is_some_and(|t| {
            t.kind == Kind::Word && self.base_verb(&t.text.to_lowercase())
        });
        let limit = if procedural {
            PROCEDURE_WORDS
        } else {
            DESCRIPTION_WORDS
        };
        if counted.len() > limit {
            let kind = if procedural {
                "an instruction"
            } else {
                "a description"
            };
            sink.add(
                first.line,
                "ste-sentence-length",
                format!(
                    "this sentence has {} words; STE allows {limit} in {kind}",
                    counted.len()
                ),
            );
        }
        self.words(sentence, sink);
        self.grammar(sentence, procedural, sink);
        self.clusters(sentence, sink);
    }

    fn grammar(&self, sentence: &[Token], procedural: bool, sink: &mut Sink) {
        let words: Vec<(String, usize)> = sentence
            .iter()
            .filter(|t| t.kind == Kind::Word)
            .map(|t| (t.text.to_lowercase(), t.line))
            .collect();
        let at = |i: usize| words.get(i).map_or("", |(w, _)| w.as_str());
        for (i, (word, line)) in words.iter().enumerate() {
            let skip = usize::from(matches!(
                at(i + 1),
                "not" | "also" | "only" | "always" | "never" | "then" | "all" | "now"
            ));
            let next = at(i + 1 + skip);
            if BE.contains(&word.as_str())
                && self.participle(next)
                && (procedural || at(i + 2 + skip) == "by")
            {
                sink.add(
                    *line,
                    "ste-passive",
                    format!(
                        "`{word} {next}` is the passive voice; write the sentence in the active voice"
                    ),
                );
            }
            if HAVE.contains(&word.as_str()) && self.participle(next) {
                sink.add(
                    *line,
                    "ste-tense",
                    format!(
                        "`{word} {next}` is a perfect tense; STE uses the simple present, the simple past and the future"
                    ),
                );
            }
            if procedural && word == "and" && self.base_verb(next) {
                let other = self.exact(next).unwrap_or(0);
                if other & (NOUN | ADJECTIVE | DETERMINER | PRONOUN) == 0 {
                    sink.add(
                        *line,
                        "ste-one-instruction",
                        format!(
                            "this instruction has a second verb, `{next}`; write one instruction in each sentence"
                        ),
                    );
                }
            }
        }
    }

    fn clusters(&self, sentence: &[Token], sink: &mut Sink) {
        let mut run = 0;
        let mut reported = false;
        for token in sentence {
            let noun = token.kind == Kind::Word && self.only_noun(&token.text.to_lowercase());
            run = if noun { run + 1 } else { 0 };
            if run == 0 {
                reported = false;
            }
            if run > NOUN_CLUSTER && !reported {
                reported = true;
                sink.add(
                    token.line,
                    "ste-noun-cluster",
                    format!(
                        "this sentence has {run} nouns in a row; STE allows {NOUN_CLUSTER}; use a preposition, for example `of` or `in`"
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Dictionary;
    use crate::Sink;
    use crate::text::tokenize;

    fn codes(text: &str) -> Vec<&'static str> {
        let dictionary = Dictionary::load();
        let mut tokens = Vec::new();
        tokenize(text, 1, &mut tokens);
        let mut sink = Sink::new("t.md");
        dictionary.paragraph(&tokens, &mut sink);
        sink.findings.iter().map(|f| f.code).collect()
    }

    #[test]
    fn a_plain_description_passes() {
        assert_eq!(
            codes("The program reads the text and writes the number of words."),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn code_numbers_and_names_are_allowed() {
        assert_eq!(
            codes("The program reads `xyzzy` from WebAssembly and writes 42 to the `stdout`."),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn an_unknown_word_is_a_finding() {
        assert_eq!(codes("The program frobnicates the text."), ["ste-word"]);
    }

    #[test]
    fn an_unapproved_word_gives_its_replacement() {
        assert_eq!(
            codes("The program utilize the text."),
            ["ste-word-unapproved"]
        );
    }

    #[test]
    fn an_ing_verb_is_a_finding() {
        assert_eq!(codes("The program is reading the text."), ["ste-word-ing"]);
    }

    #[test]
    fn a_long_description_is_a_finding() {
        let long = "The program reads a word ".repeat(6);
        assert!(codes(&format!("{long}.")).contains(&"ste-sentence-length"));
    }

    #[test]
    fn an_instruction_has_a_lower_limit() {
        let twenty_one = format!("Write {}.", "a word ".repeat(10));
        assert_eq!(codes(&twenty_one), ["ste-sentence-length"]);
        let twenty = format!("Write {} word.", "a word ".repeat(9));
        assert_eq!(codes(&twenty), Vec::<&str>::new());
    }

    #[test]
    fn two_instructions_in_one_sentence_are_a_finding() {
        assert_eq!(
            codes("Read the file and write the text."),
            ["ste-one-instruction"]
        );
    }

    #[test]
    fn the_passive_with_by_is_a_finding() {
        assert_eq!(
            codes("The text is read by the program."),
            ["ste-passive"]
        );
    }

    #[test]
    fn a_perfect_tense_is_a_finding() {
        assert_eq!(codes("The program has written the text."), ["ste-tense"]);
    }

    #[test]
    fn a_long_paragraph_is_a_finding() {
        assert_eq!(
            codes("It reads. It reads. It reads. It reads. It reads. It reads. It reads."),
            ["ste-paragraph-length"]
        );
    }

    #[test]
    fn a_noun_cluster_is_a_finding() {
        assert_eq!(
            codes("The program writes the file folder path name."),
            ["ste-noun-cluster"]
        );
    }
}
