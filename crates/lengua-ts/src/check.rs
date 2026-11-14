use lengua_idea::{Finding, WebSource};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrowFunctionExpression, AssignmentExpression, AssignmentOperator, AssignmentTarget,
    BinaryOperator, CallExpression, Class, Decorator, DoWhileStatement, Expression,
    ExpressionStatement, ForStatement, ForStatementInit, Function, FunctionType,
    IdentifierReference, ImportDeclaration, ImportExpression, JSXAttribute, JSXAttributeName,
    NewExpression, Program, SimpleAssignmentTarget, TSAnyKeyword, TSAsExpression,
    TSNonNullExpression, TSTypeAssertion, UnaryOperator, UpdateOperator, VariableDeclarator,
    WhileStatement,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_syntax::scope::ScopeFlags;

use crate::{Item, Lines, Read, add, span_of, text_of, top_items};

const MAX_LINES: usize = 60;
const FIXED_IMPORTS: [&str; 4] = ["react", "next/link", "@/lib/lengua", "@/lib/utils"];

pub(crate) struct Context<'x> {
    pub screen: &'x WebSource,
    pub lines: &'x Lines,
    pub packages: &'x [String],
    pub asyncs: &'x [String],
}

struct Asyncs {
    names: Vec<String>,
}

impl<'a> Visit<'a> for Asyncs {
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        if it.r#async
            && let Some(id) = &it.id
        {
            self.names.push(id.name.as_str().to_string());
        }
        walk::walk_function(self, it, flags);
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        let is_async = match &it.init {
            Some(Expression::ArrowFunctionExpression(a)) => a.r#async,
            Some(Expression::FunctionExpression(f)) => f.r#async,
            _ => false,
        };
        if is_async && let Some(id) = it.id.get_binding_identifier() {
            self.names.push(id.name.as_str().to_string());
        }
        walk::walk_variable_declarator(self, it);
    }
}

pub(crate) fn async_names(screens: &[WebSource], client: &[String]) -> Vec<String> {
    let mut found = Asyncs {
        names: client.to_vec(),
    };
    found.names.push("fetch".to_string());
    for screen in screens {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &screen.text, SourceType::tsx()).parse();
        found.visit_program(&parsed.program);
    }
    found.names.sort();
    found.names.dedup();
    found.names
}

struct Writes<'n> {
    name: &'n str,
    found: bool,
}

impl<'a> Visit<'a> for Writes<'_> {
    fn visit_simple_assignment_target(&mut self, it: &SimpleAssignmentTarget<'a>) {
        if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = it
            && id.name.as_str() == self.name
        {
            self.found = true;
        }
        walk::walk_simple_assignment_target(self, it);
    }
}

fn inner<'e, 'a>(expression: &'e Expression<'a>) -> &'e Expression<'a> {
    let mut at = expression;
    for _ in 0..64 {
        match at {
            Expression::ParenthesizedExpression(p) => at = &p.expression,
            _ => return at,
        }
    }
    at
}

fn callee_name<'e>(call: &'e CallExpression) -> Option<&'e str> {
    match inner(&call.callee) {
        Expression::Identifier(id) => Some(id.name.as_str()),
        Expression::StaticMemberExpression(m) => Some(m.property.name.as_str()),
        _ => None,
    }
}

fn bound(expression: &Expression) -> bool {
    match inner(expression) {
        Expression::NumericLiteral(_) => true,
        Expression::Identifier(id) => {
            let name = id.name.as_str();
            name.chars().any(|c| c.is_ascii_uppercase())
                && name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        }
        Expression::StaticMemberExpression(m) => m.property.name.as_str() == "length",
        _ => false,
    }
}

fn counter(it: &ForStatement) -> Option<String> {
    let Some(ForStatementInit::VariableDeclaration(declaration)) = &it.init else {
        return None;
    };
    if declaration.declarations.len() != 1 {
        return None;
    }
    let declarator = declaration.declarations.first()?;
    let name = declarator
        .id
        .get_binding_identifier()?
        .name
        .as_str()
        .to_string();
    if !matches!(declarator.init, Some(Expression::NumericLiteral(_))) {
        return None;
    }
    let Some(Expression::BinaryExpression(test)) = it.test.as_ref().map(inner) else {
        return None;
    };
    let compared = matches!(inner(&test.left), Expression::Identifier(id) if id.name.as_str() == name)
        && matches!(
            test.operator,
            BinaryOperator::LessThan | BinaryOperator::LessEqualThan
        )
        && bound(&test.right);
    let stepped = match it.update.as_ref().map(inner) {
        Some(Expression::UpdateExpression(u)) => {
            u.operator == UpdateOperator::Increment
                && matches!(&u.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(id) if id.name.as_str() == name)
        }
        Some(Expression::AssignmentExpression(a)) => {
            a.operator == AssignmentOperator::Addition
                && matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(id) if id.name.as_str() == name)
                && matches!(inner(&a.right), Expression::NumericLiteral(n) if (n.value - 1.0).abs() < f64::EPSILON)
        }
        _ => false,
    };
    (compared && stepped).then_some(name)
}

