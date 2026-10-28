//! The expression language of generated tests.
//!
//! A template is text with `{expression}` placeholders; `{{` and `}}` stand for literal braces.
//! Expressions work on whole numbers (checked 128-bit integers, so overflow is an error rather
//! than a wrong answer), strings and true/false values: `+ - * / %` (division truncates toward
//! zero, like the CPU's `idiv`), unary `-`, comparisons, `&& || !`, parentheses, and the functions
//! `len`, `abs`, `min`, `max`, `if`, `upper`, `lower` and `reverse`.
//!
//! Generators draw each variable's inputs: `int(lo, hi)`, `choice(a, b, …)` and
//! `string(min, max, "chars")`, with literal arguments only.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

/// Values of the variables, by name.
pub type Env = BTreeMap<String, Value>;

/// The longest string a generator may produce.
pub const MAX_STRING_LEN: usize = 1_048_576;

/// Messages show at most this many characters of a string value.
const SHOWN_CHARS: usize = 200;

/// Boundary combinations tried before random values, at most.
const MAX_BOUNDARY_CASES: usize = 64;

/// A value an expression produces.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    /// A whole number.
    Int(i128),
    /// Text.
    Str(String),
    /// True or false, from comparisons.
    Bool(bool),
}

impl Value {
    fn kind(&self) -> &'static str {
        match self {
            Self::Int(_) => "a number",
            Self::Str(_) => "a string",
            Self::Bool(_) => "true/false",
        }
    }

    /// The text a placeholder prints: numbers in decimal, strings as they are.
    ///
    /// # Errors
    ///
    /// True/false values can't be printed.
    pub fn render(&self) -> Result<String, EvalError> {
        match self {
            Self::Int(number) => Ok(number.to_string()),
            Self::Str(text) => Ok(text.clone()),
            Self::Bool(_) => Err(EvalError::Type(
                "a placeholder can't print true or false; use if(…) to choose the text".into(),
            )),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(number) => write!(f, "{number}"),
            // Messages quote text, shortened when it's long.
            Self::Str(text) if text.chars().count() > SHOWN_CHARS => {
                let start: String = text.chars().take(SHOWN_CHARS).collect();
                write!(f, "{start:?}… ({} bytes)", text.len())
            }
            Self::Str(text) => write!(f, "{text:?}"),
            Self::Bool(value) => write!(f, "{value}"),
        }
    }
}

/// Why an expression has no value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// An arithmetic result doesn't fit in 128 bits.
    Overflow(String),
    /// Division or remainder by zero.
    DivideByZero,
    /// An operation got the wrong kind of value.
    Type(String),
    /// A variable that isn't declared.
    UnknownVariable(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow(operation) => write!(f, "{operation} overflows"),
            Self::DivideByZero => f.write_str("divides by zero"),
            Self::Type(message) => f.write_str(message),
            Self::UnknownVariable(name) => write!(f, "uses `{name}`, which isn't declared in vars"),
        }
    }
}

/// A syntax error in a template, expression or generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn error<T>(message: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError(message.into()))
}

/// A parsed expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal value.
    Literal(Value),
    /// A variable.
    Var(String),
    /// Unary minus.
    Neg(Box<Expr>),
    /// Logical not.
    Not(Box<Expr>),
    /// A binary operation.
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    /// A function call.
    Call(Function, Vec<Expr>),
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`, truncating toward zero.
    Div,
    /// `%`, with the sign of the dividend.
    Rem,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`
    And,
    /// `||`
    Or,
}

impl BinaryOp {
    fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Rem => "%",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::And => "&&",
            Self::Or => "||",
        }
    }
}

/// A built-in function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Function {
    /// `len(s)`: length of a string in bytes.
    Len,
    /// `abs(n)`
    Abs,
    /// `min(a, b, …)`
    Min,
    /// `max(a, b, …)`
    Max,
    /// `if(condition, then, else)`, evaluating only the chosen branch.
    If,
    /// `upper(s)`: ASCII uppercase.
    Upper,
    /// `lower(s)`: ASCII lowercase.
    Lower,
    /// `reverse(s)`: characters in reverse order.
    Reverse,
    /// `count(s, part)`: how many times `part` appears in `s`, without overlaps.
    Count,
}

impl Function {
    const ALL: [Self; 9] = [
        Self::Len,
        Self::Abs,
        Self::Min,
        Self::Max,
        Self::If,
        Self::Upper,
        Self::Lower,
        Self::Reverse,
        Self::Count,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Len => "len",
            Self::Abs => "abs",
            Self::Min => "min",
            Self::Max => "max",
            Self::If => "if",
            Self::Upper => "upper",
            Self::Lower => "lower",
            Self::Reverse => "reverse",
            Self::Count => "count",
        }
    }

    /// The smallest and largest number of arguments.
    fn arity(self) -> (usize, usize) {
        match self {
            Self::Min | Self::Max => (2, usize::MAX),
            Self::If => (3, 3),
            Self::Count => (2, 2),
            Self::Len | Self::Abs | Self::Upper | Self::Lower | Self::Reverse => (1, 1),
        }
    }
}

/// Names that can't be used as variables.
const RESERVED: &[&str] = &[
    "true", "false", "len", "abs", "min", "max", "if", "upper", "lower", "reverse", "count", "int",
    "choice", "string",
];

