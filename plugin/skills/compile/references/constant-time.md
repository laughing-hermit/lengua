# Secrets: the secret rule

An idea's `- Secret:` line names inputs whose values must not decide how the program runs:

```markdown
# Constraints
- Uses: standard input and the environment
- Secret: standard input and the environment variable `TOKEN`
```

For such an idea, `check` adds the secret rule to [the WebAssembly dialect](power-of-10-webassembly.md).
It is a data-flow rule, which `check` holds `main.wat` to as it would a type: it follows every
value computed from a secret input, and refuses `main.wat` where one decides what the program does
next. Each refusal is on its line, `secrets main.wat:LINE: …`, with what to write instead.

What the rule covers, and what it doesn't:
- It checks `main.wat`, the WebAssembly. The machine code the engine compiles it to is trusted to
  keep the WebAssembly's timing: secrets are followed in the WebAssembly's instructions, not in
  the CPU's, until lengua checks the CPU's too (its roadmap's W13).
- It says what `main.wat` doesn't do with a secret. It isn't a claim that a program resists an
  attacker, and don't present it to the user as one.

## What is secret

The bytes a host call writes from a secret input are secret, and so is every value computed from
them: through the operand stack, locals, globals, memory and calls.

| The idea names | Secret in `main.wat` |
|---|---|
| `standard input` | what `fd_read` and `$fill` read from descriptor 0, or from a descriptor the rule can't show isn't 0 |
| ``a file `key.txt` `` | what `fd_read` and `$fill` read from any descriptor that may be an opened one's, 5 or more: the rule can't tell one opened file from another, nor from a connection, since a descriptor comes back through memory |
| ``the environment variable `TOKEN` `` | every variable's text, as `environ_get` writes them into one buffer |
| ``argument `2` `` | every argument's text, as `args_get` writes them into one buffer |

A secret that leaves may come back. Once `main.wat` writes a secret to a descriptor that may be a
file's or a connection's (5 or more, or one the rule can't bound), or sends one with `sock_send`,
anywhere in the program, what `fd_read`, `$fill` and `sock_recv` read from every file and
connection is secret too: the file or the peer may give the secret back, in this run or a later
one. A secret written to standard output or standard error alone changes nothing. So write a secret
to descriptor 1 or 2 by its named constant, and in a program that must send a secret to a file or a
peer, branch on nothing it reads from one.

What isn't secret: how many bytes `$fill` read when it reads to the end, how many a read of a file
gave, every call's error number, the sizes `args_sizes_get` and `environ_sizes_get` give, and where
`args_get` and `environ_get` put each argument or variable (the pointers). A secret's length is
public: lengths, counts and positions are free to decide anything. One count is secret: how many
bytes a single `fd_read` of a secret standard input gave. A terminal gives a line at a time, so
that count says where a line of the secret ends. Read a secret standard input with `$fill` and
`(global.get $READ_TO_END)`, whose count is the input's whole length.

The rule follows memory as the memory rules do, by region and offset. A store of a secret marks the
bytes it writes; a store of a value that isn't secret, at a constant offset, makes them public
again, and so does a `memory.fill` or a `memory.copy` at a constant offset whose length is a
constant too: one whose length varies may write fewer bytes, so it makes none public. So keep
secret data in regions of its own, and fill each iovec at constant offsets in the function that
calls `fd_read`: then the rule knows which buffer the read fills.

An access the memory rules leave to the proof may touch any region, and the bytes outside every
region: one through an address loaded from memory, one at an index checked against a limit the
rules don't follow (a limit loaded from memory), and one of a length they don't bound. With a
secret anywhere, what such an access reads is secret; a secret such an access stores makes every
byte of memory secret, and no later store makes all of it public again. So reach secret data
through a region's global and an index checked against a named constant.

A function is followed for each way it's called: which arguments are secret, what the addresses
they lend point to, and what memory holds. A helper called once with a secret and once without is
refused only where the secret reaches.

## What the rule refuses

| `main.wat` does this with a secret | Write instead |
|---|---|
| the condition of `if`, `br_if` or `br_table` | compute both outcomes and choose with a mask |
| the condition of `select` (an engine may compile it to a branch) | a mask |
| a branch that repeats or leaves a loop: the loop's count | run the loop a public count, a length or a named constant, and fold each step in |
| the address or index of a load or a store | read every place the secret may choose, and keep the one it names with a mask |
| the address or length of `memory.copy` or `memory.fill` | copy a public length, and choose what the bytes hold with a mask |
| an operand of a division or a remainder | shifts and masks, or a multiplication by a constant |
| an operand of any float operator (`f32.`, `f64.`, `f32x4.`, `f64x2.`, and conversions to or from them) | integers |
| an argument of a host call or of lengua's library: a descriptor, an address, a length, a flag, an exit code | public values only |
| a buffer a host call reads: an iovec array (its addresses and lengths), a path, a wait | public values only |
| `$fill` reading a secret input up to a stop byte, which it compares with the last byte of each read, so a secret byte decides whether it reads again and how many bytes it gives | `$fill` with `(global.get $READ_TO_END)`, -1, and the end of the data found from its length |

What passes: arithmetic, `and`, `or`, `xor`, shifts and rotations by any amount, comparisons whose
result is a value (`i32.eq`, `i32.lt_u`, `i32.eqz`), multiplication, the integer SIMD128
operators, `v128.bitselect`, loads and stores at public addresses, and the data `fd_write`,
`sock_send` and `$write_all` write out, secret or not (a secret written to a file or sent to a peer
makes what the program reads from files and connections secret, as above).

What the rule can't follow it refuses: a call that leads back to its caller, and blocks and calls
nested more than 256 deep.

## Masks

A comparison gives 1 or 0; subtracted from 0 it is a mask, all ones or all zeros:

```wat
    ;; 1 when the two words are equal, else 0; then all ones or all zeros.
    (local.set $mask
      (i32.sub (i32.const 0) (i32.eqz (i32.xor (local.get $a) (local.get $b)))))
```

A mask chooses between two values with no branch, `$x` where it's all ones and `$y` where it's 0:

```wat
    (i32.or
      (i32.and (local.get $x) (local.get $mask))
      (i32.and (local.get $y) (i32.xor (local.get $mask) (global.get $ALL_ONES))))
```

`v128.bitselect` does the same for 16 bytes at once, and an `i8x16.eq` gives a mask for each lane.

- **Equality of two buffers:** fold every byte's `xor` into one word with `or`, for the whole
  length, then make the word a mask once with `i32.eqz`: the loop runs the public length whatever
  the bytes are. Never leave at the first difference.
- **Order:** `i32.lt_u` and the other comparisons give 0 or 1 as a value, which passes; it's only
  a branch on that value that the rule refuses, so make it a mask instead.
- **A table entry the secret names:** read every entry, and keep the one whose index equals the
  secret with a mask made from `i32.eq`: the loads' addresses are public, the choice isn't seen.
- **Output that depends on a secret:** give every answer the same length, and choose its bytes
  with a mask, as secret-equal chooses between `same` and `diff`; then write it with
  `$write_all`, whose length is public. The exit code is a host call's argument, so it can't
  depend on a secret either.
- **A secret's end:** find it from a public length, never by looking for a byte. A line's
  newline, found with `br_if` on each byte, is a branch on the secret.

secret-equal (lengua's `examples/secret-equal`) is a whole program by these rules: it
reads two lines from standard input, which is secret, finds them from the input's length, folds
the `xor` of every pair of bytes into one word in a loop of public count, and chooses its answer
from that word with a mask:

```wat
          (local.set $mask
            (i32.sub (i32.const 0) (i32.eqz (call $differ (local.get $half)))))
          (i32.store (global.get $out)
            (i32.or
              (i32.and (i32.load (global.get $same)) (local.get $mask))
              (i32.and (i32.load (global.get $diff))
                (i32.xor (local.get $mask) (global.get $ALL_ONES)))))
```

## Assertions and the other rules

Every rule of the dialect still holds. An assertion's condition is a branch, so assert only what
isn't secret: lengths, counts and indexes. Bounds checks on indexes are public too, as indexes are.
Rule 2's loops already run from a named constant; a secret only changes a loop's count through a
branch out of it, which the secret rule refuses.