fn allowed(source: &str, packages: &[String]) -> bool {
    FIXED_IMPORTS.contains(&source)
        || source.starts_with("@/components/ui/")
        || source.starts_with("@/lib/wasm/")
        || source.starts_with("@/screens/")
        || packages
            .iter()
            .any(|p| source == p || source.starts_with(&format!("{p}/")))
}

struct Checker<'c, 'x> {
    context: &'c Context<'x>,
    findings: &'c mut Vec<Finding>,
    depth: usize,
    attribute: usize,
    refs: Vec<String>,
    asserts: usize,
}

impl Checker<'_, '_> {
    fn add(&mut self, offset: u32, code: &'static str, message: &str) {
        let line = self.context.lines.line(offset);
        add(
            self.findings,
            &self.context.screen.file,
            line,
            code,
            message.to_string(),
        );
    }

    fn floating(&self, expression: &Expression) -> bool {
        match inner(expression) {
            Expression::CallExpression(call) => match callee_name(call) {
                Some("catch") | None => false,
                Some("then") => call.arguments.len() < 2,
                Some("finally") => true,
                Some(name) => self.context.asyncs.iter().any(|a| a == name),
            },
            Expression::UnaryExpression(u) => {
                u.operator == UnaryOperator::Void
                    && matches!(inner(&u.argument), Expression::CallExpression(_))
            }
            _ => false,
        }
    }
}

