use crate::Sink;
use crate::behaviours;
use crate::cases::Test;
use crate::constraints::{self, Constraints};
use crate::ste::Dictionary;
use crate::structure;
use crate::text::{Line, Token, is_fence, join, lines, sentences, tokenize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IdeaKind {
    Program,
    Module,
    Screen,
}

pub(crate) const SECTIONS: [&str; 4] = ["Purpose", "Behaviours", "Constraints", "Structure"];

#[derive(Debug, Default)]
pub(crate) struct Idea {
    pub description: String,
    pub constraints: Constraints,
    pub structure: Vec<(usize, String)>,
    pub tests: Vec<Test>,
}

type Section<'l> = Option<(usize, &'l [Line])>;

fn split_sections<'l>(lines: &'l [Line], sink: &mut Sink) -> [Section<'l>; 4] {
    let mut heads: Vec<(usize, Option<usize>)> = Vec::new();
    let mut fence = false;
    for (i, line) in lines.iter().enumerate() {
        if is_fence(&line.text) {
            fence = !fence;
        }
        if let Some(name) = line.text.strip_prefix("# ").filter(|_| !fence) {
            heads.push((i, SECTIONS.iter().position(|s| *s == name.trim())));
        } else if heads.is_empty() && !line.text.trim().is_empty() {
            sink.add(
                line.number,
                "text-outside-section",
                "this text is before the first section; an idea starts with `# Purpose`"
                    .to_string(),
            );
        }
    }
    let mut found: [Section<'l>; 4] = [None; 4];
    let mut last: Option<usize> = None;
    for (k, &(i, which)) in heads.iter().enumerate() {
        let end = heads.get(k + 1).map_or(lines.len(), |h| h.0);
        let number = lines[i].number;
        let Some(which) = which else {
            sink.add(
                number,
                "section-unknown",
                format!(
                    "`{}` is not a section of an idea; the sections are `# Purpose`, `# Behaviours`, `# Constraints` and `# Structure`",
                    lines[i].text.trim()
                ),
            );
            continue;
        };
        if found[which].is_some() {
            sink.add(
                number,
                "section-duplicate",
                format!("the idea has two `# {}` sections", SECTIONS[which]),
            );
            continue;
        }
        if last.is_some_and(|l| l > which) {
            sink.add(
                number,
                "section-order",
                format!(
                    "`# {}` is out of order; the order is `# Purpose`, `# Behaviours`, `# Constraints`, `# Structure`",
                    SECTIONS[which]
                ),
            );
        }
        last = Some(which);
        found[which] = Some((number, &lines[i + 1..end]));
    }
    found
}

fn purpose(heading: usize, lines: &[Line], dictionary: &Dictionary, sink: &mut Sink) -> String {
    let mut units: Vec<Vec<Token>> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    for line in lines {
        let text = line.text.trim();
        if text.starts_with('#') || is_fence(text) {
            sink.add(
                line.number,
                "purpose-text",
                "the purpose is paragraphs of text only, with no headings and no code blocks"
                    .to_string(),
            );
            continue;
        }
        let item = text.strip_prefix("- ").or_else(|| text.strip_prefix("* "));
        if (text.is_empty() || item.is_some()) && !current.is_empty() {
            units.push(std::mem::take(&mut current));
        }
        tokenize(item.unwrap_or(text), line.number, &mut current);
    }
    if !current.is_empty() {
        units.push(current);
    }
    for unit in &units {
        dictionary.paragraph(unit, sink);
    }
    let first = units
        .first()
        .and_then(|unit| sentences(unit).first().map(|s| join(s)));
    if let Some(description) = first {
        description
    } else {
        sink.add(
            heading,
            "purpose-empty",
            "the purpose is empty; write what the program is for".to_string(),
        );
        String::new()
    }
}

pub(crate) fn parse(
    kind: IdeaKind,
    body: &str,
    first_line: usize,
    dictionary: &Dictionary,
    sink: &mut Sink,
) -> Idea {
    let lines = lines(body, first_line);
    let sections = split_sections(&lines, sink);
    let required = [true, true, kind == IdeaKind::Program, true];
    for (i, section) in sections.iter().enumerate() {
        if section.is_none() && required[i] {
            sink.add(
                1,
                "section-missing",
                format!("the idea has no `# {}` section", SECTIONS[i]),
            );
        }
    }
    let mut idea = Idea::default();
    if let Some((heading, lines)) = sections[0] {
        idea.description = purpose(heading, lines, dictionary, sink);
    }
    if let Some((heading, lines)) = sections[1] {
        idea.tests = behaviours::parse(kind, heading, lines, dictionary, sink);
    }
    if let Some((heading, lines)) = sections[2] {
        idea.constraints = constraints::parse(kind, heading, lines, dictionary, sink);
    }
    if let Some((heading, lines)) = sections[3] {
        idea.structure = structure::parse(kind, heading, lines, dictionary, sink);
    }
    idea
}

#[cfg(test)]
mod tests {
    use super::{IdeaKind, parse};
    use crate::Sink;
    use crate::ste::Dictionary;

    fn codes(kind: IdeaKind, body: &str) -> Vec<&'static str> {
        let mut sink = Sink::new("idea.md");
        parse(kind, body, 1, &Dictionary::load(), &mut sink);
        sink.findings.iter().map(|f| f.code).collect()
    }

    const MODULE: &str = "# Purpose\n\nThe module counts the words.\n\n# Behaviours\n\n## Count\n\nGiven a text,\nWhen the program runs,\nThen it writes the number.\n\n### Example: one\n- Standard input: \"a\"\n- Expect exit code: 0\n\n# Structure\n\n- `count`: counts the words in a text.\n";

    #[test]
    fn a_good_module_has_no_findings() {
        assert_eq!(codes(IdeaKind::Module, MODULE), Vec::<&str>::new());
    }

    #[test]
    fn the_purpose_gives_the_description() {
        let mut sink = Sink::new("idea.md");
        let idea = parse(IdeaKind::Module, MODULE, 1, &Dictionary::load(), &mut sink);
        assert_eq!(idea.description, "The module counts the words.");
    }

    #[test]
    fn a_program_needs_constraints() {
        assert_eq!(
            codes(IdeaKind::Program, MODULE),
            ["section-missing", "structure-main-missing"]
        );
    }

    #[test]
    fn sections_out_of_order_unknown_and_twice_are_findings() {
        let body =
            "Hello.\n# Structure\n- `main`: runs.\n# Purpose\nIt runs.\n# Purpose\n# Extra\n";
        let found = codes(IdeaKind::Module, body);
        for code in [
            "text-outside-section",
            "section-order",
            "section-duplicate",
            "section-unknown",
            "section-missing",
        ] {
            assert!(found.contains(&code), "{code} in {found:?}");
        }
    }

    #[test]
    fn a_heading_inside_a_fence_is_not_a_section() {
        let body = MODULE.replace(
            "- Standard input: \"a\"\n",
            "- Standard input:\n  ```\n# Purpose\n  ```\n",
        );
        assert_eq!(codes(IdeaKind::Module, &body), Vec::<&str>::new());
    }
}
