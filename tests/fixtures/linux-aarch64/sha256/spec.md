---
type: lengua/program-spec
title: sha256
description: Prints the SHA-256 digest of standard input.
---
# Purpose
Prints the SHA-256 digest of standard input, as FIPS 180-4 defines it: 64 lowercase hexadecimal
digits followed by a newline. It reads standard input of any length a piece at a time. Tests give
it up to `1048576` bytes. If reading the input or writing the output fails, the program exits with
code 2.

# Behaviors
## B1: Prints the digest of the standard examples
- Given standard input holding `abc`
- When the program runs
- Then it prints `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad` followed by a newline
- And it exits with code `0`

## B2: Prints the digest of empty input
- Given empty standard input
- When the program runs
- Then it prints `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` followed by a newline
- And it exits with code `0`

## B3: Prints the digest of a message that needs a second block of padding
- Given standard input holding `abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq`
- When the program runs
- Then it prints `248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1` followed by a newline
- And it exits with code `0`

## B4: Prints the digest of any input
- Given standard input holding `hello\n`
- When the program runs
- Then it prints `5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03` followed by a newline
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{sha256(text)}` followed by a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "0123456789abcdefghijklmnopqrstuvwxyz \n")` | 100 |
| `string(50, 70, "abcdefghijklmnopqrstuvwxyz")` | 300 |
| `string(118, 134, "abcdefghijklmnopqrstuvwxyz")` | 300 |
| `string(4090, 4102, "abcdefghijklmnopqrstuvwxyz")` | 300 |

# Constraints
- Uses: standard input
