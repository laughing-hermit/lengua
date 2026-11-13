# Purpose

The module writes a value as JSON text with no spaces. It writes a text in quotes, with an
escape for a quote, a backslash and each control character.

# Behaviours

## Write single values

Given a single value,
When a program calls `write`,
Then it gives its JSON text.

### Example: null

- Call: `write(&Value::Null)`
- Expect value: `"null"`

### Example: true

- Call: `write(&Value::Bool(true))`
- Expect value: `"true"`

### Example: false

- Call: `write(&Value::Bool(false))`
- Expect value: `"false"`

### Example: a number

- Call: `write(&Value::Number("-1.5".to_string()))`
- Expect value: `"-1.5"`

### Example: a text with escapes

- Call: `write(&Value::Text("a\"b\\c\n\r\t\u{1} d".to_string()))`
- Expect value: `"\"a\\\"b\\\\c\\n\\r\\t\\u0001 d\""`

## Write lists and objects

Given lists and objects in other lists and objects,
When a program calls `write`,
Then it gives the JSON text with commas between the parts.

### Example: a list in a list

- Call: `write(&parse("[1, [2, {\"k\": null}], \"x\"]").unwrap())`
- Expect value: `"[1,[2,{\"k\":null}],\"x\"]"`

### Example: an object of two fields

- Call: `write(&parse("{\"a\": 1, \"b\": [true, false]}").unwrap())`
- Expect value: `"{\"a\":1,\"b\":[true,false]}"`

### Example: empty values

- Call: `write(&parse("[[], {}]").unwrap())`
- Expect value: `"[[],{}]"`

### Cases: texts that go through `write` and `parse`

- Range `s`: text of 0 to 12 characters from "ab é~"
- Call: `parse(&write(&Value::Text("{s}".to_string())))`
- Expect value: `Ok(Text("{s}"))`

# Structure

- `write`: writes a value as JSON text, with a stack of tasks in place of recursion.
- `expand`: writes a single value, or puts the parts of a list or an object on the stack.
- `quote`: writes a text in quotes, with its escapes.