/// Whether `name` can name a variable: lowercase letters, digits and `_`, not starting with a
/// digit, and not a keyword or function name.
#[must_use]
pub fn is_variable_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !RESERVED.contains(&name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Int(i128),
    Str(String),
    Ident(String),
    Open,
    Close,
    Comma,
    Op(&'static str),
}

fn lex(text: &str) -> Result<Vec<Token>, ParseError> {
    const OPERATORS: [&str; 15] = [
        "==", "!=", "<=", ">=", "&&", "||", "+", "-", "*", "/", "%", "<", ">", "!", "=",
    ];
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(next) = rest.chars().next() {
        if next.is_whitespace() {
            rest = &rest[next.len_utf8()..];
        } else if next.is_ascii_digit() {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            let number = rest[..digits]
                .parse()
                .map_err(|_| ParseError(format!("the number {} is too large", &rest[..digits])))?;
            tokens.push(Token::Int(number));
            rest = &rest[digits..];
        } else if next.is_ascii_alphabetic() || next == '_' {
            let length = rest
                .bytes()
                .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                .count();
            tokens.push(Token::Ident(rest[..length].to_owned()));
            rest = &rest[length..];
        } else if next == '"' {
            let (string, after) = lex_string(&rest[1..])?;
            tokens.push(Token::Str(string));
            rest = after;
        } else if let Some(symbol) = ['(', ')', ','].into_iter().find(|symbol| *symbol == next) {
            tokens.push(match symbol {
                '(' => Token::Open,
                ')' => Token::Close,
                _ => Token::Comma,
            });
            rest = &rest[1..];
        } else if let Some(operator) = OPERATORS.into_iter().find(|op| rest.starts_with(op)) {
            if operator == "=" {
                return error("use == to compare");
            }
            tokens.push(Token::Op(operator));
            rest = &rest[operator.len()..];
        } else {
            return error(format!("unexpected `{next}`"));
        }
    }
    Ok(tokens)
}

/// Reads a string literal's body up to its closing quote.
fn lex_string(text: &str) -> Result<(String, &str), ParseError> {
    let mut string = String::new();
    let mut chars = text.char_indices();
    while let Some((index, character)) = chars.next() {
        match character {
            '"' => return Ok((string, &text[index + 1..])),
            '\\' => match chars.next().map(|(_, escaped)| escaped) {
                Some('"') => string.push('"'),
                Some('\\') => string.push('\\'),
                Some('n') => string.push('\n'),
                Some('t') => string.push('\t'),
                Some(other) => {
                    return error(format!(
                        "unknown escape \\{other} in a string; use \\\", \\\\, \\n or \\t"
                    ));
                }
                None => break,
            },
            other => string.push(other),
        }
    }
    error("a string literal is missing its closing \"")
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        self.position += 1;
        token
    }

    fn eat_op(&mut self, operators: &[&'static str]) -> Option<&'static str> {
        match self.peek() {
            Some(Token::Op(op)) if operators.contains(op) => {
                let op = *op;
                self.position += 1;
                Some(op)
            }
            _ => None,
        }
    }

    fn expression(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.and()?;
        while self.eat_op(&["||"]).is_some() {
            left = Expr::Binary(BinaryOp::Or, Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.comparison()?;
        while self.eat_op(&["&&"]).is_some() {
            left = Expr::Binary(BinaryOp::And, Box::new(left), Box::new(self.comparison()?));
        }
        Ok(left)
    }

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        const COMPARISONS: [&str; 6] = ["==", "!=", "<", "<=", ">", ">="];
        let left = self.sum()?;
        let Some(op) = self.eat_op(&COMPARISONS) else {
            return Ok(left);
        };
        let op = match op {
            "==" => BinaryOp::Eq,
            "!=" => BinaryOp::Ne,
            "<" => BinaryOp::Lt,
            "<=" => BinaryOp::Le,
            ">" => BinaryOp::Gt,
            _ => BinaryOp::Ge,
        };
        let right = self.sum()?;
        if self.eat_op(&COMPARISONS).is_some() {
            return error("comparisons don't chain; join them with &&");
        }
        Ok(Expr::Binary(op, Box::new(left), Box::new(right)))
    }

    fn sum(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.product()?;
        while let Some(op) = self.eat_op(&["+", "-"]) {
            let op = if op == "+" {
                BinaryOp::Add
            } else {
                BinaryOp::Sub
            };
            left = Expr::Binary(op, Box::new(left), Box::new(self.product()?));
        }
        Ok(left)
    }

    fn product(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.unary()?;
        while let Some(op) = self.eat_op(&["*", "/", "%"]) {
            let op = match op {
                "*" => BinaryOp::Mul,
                "/" => BinaryOp::Div,
                _ => BinaryOp::Rem,
            };
            left = Expr::Binary(op, Box::new(left), Box::new(self.unary()?));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        match self.eat_op(&["-", "!"]) {
            Some("-") => Ok(Expr::Neg(Box::new(self.unary()?))),
            Some(_) => Ok(Expr::Not(Box::new(self.unary()?))),
            None => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        match self.next() {
            Some(Token::Int(number)) => Ok(Expr::Literal(Value::Int(number))),
            Some(Token::Str(text)) => Ok(Expr::Literal(Value::Str(text))),
            Some(Token::Ident(name)) if name == "true" || name == "false" => {
                Ok(Expr::Literal(Value::Bool(name == "true")))
            }
            Some(Token::Ident(name)) if self.peek() == Some(&Token::Open) => {
                self.position += 1;
                self.call(&name)
            }
            Some(Token::Ident(name)) => Ok(Expr::Var(name)),
            Some(Token::Open) => {
                let inner = self.expression()?;
                match self.next() {
                    Some(Token::Close) => Ok(inner),
                    _ => error("a ( is missing its )"),
                }
            }
            Some(token) => error(format!("unexpected {}", describe(&token))),
            None => error("the expression ends too early"),
        }
    }

    fn call(&mut self, name: &str) -> Result<Expr, ParseError> {
        let Some(function) = Function::ALL
            .into_iter()
            .find(|function| function.name() == name)
        else {
            let names: Vec<&str> = Function::ALL.map(Function::name).to_vec();
            return error(format!(
                "unknown function `{name}`; the functions are {}",
                names.join(", ")
            ));
        };
        let mut arguments = Vec::new();
        if self.peek() == Some(&Token::Close) {
            self.position += 1;
        } else {
            loop {
                arguments.push(self.expression()?);
                match self.next() {
                    Some(Token::Comma) => {}
                    Some(Token::Close) => break,
                    _ => return error(format!("{name}( is missing its )")),
                }
            }
        }
        let (fewest, most) = function.arity();
        if arguments.len() < fewest || arguments.len() > most {
            let expected = match (fewest, most) {
                (1, 1) => "one argument".to_owned(),
                (2, 2) => "two arguments: a string and the part to count".into(),
                (3, 3) => "three arguments: a condition, a then value and an else value".into(),
                _ => "two or more arguments".into(),
            };
            return error(format!("{name} takes {expected}"));
        }
        Ok(Expr::Call(function, arguments))
    }
}

fn describe(token: &Token) -> String {
    match token {
        Token::Int(number) => format!("number {number}"),
        Token::Str(text) => format!("string {text:?}"),
        Token::Ident(name) => format!("name `{name}`"),
        Token::Open => "(".into(),
        Token::Close => ")".into(),
        Token::Comma => ",".into(),
        Token::Op(op) => format!("`{op}`"),
    }
}

/// Parses an expression.
///
/// # Errors
///
/// Explains the syntax error.
pub fn parse(text: &str) -> Result<Expr, ParseError> {
    let mut parser = Parser {
        tokens: lex(text)?,
        position: 0,
    };
    let expr = parser.expression()?;
    match parser.peek() {
        None => Ok(expr),
        Some(token) => error(format!(
            "unexpected {} after the expression",
            describe(token)
        )),
    }
}

fn int(value: Value, operation: &str) -> Result<i128, EvalError> {
    match value {
        Value::Int(number) => Ok(number),
        other => Err(EvalError::Type(format!(
            "{operation} needs a number, not {}",
            other.kind()
        ))),
    }
}

fn boolean(value: Value, operation: &str) -> Result<bool, EvalError> {
    match value {
        Value::Bool(value) => Ok(value),
        other => Err(EvalError::Type(format!(
            "{operation} needs true/false, not {} (compare with ==, <, …)",
            other.kind()
        ))),
    }
}

fn string(value: Value, operation: &str) -> Result<String, EvalError> {
    match value {
        Value::Str(text) => Ok(text),
        other => Err(EvalError::Type(format!(
            "{operation} needs a string, not {}",
            other.kind()
        ))),
    }
}

/// Evaluates an expression.
///
/// # Errors
///
/// Overflow, division by zero, a wrong kind of value or an undeclared variable.
pub fn eval(expr: &Expr, env: &Env) -> Result<Value, EvalError> {
    match expr {
        Expr::Literal(value) => Ok(value.clone()),
        Expr::Var(name) => env
            .get(name)
            .cloned()
            .ok_or_else(|| EvalError::UnknownVariable(name.clone())),
        Expr::Neg(inner) => {
            let number = int(eval(inner, env)?, "-")?;
            number
                .checked_neg()
                .map(Value::Int)
                .ok_or_else(|| EvalError::Overflow(format!("-({number})")))
        }
        Expr::Not(inner) => Ok(Value::Bool(!boolean(eval(inner, env)?, "!")?)),
        Expr::Binary(op, left, right) => binary(*op, left, right, env),
        Expr::Call(function, arguments) => call(*function, arguments, env),
    }
}

fn binary(op: BinaryOp, left: &Expr, right: &Expr, env: &Env) -> Result<Value, EvalError> {
    let symbol = op.symbol();
    match op {
        BinaryOp::And | BinaryOp::Or => {
            let first = boolean(eval(left, env)?, symbol)?;
            if first == (op == BinaryOp::Or) {
                return Ok(Value::Bool(first));
            }
            Ok(Value::Bool(boolean(eval(right, env)?, symbol)?))
        }
        BinaryOp::Eq | BinaryOp::Ne => {
            let (a, b) = (eval(left, env)?, eval(right, env)?);
            if std::mem::discriminant(&a) != std::mem::discriminant(&b) {
                return Err(EvalError::Type(format!(
                    "{symbol} compares {} with {}",
                    a.kind(),
                    b.kind()
                )));
            }
            Ok(Value::Bool((a == b) == (op == BinaryOp::Eq)))
        }
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            let a = int(eval(left, env)?, symbol)?;
            let b = int(eval(right, env)?, symbol)?;
            Ok(Value::Bool(match op {
                BinaryOp::Lt => a < b,
                BinaryOp::Le => a <= b,
                BinaryOp::Gt => a > b,
                _ => a >= b,
            }))
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
            let a = int(eval(left, env)?, symbol)?;
            let b = int(eval(right, env)?, symbol)?;
            if matches!(op, BinaryOp::Div | BinaryOp::Rem) && b == 0 {
                return Err(EvalError::DivideByZero);
            }
            let result = match op {
                BinaryOp::Add => a.checked_add(b),
                BinaryOp::Sub => a.checked_sub(b),
                BinaryOp::Mul => a.checked_mul(b),
                BinaryOp::Div => a.checked_div(b),
                _ => a.checked_rem(b),
            };
            result
                .map(Value::Int)
                .ok_or_else(|| EvalError::Overflow(format!("{a} {symbol} {b}")))
        }
    }
}

fn call(function: Function, arguments: &[Expr], env: &Env) -> Result<Value, EvalError> {
    let name = function.name();
    let argument = |index: usize| eval(&arguments[index], env);
    match function {
        Function::If => {
            let branch = if boolean(argument(0)?, "if")? { 1 } else { 2 };
            argument(branch)
        }
        Function::Len => {
            let text = string(argument(0)?, name)?;
            Ok(Value::Int(i128::try_from(text.len()).unwrap_or(i128::MAX)))
        }
        Function::Abs => {
            let number = int(argument(0)?, name)?;
            number
                .checked_abs()
                .map(Value::Int)
                .ok_or_else(|| EvalError::Overflow(format!("abs({number})")))
        }
        Function::Min | Function::Max => {
            let mut best = int(argument(0)?, name)?;
            for index in 1..arguments.len() {
                let number = int(argument(index)?, name)?;
                best = if function == Function::Min {
                    best.min(number)
                } else {
                    best.max(number)
                };
            }
            Ok(Value::Int(best))
        }
        Function::Upper => Ok(Value::Str(string(argument(0)?, name)?.to_ascii_uppercase())),
        Function::Lower => Ok(Value::Str(string(argument(0)?, name)?.to_ascii_lowercase())),
        Function::Reverse => Ok(Value::Str(
            string(argument(0)?, name)?.chars().rev().collect(),
        )),
        Function::Count => {
            let text = string(argument(0)?, name)?;
            let part = string(argument(1)?, name)?;
            if part.is_empty() {
                return Err(EvalError::Type(
                    "count needs a part of at least one character".into(),
                ));
            }
            let found = text.matches(part.as_str()).count();
            Ok(Value::Int(i128::try_from(found).unwrap_or(i128::MAX)))
        }
    }
}

fn collect_variables(expr: &Expr, names: &mut BTreeSet<String>) {
    match expr {
        Expr::Literal(_) => {}
        Expr::Var(name) => {
            names.insert(name.clone());
        }
        Expr::Neg(inner) | Expr::Not(inner) => collect_variables(inner, names),
        Expr::Binary(_, left, right) => {
            collect_variables(left, names);
            collect_variables(right, names);
        }
        Expr::Call(_, arguments) => {
            for argument in arguments {
                collect_variables(argument, names);
            }
        }
    }
}

/// Text with `{expression}` placeholders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    source: String,
    parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    Placeholder(Expr),
}

/// A piece of a template made only of text and bare variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternPart {
    /// Literal text.
    Text(String),
    /// A placeholder holding just a variable.
    Var(String),
}

impl Template {
    /// Parses a template.
    ///
    /// # Errors
    ///
    /// Explains unbalanced braces or a bad expression.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut rest = text;
        while let Some(character) = rest.chars().next() {
            if let Some(after) = rest.strip_prefix("{{") {
                literal.push('{');
                rest = after;
            } else if let Some(after) = rest.strip_prefix("}}") {
                literal.push('}');
                rest = after;
            } else if character == '}' {
                return error("a lone } must be written }}");
            } else if character == '{' {
                let end = placeholder_end(&rest[1..])
                    .ok_or_else(|| ParseError("a { is missing its }".into()))?;
                let source = &rest[1..=end];
                if source.trim().is_empty() {
                    return error("{} is empty; put an expression inside or write {{}} for braces");
                }
                let expr = parse(source).map_err(|problem| {
                    ParseError(format!("in {{{}}}: {problem}", source.trim()))
                })?;
                if !literal.is_empty() {
                    parts.push(Part::Text(std::mem::take(&mut literal)));
                }
                parts.push(Part::Placeholder(expr));
                rest = &rest[end + 2..];
            } else {
                literal.push(character);
                rest = &rest[character.len_utf8()..];
            }
        }
        if !literal.is_empty() {
            parts.push(Part::Text(literal));
        }
        Ok(Self {
            source: text.to_owned(),
            parts,
        })
    }

    /// The template as written.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether the template has any placeholders.
    #[must_use]
    pub fn has_placeholders(&self) -> bool {
        self.parts
            .iter()
            .any(|part| matches!(part, Part::Placeholder(_)))
    }

    /// Every variable the placeholders use.
    #[must_use]
    pub fn variables(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for part in &self.parts {
            if let Part::Placeholder(expr) = part {
                collect_variables(expr, &mut names);
            }
        }
        names
    }

    /// The template as text and bare variables, or `None` when a placeholder computes something.
    #[must_use]
    pub fn pattern(&self) -> Option<Vec<PatternPart>> {
        self.parts
            .iter()
            .map(|part| match part {
                Part::Text(text) => Some(PatternPart::Text(text.clone())),
                Part::Placeholder(Expr::Var(name)) => Some(PatternPart::Var(name.clone())),
                Part::Placeholder(_) => None,
            })
            .collect()
    }

    /// Fills in the placeholders.
    ///
    /// # Errors
    ///
    /// Fails when a placeholder can't be evaluated or printed.
    pub fn render(&self, env: &Env) -> Result<String, EvalError> {
        let mut text = String::new();
        for part in &self.parts {
            match part {
                Part::Text(literal) => text.push_str(literal),
                Part::Placeholder(expr) => text.push_str(&eval(expr, env)?.render()?),
            }
        }
        Ok(text)
    }

    /// Evaluates a template that is exactly one placeholder, keeping the value's kind.
    ///
    /// # Errors
    ///
    /// Fails when the template isn't a single placeholder or can't be evaluated.
    pub fn value(&self, env: &Env) -> Result<Value, EvalError> {
        match self.parts.as_slice() {
            [Part::Placeholder(expr)] => eval(expr, env),
            _ => Err(EvalError::Type(
                "write the exit code as a number or as one placeholder like \"{…}\"".into(),
            )),
        }
    }
}

/// Finds the `}` closing a placeholder, skipping braces inside string literals.
fn placeholder_end(text: &str) -> Option<usize> {
    let mut in_string = false;
    let mut escaped = false;
    for (index, character) in text.char_indices() {
        match character {
            _ if escaped => escaped = false,
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '}' if !in_string => return Some(index),
            _ => {}
        }
    }
    None
}

/// Draws a variable's values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Generator {
    /// `int(lo, hi)`: whole numbers from `lo` to `hi`, inclusive.
    Int {
        /// Smallest value.
        lo: i128,
        /// Largest value.
        hi: i128,
    },
    /// `choice(a, b, …)`: one of the listed values.
    Choice(Vec<Value>),
    /// `string(min, max, "chars")`: strings of `min` to `max` characters from `chars`.
    Str {
        /// Shortest length.
        min: usize,
        /// Longest length.
        max: usize,
        /// The characters to draw from, without duplicates.
        charset: Vec<char>,
    },
}

