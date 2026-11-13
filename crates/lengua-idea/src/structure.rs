use crate::Sink;
use crate::idea::IdeaKind;
use crate::ste::Dictionary;
use crate::text::{Kind, Line, sentences, snake, tokenize, upper_camel};

pub(crate) fn function_name(name: &str) -> bool {
    match name.split_once("::") {
        Some((owner, function)) => upper_camel(owner) && snake(function),
        None => snake(name),
    }
}

fn entries(lines: &[Line], sink: &mut Sink) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for line in lines {
        let text = line.text.as_str();
        if text.trim().is_empty() {
            continue;
        }
        if let Some(rest) = text.strip_prefix("- ") {
            out.push((line.number, rest.trim().to_string()));
        } else if let Some(last) = out.last_mut().filter(|_| text.starts_with(' ')) {
            last.1.push(' ');
            last.1.push_str(text.trim());
        } else {
            sink.add(
                line.number,
                "structure-text",
                "the structure has only list items, each one ``- `name`: one sentence.``"
                    .to_string(),
            );
        }
    }
    out
}

pub(crate) fn parse(
    kind: IdeaKind,
    heading: usize,
    lines: &[Line],
    dictionary: &Dictionary,
    sink: &mut Sink,
) -> Vec<(usize, String)> {
    let mut names: Vec<(usize, String)> = Vec::new();
    for (line, text) in entries(lines, sink) {
        let parsed = text
            .strip_prefix('`')
            .and_then(|rest| rest.split_once('`'))
            .and_then(|(name, rest)| Some((name, rest.strip_prefix(':')?)));
        let Some((name, sentence)) = parsed else {
            sink.add(
                line,
                "structure-item",
                "write each function as ``- `name`: one sentence.``".to_string(),
            );
            continue;
        };
        if !function_name(name) {
            sink.add(
                line,
                "structure-name",
                format!(
                    "`{name}` is not a function name; write `snake_case` or `Type::snake_case`"
                ),
            );
        }
        if names.iter().any(|(_, n)| n == name) {
            sink.add(
                line,
                "structure-duplicate",
                format!("`{name}` is in the structure two times"),
            );
        }
        names.push((line, name.to_string()));
        let mut tokens = Vec::new();
        tokenize(sentence, line, &mut tokens);
        let count = sentences(&tokens).len();
        if count == 0 {
            sink.add(
                line,
                "structure-sentence-missing",
                format!("`{name}` has no sentence; say what the function does"),
            );
        } else if count > 1 || tokens.last().is_some_and(|t| t.kind != Kind::Stop) {
            sink.add(
                line,
                "structure-one-sentence",
                format!("`{name}` has {count} sentences, or no full stop; write one sentence that ends with a full stop"),
            );
        }
        dictionary.description(&tokens, sink);
    }
    if names.is_empty() {
        sink.add(
            heading,
            "structure-empty",
            "the structure is empty; name every function of the module".to_string(),
        );
    }
    if kind == IdeaKind::Program && !names.iter().any(|(_, n)| n == "main") {
        sink.add(
            heading,
            "structure-main-missing",
            "the top idea's structure has no `main`".to_string(),
        );
    }
    names
}

#[cfg(test)]
mod tests {
    use super::{function_name, parse};
    use crate::Sink;
    use crate::idea::IdeaKind;
    use crate::ste::Dictionary;
    use crate::text::lines;

    fn run(kind: IdeaKind, text: &str) -> (Vec<&'static str>, Vec<String>) {
        let mut sink = Sink::new("idea.md");
        let names = parse(kind, 1, &lines(text, 2), &Dictionary::load(), &mut sink);
        (sink.findings.iter().map(|f| f.code).collect(), names.into_iter().map(|(_, n)| n).collect())
    }

    #[test]
    fn a_good_structure_gives_its_names() {
        let (codes, names) = run(
            IdeaKind::Program,
            "- `main`: reads the text and writes the count.\n- `Counter::add`: adds one word\n  to the count.\n",
        );
        assert_eq!(codes, Vec::<&str>::new());
        assert_eq!(names, ["main", "Counter::add"]);
    }

    #[test]
    fn bad_items_are_findings() {
        let text = "- `Main`: runs.\n- `count`: counts. It stops.\n- `count`:\n- count: no backticks.\nloose\n";
        let (codes, _) = run(IdeaKind::Program, text);
        for code in [
            "structure-name",
            "structure-one-sentence",
            "structure-duplicate",
            "structure-sentence-missing",
            "structure-item",
            "structure-text",
            "structure-main-missing",
        ] {
            assert!(codes.contains(&code), "{code} in {codes:?}");
        }
    }

    #[test]
    fn an_empty_structure_is_a_finding() {
        assert_eq!(run(IdeaKind::Module, "\n").0, ["structure-empty"]);
    }

    #[test]
    fn names_are_functions_or_methods() {
        assert!(function_name("main"));
        assert!(function_name("Cart::add_item"));
        assert!(!function_name("cart::Add"));
        assert!(!function_name("a::b::c"));
    }
}
