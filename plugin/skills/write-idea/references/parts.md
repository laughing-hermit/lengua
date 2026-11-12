# Parts in an idea

A part is a lengua program that offers some of its functions to other programs. An idea that
builds on a part names it, and the compile skill then takes the part's functions into `main.wat`,
word for word, where `check` holds each copy to the part. This page says what the idea needs.

## Build on a part

Look for a part before you specify what a program must do for itself. lengua's library is in
the compile skill's `assets/parts` folder: `decimal` reads and writes whole numbers in decimal,
`lines` reads an input into memory and finds its lines, `crc32` computes the CRC-32 of bytes,
`bytes` says if two runs of bytes are the same, and `sort64` sorts whole numbers. Each is a
folder with its own `idea.md` and `main.wat`. A program of the user's own can be a part too,
once its idea has a contract.

1. **Read the part's `# Contract` section**, in its `idea.md`. Each item is one function: what
   it reads and writes through the addresses it is lent, what it gives, and its limits, such as
   the longest text it takes.
2. **Put the part's folder beside the program's**, in one bundle, with each part that part
   builds on: `lines` and `crc32` build on `decimal`, `bytes` on `lines`, and `sort64` on
   `lines` and `decimal`.
3. **Name each part under `# Constraints`**, by a path relative to this idea:

   ```markdown
   # Constraints
   - Uses: standard input
   - Builds on: [lines](../lines/idea.md), [decimal](../decimal/idea.md)
   ```

4. **Take the part's limits into `# Purpose`**, in backticks, as the program's own: a program
   that finds its lines with `lines` takes input of at most `4096` bytes, and says what it does
   with more. The generated tests draw from those numbers.
5. **Give the idea what the part needs**: `lines` reads, so the idea's `- Uses:` line names
   `standard input`.

A part that doesn't fit, since its limit is too small or it does something else, isn't built
on: the program does that work itself, and its idea says so in the same words as any other.
A cycle of `- Builds on:` links, such as a part that builds on the program, blocks every program
in it.

## Write a part

A part is a whole program, with its own behaviors and tests. Choose a small program that uses
every function the part offers, so that its tests and its proofs go through them: `decimal`
prints the negative of a number, which reads it and writes it.

Its idea has one more section, `# Contract`, after `# Constraints`, with an item for each
function a program may take:

```markdown
# Contract
- `$decimal_format`: writes 20 bytes at `$into`. Its arguments are `$value`, as 64 bits, and
  `$into`. It writes `$value` in decimal at `$into` and gives the number of bytes it wrote.
```

- The item starts with the function's name in backticks and a colon. Then come its clauses,
  with `;` between them and a full stop after the last: ``reads N bytes at `$param` ``,
  ``writes N bytes at `$param` `` or `uses no memory`.
- After the full stop, say in plain words what each argument is, what the function gives, and
  which arguments stop the program.
- `check` holds each clause to `main.wat`, and says the clause to write when they differ, so
  write the section with the compile skill, once the functions exist.
- Where a function gives a number that a formula of the test language states, as
  `$crc32_update` gives `crc32(at)`, the item can state the formula too, with `takes`,
  `holds` and `gives` clauses: the compile skill's
  [parts.md](../../compile/references/parts.md) has their form. `check` proves the formula of
  the function alone, and the proofs of a program built on the part use it at each call.
- State the part's limits in `# Purpose` too, in backticks: its own tests draw from them.
