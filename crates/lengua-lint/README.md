# lengua-lint

## Purpose

`lengua-lint` reads the Rust source of a lengua program and holds it to the rules of the Power
of 10 that lengua's lint holds: rules 1, 2, 4, 5, 6, 8, 10 (the attributes that turn a warning
off), 11 and 12. It uses the Rust standard library only: its own tokenizer of Rust's lexical
grammar and its own reader of items and function bodies, no `syn`. What it can't read soundly is
the finding `rust-read`, not a guess. Since L5 it also makes the mutants of a program's code
for the mutation score. What each rule accepts and refuses, and what the reader
resolves, is in [Lengua3.md](../../Lengua3.md), section 5; the mutations are in section 7, "How
the tests run".

## Behaviours

- Given the Rust files of a program, each with the names of its idea's `# Structure`, when
  `lint` runs, then it gives every finding of every rule, each with the file, the line, the
  code and a plain message, sorted by file and line.
- Given a program that keeps every rule, when `lint` runs, then it gives no findings.
- Given the Rust files of a program and a cap, when `mutants` runs, then it gives at most the
  cap of mutants, spread over the functions, each with its file, line, function, change and
  edits, and the program's text with the bodies of its functions left out.
- Given a file that the tokenizer can't read (a string or a comment with no end, a delimiter
  that does not close), when `lint` runs, then it gives one `rust-read` finding for the file
  and checks the other files.

## Structure

- `lib.rs`: `lint` reads every file (`code::read`), builds the model (`model::read`), and runs
  `file_rules` (comments, tokens, macro definitions, loops in macros), `function_rules`
  (loops, nested items, length), `assertions` (with `assertions_in`), `recursion` and
  `structure`; `finding` makes a finding; `Finding`, `Source` and `LINES` are its interface.
- `lex.rs`: `tokens` cuts the text into tokens, each with its line and its byte offsets, with
  the `Lexer` (`peek`, `bump`, `bump_n`,
  `eat_while`, `push`, `shebang`, `token`, `block_comment`, `raw_prefix`, `raw_string`,
  `string`, `quote`, `exponent`, `number`); `Token::is`, `Kind`, `Error`, `ident_start`,
  `ident_continue`, `digit` and `unterminated` help it.
- `code.rs`: `read` pairs the delimiters into a `Code`, whose methods (`is`, `kind`, `text`,
  `line`, `open`, `after`, `end`, `joint`, `path_sep`, `colon`, `single_eq`, `arrow_close`,
  `ident`, `name`, `segment`, `after_dot`, `after_path`, `path`, `angle_end`, `names`,
  `macro_call`, `split`, `find`, `len`) read it; `keyword` and `closer` help.
- `model.rs`: `read` and the `Reader` (`items`, `item`, `function`, `type_def`, `trait_def`,
  `impl_def`, `module`, `use_item`, `alias`, `macro_def`, `macro_item`, `past_semicolon`,
  `brace`, `finding`, `unreadable`) find the items: `Function`, `Owner`, `TypeDef`, `Macro`,
  `Use`, in a `Model`; `skip_attributes`, `skip_qualifiers`, `qualifier_then_item`,
  `generic_names`, `segment_end`, `named_fields`, `tuple_fields`, `variant_types`,
  `impl_parts`, `last_name` and `target_name` read their parts.
- `graph.rs`: `Index` resolves the calls of each function (`edges`, `scan`, `single`, `path`,
  `method`, `receiver`, `field_types`, `scope`, `inline_macro`, `on_name`, `on_type`,
  `on_trait`, `on_self`, `generic`, `keyed`, `every_method`); `bindings`, `params`,
  `pattern_names`, `let_end`, `closure_start` and `arms` find the local names; `cycle` finds a
  path back to a function.
- `loops.rs`: `check` holds every `for`, `while` and `loop` to rule 2 (`for_loop`,
  `open_range`, `while_loop`, `endless_loop`, `exits`, `counted`, `step`, `one`,
  `other_change`, `changes`, `early_continue`, `loop_body`, `label`, `compare`, `operator`,
  `constant`).
- `rules.rs`: `Context` holds rules 1 (`dyn`, function pointers), 6 (`static_item`,
  `static_type`), 8 (`macro_use`, `derive`, `attribute`, `cfg`, `macro_definitions`), 10 and 11
  (`comments`) over the tokens of a file (`tokens`), with `declared`, `imported` and
  `local_macro`.
- `tests/rules.rs`: for every rule, a program that breaks it is refused with that rule's code,
  and a program that keeps them all has no findings; `lex.rs` has the tokenizer's tests,
  tricky inputs and random slices among them.
- `mutate.rs`: `mutants` reads each file and, for each function (`function_mutants`), finds
  the places to change (`site`: `operator`, with `operand_end` and `operand_start`; `integer`;
  the booleans with `replaced`; `negate` for a condition; `default_return`), leaving out the
  arguments of assertions (`assertion_spans`), and spreads the cap over the functions; `apply`
  makes a mutant's text from its `Edit`s; `Mutant`, `Mutation`, `Site`, `OPERATORS` and
  `DEFAULTS` are its data, `joined` and `line_start` its helpers.
