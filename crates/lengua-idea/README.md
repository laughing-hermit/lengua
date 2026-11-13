# lengua-idea

## Purpose

`lengua-idea` reads the idea files of a lengua program and checks them: the frontmatter, the
four sections, the behaviours and their examples, the constraints, the structure, the modules
and ASD-STE100 Simplified Technical English. It collects every problem of every idea in one
pass. When there are no problems, it makes `Cargo.toml` and `wit/world.wit` from the ideas,
holds the ones already there to what it wrote, and writes them with the wiki: the frontmatter
of each idea, `index.md` and `log.md`, with the trust label it is given. It reads every
example and Cases block as a test, draws the cases of a block, and gives the mutants of the
code through `lengua-lint`. It uses the Rust standard library and `lengua-lint`
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
- Given a label and a score from the build job, when `check_with` runs, then the frontmatter of
  every idea and `index.md` carry the label and the log entry the label and the score.
- Given an example or a Cases block, when the ideas are read, then the build carries it as a
  `Test` with its input and its expectations, and `cases` gives its runs: one for an example,
  and for a Cases block the boundary values of its ranges first, then 20 drawn from a fixed
  seed.
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
- Given a program's facts and the user's key, when `lock::write` runs, then it writes the lock
  of Lengua3.md, section 4, "The lock": the SHA-256 of every file, signed with HMAC-SHA256.
- Given a lock, when `lock::verify` runs, then it refuses a lock signed with another key or
  changed by hand, any listed file that changed or is not there, and, for a program's lock, a
  new idea or Rust file, naming each file and "run `check`"; otherwise it gives the sealed
  bytes it hashed.

## Structure

- `check` and `check_with` (`lib.rs`): with `prepare`, find the idea files, read each one, link the modules,
  sort the findings and, with none, make the generated files; then write them with the wiki,
  with the label that `check_with`'s function gives for the build (`check` gives `unchecked`),
  and read the ideas again for the lines of the tests in the files as written.
- `Label` (with `Label::name`), `Trust` (with `Trust::text`), `tests_hash` and `mutants`
  (`lib.rs`): the trust label and the score, the hash of the tests with the grants (part of
  the build's key), and the mutants of the program's Rust files.
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
- `cases.rs`: `Test`, `Range`, `End` (with `End::name` and `End::parse`) and `Case` are a
  test and a run of it; `cases` draws the runs of a block (`boundaries`, `pick`, `drawn`, a
  `Draw` from `seed`) and fills its strings (`filled`, `fill`); `CASES_DRAWN` is 20.
- `behaviours.rs`: `parse` reads the behaviours and gives each block as a `Test`, `statement`
  the Given, When and Then lines,
  `block` an Example or Cases block, and `items`, `fenced`, `one_string`, `strings`,
  `state_value`, `values`, `record`, `key_place`, `range`, `integer_range`, `text_range`,
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
  (`held`, `first_difference`); `artifact` names the built file and `inputs` (with `collect`:
  the generated texts, the files of `src/` and the tests' hash)
  gives the build's key.
- `ste.rs`: `Dictionary::load` reads `ste-words.txt` and `ste-replace.txt`;
  `Dictionary::paragraph`, `description` and `words` check text with the STE rules.
- `text.rs`: `tokenize`, `sentences` and `join` cut text into words and sentences;
  `split_headings`, `literal`, `split_key`, `backticked`, `snake`, `upper_camel` and
  `relative_path` read the markdown.
- `sha256.rs`: `digest` and `hex` give the SHA-256 of bytes, `hmac` the HMAC-SHA256 of RFC 2104
  (tested with the vectors of RFC 4231), `to_hex` and `from_hex` turn bytes into hexadecimal and
  back, and `same` compares two digests in a time that does not depend on where they differ.
- `lock.rs`: the user's key and the lock. `data_folder` finds lengua's data folder
  (`LENGUA_HOME`, else `%APPDATA%\lengua`, `~/Library/Application Support/lengua`, or
  `$XDG_DATA_HOME/lengua` or `~/.local/share/lengua`); `create_key`, `read_key`, `user_key` and
  `user_key_or_create` make and read `key` (64 hexadecimal digits, owner-only on Unix,
  `write_private`, `make_folder`); `program_files` lists what a program's lock holds (`ideas`,
  `sources`); `text` and `write` make and write a lock (`Facts`, `Kind`); `verify` checks a
  lock's signature (`signed`) and every file in it (`check_entry`), and gives `Verified`, with
  the `Sealed` bytes that only it makes, for `lengua-seal`.
- `wiki.rs`: `write` writes the frontmatter, `index.md`, `log.md` and the generated files;
  `Stamp` is the hash of a generated file in the frontmatter; `date` turns seconds
  into a date.
- `tests/check.rs` and `tests/fixtures/`: a good program and a program with problems of every
  kind.
- `library.rs` (since L7): `resolve` reads each `- Uses:` folder of the top idea as a lengua
  library: its `idea.md`, its folder's name as the package, its lock verified with
  `lock::verify` (kind `library`), its label and the SHA-256 of its lock; `description` reads the
  library's description from its frontmatter.
- `front.rs` (since L8): the front end of a server with screens. `app_cargo`, `app_lib`,
  `files_cargo` and `files_lib` (with `page` and `media_type`) make the texts of
  `web/app/Cargo.toml`, `web/app/lib.rs`, `web/files/Cargo.toml` and `web/files/lib.rs` from a
  `Plan` (with `Plan::fonts`, `is_font`); `assets` lists the files of `assets/`; `app_name` and
  `artifact` name the front end's package and its `.wasm`; `REQUIRED`, `WASM_BINDGEN`, `KIT`,
  `LIBRARY_NAMES` (the library names of GPUI's crates, for the lint) and the paths are its
  constants. In `generate.rs`, `front_plan` makes the `Front` of the build (with its `Screen`s,
  `type_name`), `front_rules` checks the crates and the fonts, `split` divides the crates of the
  server and of the screens, and `plan` writes the four files and the `screen-server` finding.
- `cases.rs`, since L8: `Step` and `Value` are the steps of a screen's example, in order
  (`State`, `Click`, `Type into`, `Expect state`, `Expect text`), filled for a drawn case;
  `behaviours.rs` records them (`screen_step`, `state`).
- `lib.rs`, since L8: `screen_checks` (with `named`) holds each element and each state field of
  a screen's examples to the screens' Rust files; `Front` (with `Front::kit`,
  `Front::wasm_bindgen` and `Front::screen_of`) and `Screen` are in the `Build`.