impl<'a> Visit<'a> for Checker<'_, '_> {
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        if self.depth > 0 && it.r#type == FunctionType::FunctionDeclaration {
            self.add(
                it.span.start,
                "ts-nested",
                "a function declared inside a function has no name in `# Structure`; move it to the top of the file and name it there, or make it an arrow function",
            );
        }
        self.depth += 1;
        walk::walk_function(self, it, flags);
        self.depth -= 1;
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        if self.attribute > 0 && it.r#async {
            self.add(
                it.span.start,
                "ts-promise",
                "React drops the promise of an `async` event handler, so its failure goes nowhere; call the work with `.catch(...)` from a plain handler",
            );
        }
        self.depth += 1;
        walk::walk_arrow_function_expression(self, it);
        self.depth -= 1;
    }

    fn visit_ts_any_keyword(&mut self, it: &TSAnyKeyword) {
        self.add(
            it.span.start,
            "ts-any",
            "`any` turns the type checks off; write the type, or `unknown` and check it",
        );
    }

    fn visit_ts_as_expression(&mut self, it: &TSAsExpression<'a>) {
        let written = text_of(&self.context.screen.text, span_of(&it.type_annotation));
        if written.trim() != "const" {
            self.add(
                it.span.start,
                "ts-cast",
                "a cast with `as` tells the compiler a type it can't check; check the value instead (`as const` is allowed)",
            );
        }
        walk::walk_ts_as_expression(self, it);
    }

    fn visit_ts_type_assertion(&mut self, it: &TSTypeAssertion<'a>) {
        self.add(
            it.span.start,
            "ts-cast",
            "a cast tells the compiler a type it can't check; check the value instead",
        );
        walk::walk_ts_type_assertion(self, it);
    }

    fn visit_ts_non_null_expression(&mut self, it: &TSNonNullExpression<'a>) {
        self.add(
            it.span.start,
            "ts-non-null",
            "the non-null `!` says a value is there without a check, as `unwrap` does; check it, or give it a default",
        );
        walk::walk_ts_non_null_expression(self, it);
    }

    fn visit_class(&mut self, it: &Class<'a>) {
        self.add(
            it.span.start,
            "ts-class",
            "a class is not used in a screen; write functions and components, each named in `# Structure`",
        );
        walk::walk_class(self, it);
    }

    fn visit_while_statement(&mut self, it: &WhileStatement<'a>) {
        self.add(
            it.span.start,
            "ts-loop",
            "a `while` loop has no bound; use `for...of`, an array method, or a `for` with a counter against a constant",
        );
        walk::walk_while_statement(self, it);
    }

    fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'a>) {
        self.add(
            it.span.start,
            "ts-loop",
            "a `do...while` loop has no bound; use `for...of`, an array method, or a `for` with a counter against a constant",
        );
        walk::walk_do_while_statement(self, it);
    }

    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        let changed = counter(it).is_none_or(|name| {
            let mut writes = Writes {
                name: &name,
                found: false,
            };
            writes.visit_statement(&it.body);
            writes.found
        });
        if changed {
            self.add(
                it.span.start,
                "ts-loop",
                "a `for` is bounded when its counter starts at a number, is compared with `<` or `<=` to a number, an `UPPER_CASE` constant or a `.length`, steps by `++` or `+= 1`, and is not changed in the body",
            );
        }
        walk::walk_for_statement(self, it);
    }

    fn visit_decorator(&mut self, it: &Decorator<'a>) {
        self.add(
            it.span.start,
            "ts-decorator",
            "a decorator runs code that no idea documents; remove it",
        );
        walk::walk_decorator(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        let name = match inner(&it.callee) {
            Expression::Identifier(id) => Some(id.name.as_str()),
            _ => None,
        };
        match name {
            Some("assert") => self.asserts += 1,
            Some("eval") => self.add(
                it.span.start,
                "ts-eval",
                "`eval` runs text as code; remove it",
            ),
            Some("require") => self.add(
                it.span.start,
                "ts-import",
                "`require` is not used; write an `import` at the top of the file",
            ),
            Some("fetch") => self.add(
                it.span.start,
                "ts-fetch",
                "a screen calls the back end only through the generated functions of `@/lib/lengua`, so that it can't drift from the server",
            ),
            _ => {}
        }
        if let Expression::StaticMemberExpression(m) = inner(&it.callee)
            && m.property.name.as_str() == "write"
            && matches!(inner(&m.object), Expression::Identifier(o) if o.name.as_str() == "document")
        {
            self.add(
                it.span.start,
                "ts-unsafe",
                "`document.write` puts text on the page as code; render it with React",
            );
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        if matches!(inner(&it.callee), Expression::Identifier(id) if id.name.as_str() == "Function")
        {
            self.add(
                it.span.start,
                "ts-eval",
                "`new Function` runs text as code; remove it",
            );
        }
        walk::walk_new_expression(self, it);
    }

    fn visit_import_expression(&mut self, it: &ImportExpression<'a>) {
        self.add(
            it.span.start,
            "ts-import",
            "`import()` loads code at run time; write an `import` at the top of the file",
        );
        walk::walk_import_expression(self, it);
    }

    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        let source = it.source.value.as_str().unwrap_or("");
        if !allowed(source, self.context.packages) {
            let message = format!(
                "`{source}` is not a package the idea declares; screens import `react`, `next/link`, `@/lib/lengua`, `@/components/ui/...`, other screens, and the packages of `- Uses package:`"
            );
            self.add(it.span.start, "ts-import", &message);
        }
        walk::walk_import_declaration(self, it);
    }

    fn visit_jsx_attribute(&mut self, it: &JSXAttribute<'a>) {
        if let JSXAttributeName::Identifier(id) = &it.name
            && id.name.as_str() == "dangerouslySetInnerHTML"
        {
            self.add(
                it.span.start,
                "ts-unsafe",
                "`dangerouslySetInnerHTML` puts text on the page as code; render it with React",
            );
        }
        self.attribute += 1;
        walk::walk_jsx_attribute(self, it);
        self.attribute -= 1;
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        if let AssignmentTarget::StaticMemberExpression(m) = &it.left
            && matches!(m.property.name.as_str(), "innerHTML" | "outerHTML")
        {
            self.add(
                it.span.start,
                "ts-unsafe",
                "setting `innerHTML` or `outerHTML` puts text on the page as code; render it with React",
            );
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_expression_statement(&mut self, it: &ExpressionStatement<'a>) {
        if self.floating(&it.expression) {
            self.add(
                it.span.start,
                "ts-promise",
                "this promise is not handled, so its failure goes nowhere; `await` it, return it, or end it with `.catch(...)`",
            );
        }
        walk::walk_expression_statement(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        self.refs.push(it.name.as_str().to_string());
    }

    fn visit_argument(&mut self, it: &Argument<'a>) {
        walk::walk_argument(self, it);
    }
}

pub(crate) fn run(context: &Context, program: &Program, findings: &mut Vec<Finding>) -> Vec<Item> {
    let mut all = Vec::new();
    for statement in &program.body {
        let mut items = top_items(statement);
        let mut checker = Checker {
            context,
            findings,
            depth: 0,
            attribute: 0,
            refs: Vec::new(),
            asserts: 0,
        };
        checker.visit_statement(statement);
        let (refs, asserts) = (checker.refs, checker.asserts);
        for item in &mut items {
            item.refs.clone_from(&refs);
            item.asserts = asserts;
            let first = context.lines.line(item.span.start);
            let last = context.lines.line(item.span.end);
            if last + 1 - first > MAX_LINES {
                add(
                    findings,
                    &context.screen.file,
                    first,
                    "ts-length",
                    format!(
                        "`{}` has {} lines; a function has {MAX_LINES} lines or fewer, so split it",
                        item.name,
                        last + 1 - first
                    ),
                );
            }
        }
        all.extend(items);
    }
    all
}

pub(crate) fn structure(context: &Context, items: &[Item], findings: &mut Vec<Finding>) {
    let screen = context.screen;
    let names = &screen.structure;
    for item in items {
        if !names.iter().any(|(_, n)| *n == item.name) {
            add(
                findings,
                &screen.file,
                context.lines.line(item.span.start),
                "ts-unnamed",
                format!(
                    "`{}` is not in the `# Structure` of `{}`; name it there with one sentence",
                    item.name, screen.idea
                ),
            );
        }
    }
    for (line, name) in names {
        if !items.iter().any(|i| i.name == *name) {
            add(
                findings,
                &screen.idea,
                *line,
                "rule-12-unknown",
                format!(
                    "`{name}` is in `# Structure`, and `{}` has no function or component of that name",
                    screen.file
                ),
            );
        }
    }
    if !items.iter().any(|i| i.name == screen.main) {
        add(
            findings,
            &screen.file,
            1,
            "ts-main-missing",
            format!(
                "the screen's main component, `{}`, is not in the file; its route shows it, so write it and export it",
                screen.main
            ),
        );
    }
}

pub(crate) fn assertions(screens: &[WebSource], reads: &[Read], findings: &mut Vec<Finding>) {
    let mut counted = 0usize;
    let mut total = 0usize;
    let mut first: Option<(String, String)> = None;
    for read in reads {
        let main = screens
            .iter()
            .find(|s| s.file == read.file)
            .map_or("", |s| s.main.as_str());
        for item in read.items.iter().filter(|i| i.name != main) {
            counted += 1;
            total += item.asserts;
            if item.asserts < 2 && first.is_none() {
                first = Some((read.file.clone(), item.name.clone()));
            }
        }
    }
    if total < 2 * counted
        && let Some((file, name)) = first
    {
        findings.push(Finding {
            file,
            line: 1,
            code: "ts-assertions",
            message: format!(
                "the functions of the screens have {total} calls of `assert` for {counted} functions, fewer than two each on average; `{name}` has fewer than two, so check its inputs and results with `assert(condition, message)` from `@/lib/lengua`"
            ),
        });
    }
}

fn reaches(edges: &[Vec<usize>], from: usize, to: usize) -> bool {
    let mut seen = vec![false; edges.len()];
    let mut stack: Vec<usize> = edges.get(from).cloned().unwrap_or_default();
    for _ in 0..=(edges.len() * edges.len()) {
        let Some(at) = stack.pop() else {
            return false;
        };
        if at == to {
            return true;
        }
        if let Some(slot) = seen.get_mut(at)
            && !*slot
        {
            *slot = true;
            stack.extend(edges.get(at).cloned().unwrap_or_default());
        }
    }
    false
}

pub(crate) fn recursion(reads: &[Read], findings: &mut Vec<Finding>) {
    let nodes: Vec<(&str, &Item)> = reads
        .iter()
        .flat_map(|r| r.items.iter().map(move |i| (r.file.as_str(), i)))
        .collect();
    let edges: Vec<Vec<usize>> = nodes
        .iter()
        .map(|(_, item)| {
            nodes
                .iter()
                .enumerate()
                .filter(|(_, (_, other))| item.refs.contains(&other.name))
                .map(|(k, _)| k)
                .collect()
        })
        .collect();
    for (k, (file, item)) in nodes.iter().enumerate() {
        if reaches(&edges, k, k) {
            findings.push(Finding {
                file: (*file).to_string(),
                line: 1,
                code: "ts-recursion",
                message: format!(
                    "`{}` can reach itself through the functions and components it calls; break the cycle",
                    item.name
                ),
            });
        }
    }
}