impl Generator {
    /// Parses a generator such as `int(-10, 10)`.
    ///
    /// # Errors
    ///
    /// Explains what the generator should look like.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let usage = "write int(lo, hi), choice(a, b, …) or string(min, max, \"chars\") with \
                     literal arguments";
        let tokens = lex(text)?;
        let (Some(Token::Ident(name)), Some(Token::Open), Some(Token::Close)) =
            (tokens.first(), tokens.get(1), tokens.last())
        else {
            return error(usage);
        };
        let mut arguments = Vec::new();
        let mut inner = tokens[2..tokens.len() - 1].iter().peekable();
        while inner.peek().is_some() {
            let value = match (inner.next(), inner.peek()) {
                (Some(Token::Int(number)), _) => Value::Int(*number),
                (Some(Token::Op("-")), Some(Token::Int(number))) => {
                    let number = *number;
                    inner.next();
                    Value::Int(-number)
                }
                (Some(Token::Str(text)), _) => Value::Str(text.clone()),
                _ => return error(format!("{name}(…) takes literal values only; {usage}")),
            };
            arguments.push(value);
            match inner.next() {
                None => break,
                Some(Token::Comma) if inner.peek().is_some() => {}
                Some(_) => return error(format!("separate {name}'s arguments with commas")),
            }
        }
        match (name.as_str(), arguments.as_slice()) {
            ("int", [Value::Int(lo), Value::Int(hi)]) => {
                let range = i128::from(i64::MIN)..=i128::from(i64::MAX);
                if lo > hi || !range.contains(lo) || !range.contains(hi) {
                    return error(format!(
                        "int({lo}, {hi}) needs lo ≤ hi, both between {} and {}",
                        i64::MIN,
                        i64::MAX
                    ));
                }
                Ok(Self::Int { lo: *lo, hi: *hi })
            }
            ("int", _) => error("int takes two whole numbers: int(lo, hi)"),
            ("choice", values) if !values.is_empty() => {
                let all_ints = values.iter().all(|value| matches!(value, Value::Int(_)));
                let all_strings = values.iter().all(|value| matches!(value, Value::Str(_)));
                if !all_ints && !all_strings {
                    return error("choice's values must be all numbers or all strings");
                }
                Ok(Self::Choice(values.to_vec()))
            }
            ("choice", _) => error("choice needs at least one value"),
            ("string", [Value::Int(min), Value::Int(max), Value::Str(chars)]) => {
                let limit = i128::try_from(MAX_STRING_LEN).unwrap_or(i128::MAX);
                if *min < 0 || min > max || *max > limit {
                    return error(format!(
                        "string({min}, {max}, …) needs 0 ≤ min ≤ max ≤ {MAX_STRING_LEN}"
                    ));
                }
                let mut charset = Vec::new();
                for character in chars.chars() {
                    if character == '\0' {
                        return error("string's characters can't include NUL");
                    }
                    if !charset.contains(&character) {
                        charset.push(character);
                    }
                }
                if charset.is_empty() {
                    return error("string needs at least one character to draw from");
                }
                Ok(Self::Str {
                    min: usize::try_from(*min).unwrap_or(0),
                    max: usize::try_from(*max).unwrap_or(0),
                    charset,
                })
            }
            ("string", _) => error("string takes string(min, max, \"chars\")"),
            _ => error(format!("unknown generator `{name}`; {usage}")),
        }
    }

    /// Whether `value` is one this generator can produce.
    #[must_use]
    pub fn contains(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::Int { lo, hi }, Value::Int(number)) => (lo..=hi).contains(&number),
            (Self::Choice(values), value) => values.contains(value),
            (Self::Str { min, max, charset }, Value::Str(text)) => {
                (min..=max).contains(&&text.chars().count())
                    && text.chars().all(|character| charset.contains(&character))
            }
            _ => false,
        }
    }

    /// The value whose printed form is `text`, if this generator's kind can print it.
    #[must_use]
    pub fn read(&self, text: &str) -> Option<Value> {
        match self {
            Self::Int { .. } => {
                let digits = text.strip_prefix('-').unwrap_or(text);
                let canonical = !digits.is_empty()
                    && digits.bytes().all(|byte| byte.is_ascii_digit())
                    && (digits == "0" || !digits.starts_with('0'))
                    && text != "-0";
                if canonical {
                    text.parse().ok().map(Value::Int)
                } else {
                    None
                }
            }
            Self::Choice(values) => values
                .iter()
                .find(|value| value.render().is_ok_and(|rendered| rendered == text))
                .cloned(),
            Self::Str { .. } => Some(Value::Str(text.to_owned())),
        }
    }

    /// The edge values tried first: an int range's ends, 0 and ±1; every choice (or the first
    /// and last of many); the shortest and longest strings.
    #[must_use]
    pub fn boundaries(&self) -> Vec<Value> {
        let mut values = match self {
            Self::Int { lo, hi } => [*lo, *hi, 0, 1, -1]
                .into_iter()
                .filter(|number| (lo..=hi).contains(&number))
                .map(Value::Int)
                .collect(),
            Self::Choice(values) if values.len() <= 16 => values.clone(),
            Self::Choice(values) => vec![values[0].clone(), values[values.len() - 1].clone()],
            Self::Str { min, max, charset } => vec![
                Value::Str(charset[0].to_string().repeat(*min)),
                Value::Str(charset[charset.len() - 1].to_string().repeat(*max)),
            ],
        };
        let mut seen = Vec::new();
        values.retain(|value| {
            let new = !seen.contains(value);
            seen.push(value.clone());
            new
        });
        values
    }

    /// A random value.
    pub fn sample(&self, rng: &mut ChaCha8Rng) -> Value {
        match self {
            Self::Int { lo, hi } => Value::Int(rng.random_range(*lo..=*hi)),
            Self::Choice(values) => values[rng.random_range(0..values.len())].clone(),
            Self::Str { min, max, charset } => {
                let length = rng.random_range(*min..=*max);
                Value::Str(
                    (0..length)
                        .map(|_| charset[rng.random_range(0..charset.len())])
                        .collect(),
                )
            }
        }
    }

    /// How many different values it can draw, counted exactly; `None` when it's more than a
    /// `u128` holds.
    #[must_use]
    pub fn domain_size(&self) -> Option<u128> {
        match self {
            Self::Int { lo, hi } => u128::try_from(hi - lo).ok()?.checked_add(1),
            Self::Choice(values) => u128::try_from(distinct(values).len()).ok(),
            Self::Str { min, max, charset } => {
                let base = u128::try_from(charset.len()).ok()?;
                let mut total: u128 = 0;
                for length in *min..=*max {
                    let count = base.checked_pow(u32::try_from(length).ok()?)?;
                    total = total.checked_add(count)?;
                }
                Some(total)
            }
        }
    }

    /// The value at `index` (below [`Self::domain_size`]) in a fixed order: numbers from low to
    /// high, choices as listed, strings by length and then by the order of their characters.
    #[must_use]
    pub fn nth(&self, index: u128) -> Value {
        match self {
            Self::Int { lo, .. } => Value::Int(lo + i128::try_from(index).unwrap_or(0)),
            Self::Choice(values) => {
                let values = distinct(values);
                values[usize::try_from(index).unwrap_or(0) % values.len()].clone()
            }
            Self::Str { min, max, charset } => {
                let base = charset.len() as u128;
                let mut rest = index;
                for length in *min..=*max {
                    let count = base.saturating_pow(u32::try_from(length).unwrap_or(u32::MAX));
                    if rest < count {
                        let mut digits = Vec::with_capacity(length);
                        for _ in 0..length {
                            digits.push(charset[usize::try_from(rest % base).unwrap_or(0)]);
                            rest /= base;
                        }
                        digits.reverse();
                        return Value::Str(digits.into_iter().collect());
                    }
                    rest -= count;
                }
                Value::Str(String::new())
            }
        }
    }

    /// The numbers that bound this generator: an int range's ends and a string's lengths.
    #[must_use]
    pub fn limits(&self) -> Vec<i128> {
        match self {
            Self::Int { lo, hi } => vec![*lo, *hi],
            Self::Choice(_) => Vec::new(),
            Self::Str { min, max, .. } => [*min, *max]
                .into_iter()
                .map(|length| i128::try_from(length).unwrap_or(i128::MAX))
                .collect(),
        }
    }
}

