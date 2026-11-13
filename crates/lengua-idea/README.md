# lengua-idea

## Purpose

`lengua-idea` reads the idea files of a lengua program and checks them: the frontmatter, the
four sections, the behaviours and their examples, the constraints, the structure, the modules
and ASD-STE100 Simplified Technical English. It collects every problem of every idea in one
pass. When there are no problems, it makes `Cargo.toml` and `wit/world.wit` from the ideas,
holds the ones already there to what it wrote, and writes them with the wiki: the frontmatter
of each idea, `index.md` and `log.md`. It uses the Rust standard library and `lengua-lint`
(itself on the standard library alone) only. The grammar and
the formats are in
[Lengua3.md](../../Lengua3.md), section 4, "The idea file's grammar", "What the STE check
holds", "The wiki's formats" and "The generated files, the build job and `doctor`".

## Behaviours

- Given a folder with no `idea.md`, when `check` runs, then it gives one finding,
  `idea-missing`.
- Given a folder whose ideas follow the grammar, when `check` runs, then it gives no findings
  and writes the frontmatter of each idea, `index.md`, `log.md`, `Cargo.toml` and
  `wit/world.wit`, and its report has the build's package, file and key.
- Given the same folder, when `check` runs again, then it writes only `log.md`, with an entry
  that says nothing changed.
- Given a Rust file that changed while its idea did not, when `check` runs, then its reply has
  a note and the log entry says so.
- Given a `src/main.rs` that breaks a rule of the Power of 10 that lengua's lint holds, when
  `check` runs, then it gives each finding of the lint with the findings of the ideas, and
  writes nothing.
- Given a folder with problems of many kinds, when `check` runs, then it gives every problem in
  one report, sorted by file and line, and writes nothing.
- Given a `Cargo.toml` or `wit/world.wit` changed by hand, when `check` runs, then it gives the
  finding `generated-edited` at the first line that differs, and writes nothing; given the
  file removed, it writes it again.

## Structure

- `check` (`lib.rs`): with `prepare`, finds the idea files, reads each one, links the modules,
  sorts the findings and, with none, makes the generated files; then writes them with the wiki.
- `program` (`lib.rs`): does what `check` does without writing anything, for the host's `run`.
- `Finding`, `Note`, `Report` (`lib.rs`): the reply of `check` and `program`.
- `Sink::new`, `Sink::add` (`lib.rs`): collect the findings of one file.
- `Build`, `build`, `sha256_hex` (`lib.rs`): what `check` gives the build job and the host,
  and the SHA-256 of bytes.
- `Grants` (with `Grants::reach`), `Reach`, `Folder`, `grants` (`lib.rs`): the top idea's grants as
  the host reads them: standard
  input, arguments, the clock, random numbers, the folders (read or read-write), the hosts, the
  memory in bytes and the time in milliseconds.
- `markdown_files`, `discover`, `source_file`, `concept`, `read_page`, `link` (`lib.rs`): find
  the ideas, read one, and hold the `- Modules:` and `- Uses:` lines to the files.
- `frontmatter.rs`: `split` reads the frontmatter and `Front::render` writes it back with
  lengua's keys first; `Front::value` and `Front::body_text` read it.
- `idea.rs`: `parse` splits the sections and reads `# Purpose` (`purpose`).
- `behaviours.rs`: `parse` reads the behaviours, `statement` the Given, When and Then lines,
  `block` an Example or Cases block, and `items`, `fenced`, `one_string`, `strings`,
  `state_value`, `values`, `key_place`, `range`, `integer_range`, `text_range`,
  `template_names` and `templates` their items.
- `constraints.rs`: `parse` and `Reader::item` read the constraints; `module_name`, `names`,
  `reads`, `files_in` (with `folder`), `host_port`, `amount`, `semver` and `crate_line` check
  their values, and keep the folders, the hosts, the memory and the time.
- `structure.rs`: `parse` reads the structure, with the line of each name; `function_name`
  checks a name.
- `rust.rs`: `lint`, when `src/main.rs` is there, reads the Rust file of each idea and gives it
  to `lengua-lint` with the names of its `# Structure` and the declared crates, and finds the
  Rust files of no idea (`rust_files`), the ideas with no Rust file and a `build.rs`;
  `finding` makes a finding.
- `generate.rs`: `plan` makes the text of `Cargo.toml` (`cargo_toml`, `Package`) and of
  `wit/world.wit` (`world` and `world_names`: the always set, then the interfaces of each
  grant from `interfaces`), checks the
  package's name (`package_name`) and the crates (`crates`), and holds the files there
  (`held`, `first_difference`); `artifact` names the built file and `inputs` (with `collect`)
  gives the build's key.
- `ste.rs`: `Dictionary::load` reads `ste-words.txt` and `ste-replace.txt`;
  `Dictionary::paragraph`, `description` and `words` check text with the STE rules.
- `text.rs`: `tokenize`, `sentences` and `join` cut text into words and sentences;
  `split_headings`, `literal`, `split_key`, `backticked`, `snake`, `upper_camel` and
  `relative_path` read the markdown.
- `sha256.rs`: `hex` gives the SHA-256 of bytes.
- `wiki.rs`: `write` writes the frontmatter, `index.md`, `log.md` and the generated files;
  `Stamp` is the hash of a generated file in the frontmatter; `date` turns seconds
  into a date.
- `tests/check.rs` and `tests/fixtures/`: a good program and a program with problems of every
  kind.
