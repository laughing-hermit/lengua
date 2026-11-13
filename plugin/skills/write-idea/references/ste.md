# STE: what check holds

Write every sentence of `# Purpose`, the `Given`, `When` and `Then` lines and the
`# Structure` sentences in ASD-STE100 Simplified Technical English. `check` refuses a
sentence that breaks one of these rules, and says which.

| Rule | Code |
|---|---|
| Use only words of lengua's STE dictionary, or their regular forms (`-s`, `-ed`) | `ste-word` |
| Use the approved word, not the unapproved one: `use`, not `utilize`; `about`, not `approximately`; `start`, not `commence` | `ste-word-unapproved` |
| Do not use the `-ing` form of a verb: write `the program reads`, not `the program is reading` | `ste-word-ing` |
| An instruction has 20 words or fewer | `ste-sentence-length` |
| A description has 25 words or fewer | `ste-sentence-length` |
| A paragraph has six sentences or fewer | `ste-paragraph-length` |
| One instruction in each sentence | `ste-one-instruction` |
| Use the active voice: `the program reads the file`, not `the file is read by the program` | `ste-passive` |
| Use the simple tenses: `the program wrote`, not `the program has written` | `ste-tense` |
| Do not put more than three nouns in a row | `ste-noun-cluster` |

## Technical names

These are always allowed:

- a name in backticks: `` `stdout` ``, `` `Cart::add` ``, `` `data/in.txt` ``;
- a string in double quotes;
- a number, and a word with a digit or `_` in it;
- a word with a capital letter after its first letter: `WebAssembly`, `JSON`;
- a word with a capital first letter that does not start the sentence: `Rust`, `Linux`.

When `check` refuses a technical word, put it in backticks. When it refuses a general word,
use a simpler one: `check`'s message names the word.

## What check does not hold

That each word has its one approved meaning, articles, and good style. Write short, clear
sentences, one idea each, with the subject first.