impl fmt::Display for Generator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int { lo, hi } => write!(f, "int({lo}, {hi})"),
            Self::Choice(values) => {
                let values: Vec<String> = values.iter().map(ToString::to_string).collect();
                write!(f, "choice({})", values.join(", "))
            }
            Self::Str { min, max, charset } => {
                let chars: String = charset.iter().collect();
                write!(f, "string({min}, {max}, {chars:?})")
            }
        }
    }
}

/// The values without repeats, in their first order.
fn distinct(values: &[Value]) -> Vec<Value> {
    let mut seen = Vec::new();
    for value in values {
        if !seen.contains(value) {
            seen.push(value.clone());
        }
    }
    seen
}

/// Every combination of the variables' values, in a fixed order: the first variable changes
/// fastest. Use it only when [`combinations`] is small.
pub fn every_case(vars: &[(String, Generator)]) -> Cases<'_> {
    let sizes: Vec<u128> = vars
        .iter()
        .map(|(_, generator)| generator.domain_size().unwrap_or(0))
        .collect();
    let total = combinations(vars).map_or(0, |count| usize::try_from(count).unwrap_or(0));
    Cases {
        vars,
        mode: Mode::Every { sizes },
        runs: total,
        index: 0,
    }
}

/// How many combinations of values the variables have, when it fits a `u128`.
#[must_use]
pub fn combinations(vars: &[(String, Generator)]) -> Option<u128> {
    vars.iter().try_fold(1_u128, |product, (_, generator)| {
        product.checked_mul(generator.domain_size()?)
    })
}

