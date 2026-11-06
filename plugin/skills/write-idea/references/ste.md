# Simplified Technical English in idea files

An idea is written in ASD-STE100 Simplified Technical English (STE). STE is the controlled
English that ASD, the association of Europe's aerospace and defence industries, made for
technical documents: a limited set of words, each with one meaning, and rules for how to put
them together. A reader for whom English is a second language understands STE, and an agent or a
translation tool reads it one way. An idea is the source of a program and the record of what the
user asked for, so it must say one thing, the same to every reader.

ASD gives the standard away. Issue 9 (January 2025), with its dictionary of approved words, is
free to download from [ASD's STE page](https://www.asd-europe.org/standards-specifications/simplified-technical-english/download-asd-ste-100).
The standard is ASD's copyrighted work, so lengua states its rules in its own words here and
copies nothing of the dictionary. When you are not sure of a word, the dictionary decides.

## The rules, as lengua applies them

### Words

- Use simple, common words. Give each word one meaning only, and use the same word for the same
  thing every time: an idea that says "folder" once never says "directory".
- Technical names are words too, with their technical meaning: standard input, standard output,
  standard error, file, folder, argument, environment variable, exit code, connection, port,
  byte, newline, and the names of the user's own field, such as "invoice" or "checksum".
- Use a verb, not a noun made from it: "it sorts the lines", not "it does a sort of the lines".

### Noun clusters

Put at most three nouns in a row. Break a longer cluster with "of", "for" or a clause: "the
limit for the length of a line", not "the line length limit value".

### Verbs

- Use the simple present for what the program does ("it prints", "it exits"), the simple past
  for what happened before ("the user gave"), and the simple future only where you need it.
- Use the imperative for an instruction ("Give the name of the file").
- Use a past participle only as an adjective: "a closed output", "the sorted lines".
- Do not use the -ing form of a verb. Write "it reads the file", not "reading the file", and
  "when the program cannot write", not "when writing fails". An -ing word that is a technical
  name stays: string, padding, encoding, floating point.
- Use the active voice: "the program makes the file", not "the file is made". Use the passive
  only when nobody knows who acts.

### Sentences

- Write one idea in each sentence.
- A sentence that describes has at most 25 words, and an instruction at most 20.
- Keep the articles ("a", "the"), the verbs and the subjects: do not drop them to make a
  sentence shorter. Write "It prints the sum", not "Prints sum".
- Use a list, not a long sentence, for several items or cases.

### Paragraphs

Give each paragraph one topic and at most six sentences, and start it with the sentence that
names the topic. Use a list for complex text, such as the cases of an error.

### Behaviors

The idea follows behavior-driven development. Each behavior is a `## B1:` heading whose title is
in the simple present ("Adds two numbers", "Rejects a line that is too long"), then its Given,
When and Then steps. Each step is one short STE sentence in lengua's step words, below. The title
says the rule, and the steps show one example of it.

## What `check` enforces

A computer cannot check every STE rule, so `check` checks these, and the rest is yours to keep.
It reads the prose of the idea: the frontmatter's `description`, the paragraphs of `# Purpose`,
the text of each `## B1:` heading, and every step (`- Given`, `- When`, `- Then`, `- And` and
`- But`) with the lines that go on with it. It does not read code blocks, tables, the `###`
headings and the prose under them, or `# Constraints`.

1. **Values.** A value in backticks is one word, whatever it holds, and its words are not prose.
2. **Sentence length.** A sentence has at most 25 words. A word is anything between spaces, and
   a number is a word. A sentence ends at `.`, `!` or `?` before a space or the end of the line,
   or where its paragraph, title or step ends, so a sentence can go on over several lines.
3. **Paragraph length.** A paragraph has at most six sentences. A paragraph is its lines up to a
   blank line, and each item of a list is a paragraph of its own.
4. **-ing forms.** A word of 5 letters or more that ends in "ing" is refused, except these
   technical names and words that are not verbs: string, strings, nothing, something, anything,
   everything, during, spring, ceiling, morning, evening, building, heading, padding, encoding,
   encodings, rounding, bring, thing, things, and "floating" just before "point".
5. **Old step words.** A step uses none of lengua's old phrasings, [below](#the-step-words).

Each problem starts with `STE:`, names its line, and says what to write:

```text
line 7: STE: this sentence has 31 words, more than the 25 STE allows: put one idea in each sentence, and make two or more sentences of this one
line 12: STE: `reading` is an -ing form. STE allows -ing forms only in technical names: say it with a simple verb, as in `it reads` for `reading`
line 15: STE: write `standard input that contains `…``, not `standard input holding `…``
```

STE's problems are problems in the idea: `check` records the idea only when they are fixed, as
for any other problem.

## The step words

lengua reads each STE phrasing exactly as the old phrasing meant, and refuses the old one with
the STE phrasing as its fix:

| Write | Not |
|---|---|
| ``standard input that contains `…` `` | ``standard input holding `…` `` |
| ``a file `notes.txt` that contains `…` `` | ``a file `notes.txt` holding `…` ``, ``a file named `notes.txt` holding `…` `` |
| ``the environment variable `NAME` with the value `…` `` | ``the environment variable `NAME` set to `…` `` |
| ``it prints text that matches `rule` `` | ``it prints text matching `rule` `` |
| ``the file `out.txt` contains `…` `` | ``the file `out.txt` holds `…` `` |
| ``the client receives `…` `` | ``the client gets back `…` `` |
| `it does not stop` | `it keeps serving` |
| ``no server on port `8080` `` | ``nothing listening on port `8080` `` |
| `` `…` and a newline`` | `` `…` followed by a newline`` |
| `` `…` then `…` `` | `` `…` followed by `…` `` |
| `` `a` `4097` times`` | `` `a` repeated `4097` times`` |

The other phrasings were STE already and stay as they are: `the arguments … and …`, `no
arguments`, `no standard input`, `empty standard input`, `an empty file`, `no file`, `an empty
folder`, `the clock at …`, `then …`, `the random bytes … and …`, `a server on port … that
receives … and replies …`, `at …`, `a client that sends …`, `the clients connect at once`,
`standard output is closed`, `the program runs`, `it prints …`, `to standard error`, `it prints
nothing`, `there is no file …`, `there is no folder …`, `there is a folder …` and `it exits with
code …`. [The test format](test-format.md#the-steps) says what each one gives the program or
expects of it.

## Words to change

Words that ideas often use and STE refuses, with what to write instead:

| Not | Write |
|---|---|
| reading, writing | it reads, it writes; "if the program cannot read its input" |
| missing | no: "when there is no argument" |
| leading, trailing | at the start, at the end: "spaces at the end of a line" |
| using | with |
| including | with, and also |
| containing | that contains |
| matching | that matches |
| existing | that exists |
| remaining | the rest of, the other |
| following | next, after |
| starting, ending | that starts, that ends; at the start, at the end |
| waiting | it waits |
| counting, sorting | it counts, it sorts |

## Before and after

A behavior's title and a step:

```text
## B3: Rejecting a number that is too large
- Then it prints `error: too large` followed by a newline to standard error
```

```text
## B3: Rejects a number that is too large
- Then it prints `error: too large` and a newline to standard error
```

A paragraph of `# Purpose`:

```text
Counts the words of standard input, ignoring leading and trailing spaces, and prints the count
followed by a newline; if reading fails, the program exits with code 3.
```

```text
Counts the words on standard input and prints the count. Spaces at the start and at the end of a
line are not words. If the program cannot read its input, it exits with code 3.
```

Given steps:

```text
- Given a file `in.csv` holding `a,1` followed by a newline then `b,2`
- And the environment variable `SEP` set to `,`
```

```text
- Given a file `in.csv` that contains `a,1` and a newline then `b,2`
- And the environment variable `SEP` with the value `,`
```
