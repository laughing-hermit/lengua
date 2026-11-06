---
type: lengua/program-idea
title: sha256
description: Prints the SHA-256 digest of standard input.
---
# Purpose
The program prints the SHA-256 digest of standard input, as FIPS 180-4 defines it. The digest is
64 lowercase hexadecimal digits and a newline. The program reads standard input of any length, one
piece at a time.

The tests give it up to `1048576` bytes, and messages of `0` to `64` bytes. Other tests try the
edges of its padding and of its pieces. They give it messages of exactly `56` bytes, and of `50`
to `70`, `118` to `134` and `4090` to `4102` bytes. If the program cannot read its input or write
its output, it exits with code 2.

# Behaviors
## B1: Prints the digest of the standard examples
- Given standard input that contains `abc`
- When the program runs
- Then it prints `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{sha256(text)}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 64, "abc")` | 100 |

## B2: Prints the digest of empty input
- Given empty standard input
- When the program runs
- Then it prints `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` and a newline
- And it exits with code `0`

## B3: Prints the digest of a message that needs a second block of padding
- Given standard input that contains `abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq`
- When the program runs
- Then it prints `248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{sha256(text)}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(56, 56, "abcdefghijklmnopq")` | 100 |

## B4: Prints the digest of any input
- Given standard input that contains `hello\n`
- When the program runs
- Then it prints `5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{sha256(text)}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "0123456789abcdefghijklmnopqrstuvwxyz \n")` | 100 |
| `string(50, 70, "abcdefghijklmnopqrstuvwxyz")` | 300 |
| `string(118, 134, "abcdefghijklmnopqrstuvwxyz")` | 300 |
| `string(4090, 4102, "abcdefghijklmnopqrstuvwxyz")` | 300 |

# Constraints
- Uses: standard input