/// Every run's variable values: boundary values first (every combination when there are at most
/// 64, otherwise each boundary value once), then values drawn from a ChaCha8 generator seeded with
/// `seed`. Runs are made one at a time, so long strings don't pile up in memory.
pub fn cases(vars: &[(String, Generator)], runs: usize, seed: u64) -> Cases<'_> {
    let boundaries: Vec<Vec<Value>> = vars
        .iter()
        .map(|(_, generator)| generator.boundaries())
        .collect();
    let combinations = boundaries
        .iter()
        .try_fold(1_usize, |product, values| product.checked_mul(values.len()));
    let (boundary_runs, every_combination) = match combinations {
        Some(count) if count <= MAX_BOUNDARY_CASES => (count, true),
        _ => (boundaries.iter().map(Vec::len).max().unwrap_or(0), false),
    };
    Cases {
        vars,
        mode: Mode::Sampled {
            boundaries,
            boundary_runs: boundary_runs.min(runs),
            every_combination,
            rng: Box::new(ChaCha8Rng::seed_from_u64(seed)),
        },
        runs,
        index: 0,
    }
}

/// The runs [`cases`] or [`every_case`] makes, one at a time.
#[derive(Debug)]
pub struct Cases<'a> {
    vars: &'a [(String, Generator)],
    mode: Mode,
    runs: usize,
    index: usize,
}

