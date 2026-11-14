mod check;
mod mutate;

use lengua_idea::{Finding, WebLintInput, WebLintOutput};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, Expression, Function, Program, Statement,
    VariableDeclaration, VariableDeclarationKind,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

pub use crate::mutate::groups;

pub(crate) struct Lines {
    starts: Vec<usize>,
}

impl Lines {
    pub(crate) fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { starts }
    }

    pub(crate) fn line(&self, offset: u32) -> usize {
        let at = usize::try_from(offset).unwrap_or(usize::MAX);
        self.starts.partition_point(|s| *s <= at).max(1)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Item {
    pub name: String,
    pub span: Span,
    pub asserts: usize,
    pub refs: Vec<String>,
}

pub(crate) fn add(
    findings: &mut Vec<Finding>,
    file: &str,
    line: usize,
    code: &'static str,
    message: String,
) {
    findings.push(Finding {
        file: file.to_string(),
        line,
        code,
        message,
    });
}

fn function_item(function: &Function) -> Option<Item> {
    let name = function.id.as_ref()?.name.as_str().to_string();
    Some(Item {
        name,
        span: function.span,
        asserts: 0,
        refs: Vec::new(),
    })
}

fn variable_items(declaration: &VariableDeclaration, out: &mut Vec<Item>) {
    for declarator in &declaration.declarations {
        let Some(id) = declarator.id.get_binding_identifier() else {
            continue;
        };
        let is_function = matches!(
            declarator.init,
            Some(Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_))
        );
        if is_function {
            out.push(Item {
                name: id.name.as_str().to_string(),
                span: declarator.span,
                asserts: 0,
                refs: Vec::new(),
            });
        }
    }
}

pub(crate) fn top_items(statement: &Statement) -> Vec<Item> {
    let mut out = Vec::new();
    match statement {
        Statement::FunctionDeclaration(f) => out.extend(function_item(f)),
        Statement::VariableDeclaration(d) => variable_items(d, &mut out),
        Statement::ExportDeclaration(e) => match &e.declaration {
            Declaration::FunctionDeclaration(f) => out.extend(function_item(f)),
            Declaration::VariableDeclaration(d) => variable_items(d, &mut out),
            _ => {}
        },
        Statement::ExportDefaultDeclaration(e) => {
            if let ExportDefaultDeclarationKind::FunctionDeclaration(f) = &e.declaration {
                out.extend(function_item(f));
            }
        }
        _ => {}
    }
    out
}

fn global_rules(file: &str, lines: &Lines, program: &Program, findings: &mut Vec<Finding>) {
    for statement in &program.body {
        let declaration = match statement {
            Statement::VariableDeclaration(d) => Some(&**d),
            Statement::ExportDeclaration(e) => match &e.declaration {
                Declaration::VariableDeclaration(d) => Some(&**d),
                _ => None,
            },
            _ => None,
        };
        let Some(declaration) = declaration else {
            continue;
        };
        let line = lines.line(declaration.span.start);
        if !matches!(declaration.kind, VariableDeclarationKind::Const) {
            add(
                findings,
                file,
                line,
                "ts-global",
                "a module-level `let` or `var` is state every render shares; keep state in `useScreen` or in a function".to_string(),
            );
        }
        let made = declaration
            .declarations
            .iter()
            .any(|d| matches!(d.init, Some(Expression::NewExpression(_))));
        if made {
            add(
                findings,
                file,
                line,
                "ts-global",
                "a module-level `new` makes an object every render shares and can change; make it in a function".to_string(),
            );
        }
    }
}

fn comments(file: &str, text: &str, lines: &Lines, program: &Program, findings: &mut Vec<Finding>) {
    for comment in &program.comments {
        let span = comment.span;
        let body = text
            .get(usize::try_from(span.start).unwrap_or(0)..usize::try_from(span.end).unwrap_or(0))
            .unwrap_or("");
        let ignore = [
            "@ts-ignore",
            "@ts-expect-error",
            "@ts-nocheck",
            "eslint-disable",
        ]
        .iter()
        .find(|w| body.contains(**w));
        match ignore {
            Some(word) => add(
                findings,
                file,
                lines.line(span.start),
                "ts-ignore",
                format!("`{word}` turns a check off, and a comment is not allowed in code; fix what the check found"),
            ),
            None => add(
                findings,
                file,
                lines.line(span.start),
                "ts-comment",
                "code files have no comments; the idea is the documentation, so write it in the screen's idea".to_string(),
            ),
        }
    }
}

#[must_use]
pub fn comment_lines(text: &str) -> Vec<usize> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, text, SourceType::tsx()).parse();
    let lines = Lines::new(text);
    parsed
        .program
        .comments
        .iter()
        .map(|c| lines.line(c.span.start))
        .collect()
}

pub(crate) struct Read {
    pub file: String,
    pub items: Vec<Item>,
}

#[must_use]
pub fn lint(input: &WebLintInput) -> WebLintOutput {
    let mut output = WebLintOutput::default();
    let mut reads: Vec<Read> = Vec::new();
    let asyncs = check::async_names(&input.screens, &input.client);
    for screen in &input.screens {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &screen.text, SourceType::tsx()).parse();
        if let Some(first) = parsed.diagnostics.into_iter().next() {
            add(
                &mut output.findings,
                &screen.file,
                1,
                "ts-read",
                format!("lengua could not read the file as TypeScript: {first}"),
            );
            continue;
        }
        let program = parsed.program;
        let lines = Lines::new(&screen.text);
        comments(
            &screen.file,
            &screen.text,
            &lines,
            &program,
            &mut output.findings,
        );
        global_rules(&screen.file, &lines, &program, &mut output.findings);
        let context = check::Context {
            screen,
            lines: &lines,
            packages: &input.packages,
            asyncs: &asyncs,
        };
        let items = check::run(&context, &program, &mut output.findings);
        let calls: Vec<String> = input
            .client
            .iter()
            .filter(|c| items.iter().any(|i| i.refs.contains(c)))
            .cloned()
            .collect();
        output.calls.push((screen.file.clone(), calls));
        check::structure(&context, &items, &mut output.findings);
        reads.push(Read {
            file: screen.file.clone(),
            items,
        });
    }
    check::assertions(&input.screens, &reads, &mut output.findings);
    check::recursion(&reads, &mut output.findings);
    output.findings.sort();
    output
}

pub(crate) fn text_of(text: &str, span: Span) -> &str {
    text.get(usize::try_from(span.start).unwrap_or(0)..usize::try_from(span.end).unwrap_or(0))
        .unwrap_or("")
}

pub(crate) fn span_of<T: GetSpan>(node: &T) -> Span {
    node.span()
}
