# Purpose

The program is a small language model, and it is a native part. It runs on a GPU of the computer
when the build can use one, and on the CPU if not. It reads a prompt on its standard input. It
writes the prompt and the next eight letters that the model gives, one letter at a time. A rule in
the program gives the weights of the model, so the model needs no download.

On its standard error, the program tells which device it uses. The tests do not compare it,
because it is not the same on each computer. The program `desk` starts it beside its own
WebAssembly file.

# Behaviours

## Give the next letters of a prompt

Given a prompt of letters,
When the program runs,
Then it writes the prompt and the eight letters that come after it, from the model.

### Example: two letters

- Standard input: "ab\n"
- Expect standard output: "abfbedbfbe\n"
- Expect exit code: 0

### Example: a word

- Standard input: "hello"
- Expect standard output: "helloeedbfbed\n"
- Expect exit code: 0

### Example: letters in upper case

- Standard input: "Zz\n"
- Expect standard output: "zzbfbedbfb\n"
- Expect exit code: 0

### Example: a space in the prompt

- Standard input: "a b"
- Expect standard output: "a bedbfbedb\n"
- Expect exit code: 0

## Start from nothing

Given an empty prompt,
When the program runs,
Then it writes the eight letters that the model gives after a space.

### Example: no prompt

- Expect standard output: "eedbfbed\n"
- Expect exit code: 0

# Constraints

- Native: a small language model on the GPU, for the fastest local inference
- Reads: standard input
- Memory: 256 MiB
- Time: 20 s
- Uses crate: `candle-core` 0.11.1, for the tables of numbers of the model, on the GPU or the CPU
- Uses crate: `candle-core` 0.11.1 with `metal` on macOS, for the GPU of a Mac

# Structure

- `main`: reads the prompt, finds the device, and writes the text that the model gives.
- `device`: finds a GPU that the build can use, or the CPU.
- `token`: gives the number of a letter, or the number of the space for any other character.
- `letter`: gives the letter of a number.
- `weight`: gives one weight of the model from the rule.
- `weights`: makes the two tables of the model on the device.
- `next`: gives the next letter from the last two letters.
- `highest`: gives the place of the highest score, the first one when two are equal.
- `generate`: gives the prompt and the eight letters after it.