#[derive(Debug)]
enum Mode {
    /// Boundary values, then random ones.
    Sampled {
        boundaries: Vec<Vec<Value>>,
        boundary_runs: usize,
        every_combination: bool,
        rng: Box<ChaCha8Rng>,
    },
    /// Every combination; each variable's domain size.
    Every { sizes: Vec<u128> },
}

impl Iterator for Cases<'_> {
    type Item = Env;

    fn next(&mut self) -> Option<Env> {
        if self.index >= self.runs {
            return None;
        }
        let index = self.index;
        self.index += 1;
        match &mut self.mode {
            Mode::Every { sizes } => {
                let mut rest = index as u128;
                Some(
                    self.vars
                        .iter()
                        .zip(sizes.iter())
                        .map(|((name, generator), size)| {
                            let value = generator.nth(rest % size);
                            rest /= size;
                            (name.clone(), value)
                        })
                        .collect(),
                )
            }
            Mode::Sampled {
                boundaries,
                boundary_runs,
                every_combination,
                ..
            } if index < *boundary_runs => {
                let mut rest = index;
                let every_combination = *every_combination;
                Some(
                    self.vars
                        .iter()
                        .zip(boundaries.iter())
                        .map(|((name, _), values)| {
                            let value = if every_combination {
                                let value = values[rest % values.len()].clone();
                                rest /= values.len();
                                value
                            } else {
                                values[index % values.len()].clone()
                            };
                            (name.clone(), value)
                        })
                        .collect(),
                )
            }
            Mode::Sampled { rng, .. } => Some(
                self.vars
                    .iter()
                    .map(|(name, generator)| (name.clone(), generator.sample(rng)))
                    .collect(),
            ),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.runs - self.index;
        (left, Some(left))
    }
}

