use lengua_lint::{Edit, Mutant};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BinaryExpression, BooleanLiteral, CallExpression, Expression, IfStatement, LogicalExpression,
    LogicalOperator, NumericLiteral, TSType,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use crate::{Lines, text_of, top_items};

const OPERATORS: [(&str, &str); 15] = [
    ("+", "-"),
    ("-", "+"),
    ("*", "/"),
    ("/", "*"),
    ("%", "*"),
    ("<", "<="),
    ("<=", "<"),
    (">", ">="),
    (">=", ">"),
    ("===", "!=="),
    ("!==", "==="),
    ("==", "!="),
    ("!=", "=="),
    ("&&", "||"),
    ("||", "&&"),
];

struct Site {
    start: usize,
    end: usize,
    from: String,
    to: String,
}

struct Finder<'t> {
    text: &'t str,
    types: usize,
    sites: Vec<Site>,
}

fn offset(value: u32) -> usize {
    usize::try_from(value).unwrap_or(0)
}

impl Finder<'_> {
    fn operator(&mut self, left: u32, right: u32) {
        let (left, right) = (offset(left), offset(right));
        let Some(between) = self.text.get(left..right) else {
            return;
        };
        let written = between.trim();
        let Some((_, to)) = OPERATORS.iter().find(|(from, _)| *from == written) else {
            return;
        };
        let lead = between.len() - between.trim_start().len();
        self.sites.push(Site {
            start: left + lead,
            end: left + lead + written.len(),
            from: written.to_string(),
            to: (*to).to_string(),
        });
    }
}

impl<'a> Visit<'a> for Finder<'_> {
    fn visit_ts_type(&mut self, it: &TSType<'a>) {
        self.types += 1;
        walk::walk_ts_type(self, it);
        self.types -= 1;
    }

    fn visit_binary_expression(&mut self, it: &BinaryExpression<'a>) {
        if self.types == 0 {
            self.operator(it.left.span().end, it.right.span().start);
        }
        walk::walk_binary_expression(self, it);
    }

    fn visit_logical_expression(&mut self, it: &LogicalExpression<'a>) {
        if self.types == 0 && it.operator != LogicalOperator::Coalesce {
            self.operator(it.left.span().end, it.right.span().start);
        }
        walk::walk_logical_expression(self, it);
    }

    fn visit_numeric_literal(&mut self, it: &NumericLiteral<'a>) {
        let written = text_of(self.text, it.span);
        if self.types == 0
            && !written.is_empty()
            && written.chars().all(|c| c.is_ascii_digit())
            && let Ok(value) = written.parse::<u64>()
        {
            self.sites.push(Site {
                start: offset(it.span.start),
                end: offset(it.span.end),
                from: written.to_string(),
                to: value.saturating_add(1).to_string(),
            });
        }
    }

    fn visit_boolean_literal(&mut self, it: &BooleanLiteral) {
        if self.types == 0 {
            let (from, to) = if it.value {
                ("true", "false")
            } else {
                ("false", "true")
            };
            self.sites.push(Site {
                start: offset(it.span.start),
                end: offset(it.span.end),
                from: from.to_string(),
                to: to.to_string(),
            });
        }
    }

    fn visit_if_statement(&mut self, it: &IfStatement<'a>) {
        let span = it.test.span();
        let written = text_of(self.text, span);
        self.sites.push(Site {
            start: offset(span.start),
            end: offset(span.end),
            from: written.to_string(),
            to: format!("!({written})"),
        });
        walk::walk_if_statement(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if matches!(&it.callee, Expression::Identifier(id) if id.name.as_str() == "assert") {
            return;
        }
        walk::walk_call_expression(self, it);
    }
}

#[must_use]
pub fn groups(files: &[(String, String)]) -> (Vec<Vec<Mutant>>, String) {
    let mut groups = Vec::new();
    let mut skeleton = String::new();
    for (file, text) in files {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, text, SourceType::tsx()).parse();
        if !parsed.diagnostics.is_empty() {
            continue;
        }
        let lines = Lines::new(text);
        skeleton.push_str(file);
        skeleton.push('\n');
        for statement in &parsed.program.body {
            let items = top_items(statement);
            let Some(first) = items.first() else {
                skeleton.push_str(text_of(text, statement.span()));
                skeleton.push('\n');
                continue;
            };
            skeleton.push_str(&first.name);
            skeleton.push('\n');
            let mut finder = Finder {
                text,
                types: 0,
                sites: Vec::new(),
            };
            finder.visit_statement(statement);
            let function_text = text_of(text, statement.span()).to_string();
            let mutants: Vec<Mutant> = finder
                .sites
                .into_iter()
                .enumerate()
                .map(|(site, s)| Mutant {
                    file: file.clone(),
                    line: lines.line(u32::try_from(s.start).unwrap_or(0)),
                    function: first.name.clone(),
                    function_text: function_text.clone(),
                    site,
                    from: s.from,
                    to: s.to.clone(),
                    edits: vec![Edit {
                        start: s.start,
                        end: s.end,
                        text: s.to,
                    }],
                })
                .collect();
            groups.push(mutants);
        }
    }
    (groups, skeleton)
}
