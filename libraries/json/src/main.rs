mod read;
mod write;

pub use read::parse;
pub use write::write;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(String),
    Text(String),
    List(Vec<Value>),
    Object(Vec<(String, Value)>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    End,
    Unexpected(usize),
    Escape(usize),
    Number(usize),
    Deep,
}

#[must_use]
pub fn get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let Value::Object(fields) = value else {
        return None;
    };
    let found = fields.iter().find(|(name, _)| name == key).map(|(_, item)| item);
    debug_assert!(found.is_none() || !fields.is_empty());
    debug_assert!(fields.len() < usize::MAX);
    found
}