impl ExactSizeIterator for Cases<'_> {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn value(text: &str, env: &Env) -> Result<Value, EvalError> {
        eval(&parse(text).expect("valid expression"), env)
    }

    fn number(text: &str) -> i128 {
        match value(text, &Env::new()) {
            Ok(Value::Int(number)) => number,
            other => panic!("{text} gave {other:?}"),
        }
    }

    fn env(pairs: &[(&str, Value)]) -> Env {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect()
    }

    #[test]
    fn follows_precedence() {
        assert_eq!(number("1 + 2 * 3"), 7);
        assert_eq!(number("(1 + 2) * 3"), 9);
        assert_eq!(number("10 - 4 - 3"), 3);
        assert_eq!(number("2 * 3 % 4"), 2);
        assert_eq!(number("max(1, 2) + min(3, 4, 5) * abs(-2)"), 8);
        assert_eq!(
            value("1 < 2 && 2 < 1 || !(3 == 4)", &Env::new()),
            Ok(Value::Bool(true))
        );
        assert!(parse("1 < 2 < 3").is_err(), "comparisons don't chain");
    }

    #[test]
    fn handles_unary_minus() {
        assert_eq!(number("-2 * 3"), -6);
        assert_eq!(number("2 - -3"), 5);
        assert_eq!(number("--4"), 4);
        assert_eq!(number("-(1 + 2)"), -3);
    }

    #[test]
    fn divides_like_idiv() {
        assert_eq!(number("7 / 2"), 3);
        assert_eq!(number("-7 / 2"), -3);
        assert_eq!(number("7 % -2"), 1);
        assert_eq!(number("-7 % 2"), -1);
        assert_eq!(value("1 / 0", &Env::new()), Err(EvalError::DivideByZero));
        assert_eq!(value("1 % 0", &Env::new()), Err(EvalError::DivideByZero));
    }

    #[test]
    fn reports_overflow() {
        let max = i128::MAX.to_string();
        assert!(matches!(
            value(&format!("{max} + 1"), &Env::new()),
            Err(EvalError::Overflow(_))
        ));
        assert!(matches!(
            value(&format!("{max} * 2"), &Env::new()),
            Err(EvalError::Overflow(_))
        ));
        assert!(matches!(
            value(&format!("-(-{max} - 1)"), &Env::new()),
            Err(EvalError::Overflow(_))
        ));
        assert!(parse("999999999999999999999999999999999999999999").is_err());
    }

    #[test]
    fn counts_and_lists_every_value() {
        let int = Generator::parse("int(-2, 2)").expect("valid");
        assert_eq!(int.domain_size(), Some(5));
        assert_eq!(int.nth(0), Value::Int(-2));
        assert_eq!(int.nth(4), Value::Int(2));
        let choice = Generator::parse("choice(1, 2, 1)").expect("valid");
        assert_eq!(choice.domain_size(), Some(2), "repeats count once");
        let text = Generator::parse(r#"string(0, 2, "ab")"#).expect("valid");
        assert_eq!(text.domain_size(), Some(1 + 2 + 4));
        let all: Vec<Value> = (0..7).map(|index| text.nth(index)).collect();
        let strings = ["", "a", "b", "aa", "ab", "ba", "bb"];
        assert_eq!(all, strings.map(|text| Value::Str(text.into())));
        let huge = Generator::parse(r#"string(0, 4096, "abc")"#).expect("valid");
        assert_eq!(huge.domain_size(), None, "too many to count");

        let vars = vec![("a".to_owned(), int), ("s".to_owned(), text)];
        assert_eq!(combinations(&vars), Some(35));
        let every: Vec<Env> = every_case(&vars).collect();
        assert_eq!(every.len(), 35);
        let distinct: BTreeSet<String> = every.iter().map(|env| format!("{env:?}")).collect();
        assert_eq!(distinct.len(), 35, "each combination once");
    }

    #[test]
    fn calls_functions() {
        let vars = env(&[("s", Value::Str("Héllo".into()))]);
        assert_eq!(
            value("len(s)", &vars),
            Ok(Value::Int(6)),
            "len counts bytes"
        );
        assert_eq!(value("upper(s)", &vars), Ok(Value::Str("HéLLO".into())));
        assert_eq!(value("lower(s)", &vars), Ok(Value::Str("héllo".into())));
        assert_eq!(value("reverse(s)", &vars), Ok(Value::Str("olléH".into())));
        assert_eq!(value("count(s, \"l\")", &vars), Ok(Value::Int(2)));
        assert_eq!(
            value("count(\"aaaa\", \"aa\")", &vars),
            Ok(Value::Int(2)),
            "count doesn't overlap"
        );
        assert!(value("count(s, \"\")", &vars).is_err());
        assert!(parse("count(s)").is_err_and(|error| error.0.contains("two arguments")));
        assert_eq!(
            value("if(1 > 0, \"yes\", 1 / 0)", &vars),
            Ok(Value::Str("yes".into())),
            "if evaluates only the chosen branch"
        );
        assert!(parse("lenght(s)").is_err_and(|error| error.0.contains("unknown function")));
        assert!(parse("abs(1, 2)").is_err_and(|error| error.0.contains("one argument")));
        assert!(parse("if(true, 1)").is_err());
        assert!(parse("min(1)").is_err());
    }

    #[test]
    fn checks_kinds() {
        let vars = env(&[("s", Value::Str("a".into()))]);
        assert!(matches!(value("1 + s", &vars), Err(EvalError::Type(_))));
        assert!(matches!(
            value("if(1, 2, 3)", &vars),
            Err(EvalError::Type(_))
        ));
        assert!(matches!(value("s == 1", &vars), Err(EvalError::Type(_))));
        assert!(matches!(value("s < \"b\"", &vars), Err(EvalError::Type(_))));
        assert_eq!(
            value("x", &vars),
            Err(EvalError::UnknownVariable("x".into()))
        );
        assert!(parse("a = 1").is_err_and(|error| error.0.contains("==")));
    }

    #[test]
    fn renders_templates() {
        let vars = env(&[("a", Value::Int(2)), ("b", Value::Int(3))]);
        let template = Template::parse("{{sum}} = {a + b}\n").expect("valid template");
        assert_eq!(template.render(&vars), Ok("{sum} = 5\n".into()));
        assert_eq!(
            template.variables(),
            ["a".to_owned(), "b".to_owned()].into()
        );
        assert!(template.pattern().is_none(), "it computes a + b");
        let braces = Template::parse("{if(a > 0, \"}\", \"{\")}").expect("braces in strings");
        assert_eq!(braces.render(&vars), Ok("}".into()));
        let escapes = Template::parse("{\"q\\\"\\\\\\n\\t\"}").expect("escapes");
        assert_eq!(escapes.render(&vars), Ok("q\"\\\n\t".into()));
        assert!(Template::parse("{a").is_err());
        assert!(Template::parse("a}").is_err());
        assert!(Template::parse("{}").is_err());
        assert!(Template::parse("{a +}").is_err());
        let truth = Template::parse("{a > b}").expect("valid template");
        assert!(matches!(truth.render(&vars), Err(EvalError::Type(_))));
    }

    #[test]
    fn reads_bare_variable_patterns() {
        let template = Template::parse("--count={n}:{s}").expect("valid template");
        assert_eq!(
            template.pattern(),
            Some(vec![
                PatternPart::Text("--count=".into()),
                PatternPart::Var("n".into()),
                PatternPart::Text(":".into()),
                PatternPart::Var("s".into()),
            ])
        );
        assert!(!Template::parse("plain").expect("text").has_placeholders());
    }

    #[test]
    fn parses_generators() {
        assert_eq!(
            Generator::parse("int(-5, 5)"),
            Ok(Generator::Int { lo: -5, hi: 5 })
        );
        assert_eq!(
            Generator::parse("choice(\"a\", \"b\")").map(|g| g.to_string()),
            Ok("choice(\"a\", \"b\")".into())
        );
        assert_eq!(
            Generator::parse("string(0, 3, \"abca\")"),
            Ok(Generator::Str {
                min: 0,
                max: 3,
                charset: vec!['a', 'b', 'c'],
            })
        );
        for invalid in [
            "int(5, 1)",
            "int(0, 99999999999999999999)",
            "int(1)",
            "int(a, 2)",
            "choice()",
            "choice(1, \"a\")",
            "string(2, 1, \"a\")",
            "string(0, 3, \"\")",
            "string(0, 1048577, \"a\")",
            "float(0, 1)",
            "int(1, 2",
            "int(1 2)",
        ] {
            assert!(Generator::parse(invalid).is_err(), "{invalid} was accepted");
        }
    }

    #[test]
    fn reads_and_contains_values() {
        let range = Generator::Int { lo: -10, hi: 10 };
        assert_eq!(range.read("7"), Some(Value::Int(7)));
        assert_eq!(range.read("-7"), Some(Value::Int(-7)));
        for not_canonical in ["007", "-0", "+7", "", "-", "7a"] {
            assert_eq!(range.read(not_canonical), None, "{not_canonical:?}");
        }
        assert!(range.contains(&Value::Int(10)));
        assert!(!range.contains(&Value::Int(11)));
        let choice = Generator::Choice(vec![Value::Int(1), Value::Int(2)]);
        assert_eq!(choice.read("2"), Some(Value::Int(2)));
        assert_eq!(choice.read("3"), None);
        let text = Generator::Str {
            min: 1,
            max: 2,
            charset: vec!['a', 'b'],
        };
        assert!(text.contains(&Value::Str("ab".into())));
        assert!(!text.contains(&Value::Str("abc".into())));
        assert!(!text.contains(&Value::Str("ac".into())));
        assert!(!text.contains(&Value::Str(String::new())));
    }

    #[test]
    fn tries_boundaries_first() {
        assert_eq!(
            Generator::Int { lo: -5, hi: 5 }.boundaries(),
            [-5, 5, 0, 1, -1].map(Value::Int)
        );
        assert_eq!(
            Generator::Int { lo: 1, hi: 10 }.boundaries(),
            [1, 10].map(Value::Int)
        );
        assert_eq!(
            Generator::Str {
                min: 0,
                max: 2,
                charset: vec!['a', 'z'],
            }
            .boundaries(),
            [Value::Str(String::new()), Value::Str("zz".into())]
        );
        let vars = vec![
            ("a".to_owned(), Generator::Int { lo: -5, hi: 5 }),
            ("b".to_owned(), Generator::Int { lo: 1, hi: 10 }),
        ];
        let runs: Vec<Env> = cases(&vars, 20, 1).collect();
        assert_eq!(runs.len(), 20);
        assert_eq!(runs[0], env(&[("a", Value::Int(-5)), ("b", Value::Int(1))]));
        assert_eq!(runs[1], env(&[("a", Value::Int(5)), ("b", Value::Int(1))]));
        let boundary_pairs: BTreeSet<_> = runs[..10].iter().cloned().collect();
        assert_eq!(
            boundary_pairs.len(),
            10,
            "all 5 × 2 boundary pairs come first"
        );
        assert!(runs.iter().all(|case| {
            vars.iter()
                .all(|(name, generator)| generator.contains(&case[name]))
        }));
    }

    #[test]
    fn random_cases_depend_only_on_the_seed() {
        let vars = vec![
            (
                "n".to_owned(),
                Generator::Int {
                    lo: -1000,
                    hi: 1000,
                },
            ),
            (
                "s".to_owned(),
                Generator::Str {
                    min: 0,
                    max: 8,
                    charset: "abc".chars().collect(),
                },
            ),
        ];
        let collect = |seed| cases(&vars, 50, seed).collect::<Vec<Env>>();
        assert_eq!(collect(7), collect(7));
        assert_ne!(collect(7), collect(8));
        assert_eq!(cases(&vars, 3, 7).len(), 3);
    }

    #[test]
    fn validates_variable_names() {
        assert!(is_variable_name("a"));
        assert!(is_variable_name("count_2"));
        for invalid in ["", "2a", "A", "len", "true", "a-b", "string"] {
            assert!(!is_variable_name(invalid), "{invalid:?}");
        }
    }
}
