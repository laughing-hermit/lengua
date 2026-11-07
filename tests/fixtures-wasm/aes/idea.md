---
type: Attested Computation
title: aes
description: Encrypts blocks of hexadecimal data with AES-128 in ECB mode, in portable code.
runtime: webassembly
computation: main.wat
parameters:
  - { name: input, type: text, required: true, from: standard input }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
This program encrypts data with AES-128 (FIPS 197). It computes AES in portable code, the same on
every computer, with arithmetic on the bytes alone. It looks up no table with a byte of the key or
of the data. So the values of the key and the data do not change which memory the encryption
reads. The encryption has no branch that depends on them.

Standard input has two lines. The first line is the 128-bit key, `32` hexadecimal digits. The
second line is the data, from `32` to `131072` hexadecimal digits in a multiple of 32. That is 1
to 4096 blocks of 16 bytes. The last newline can be absent, and the digits can be lowercase or
uppercase.

The tests use data of `32` and of `128` digits, which is one block and four blocks. For input that
is not of this form, they use keys of `30` digits. They also use data of `0` to `30` digits or of
`33` digits.

The program encrypts each block separately with the key. This is ECB mode, as in the known-answer
examples of the standard. ECB mode is not a safe mode for real data. The program prints the
ciphertext as lowercase hexadecimal digits and a newline.

If the input is not of this form, the program prints `aes: bad input` to standard error and exits
with code 3. If the program cannot read its input or write its output, it exits with code 2.

# Behaviors
## B1: Encrypts the example in appendix C.1 of FIPS 197
- Given standard input that contains `000102030405060708090a0b0c0d0e0f\n00112233445566778899aabbccddeeff\n`
- When the program runs
- Then it prints `69c4e0d86a7b0430d8cdb78070b4c55a` and a newline
- And it exits with code `0`

### For any `key` and `data`
- Given standard input that contains `{key}\n{data}\n`
- When the program runs
- Then it prints `{aes128(key, data)}` and a newline
- And it exits with code `0`

| key | data | runs |
|---|---|---|
| `string(32, 32, "0123456789abcdef")` | `string(32, 32, "0123456789abcdef")` | 200 |

## B2: Encrypts the example in appendix B of FIPS 197
- Given standard input that contains `2b7e151628aed2a6abf7158809cf4f3c\n3243f6a8885a308d313198a2e0370734\n`
- When the program runs
- Then it prints `3925841d02dc09fbdc118597196a0b32` and a newline
- And it exits with code `0`

### For any `key` and `data`
- Given standard input that contains `{key}\n{data}\n`
- When the program runs
- Then it prints `{aes128(key, data)}` and a newline
- And it exits with code `0`

| key | data | runs |
|---|---|---|
| `string(32, 32, "0123456789abcdef")` | `string(32, 32, "0123456789abcdef")` | 200 |

## B3: Encrypts four blocks, as the ECB-AES128 example of NIST SP 800-38A does
- Given standard input that contains `2b7e151628aed2a6abf7158809cf4f3c\n6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e5130c81c46a35ce411e5fbc1191a0a52eff69f2445df4f9b17ad2b417be66c3710\n`
- When the program runs
- Then it prints `3ad77bb40d7a3660a89ecaf32466ef97f5d3d58503b9699de785895a96fdbaaf43b1cd7f598ece23881b00e3ed0306887b0c785e27e8ad3f8223207104725dd4` and a newline
- And it exits with code `0`

### For any `key` and `data`
- Given standard input that contains `{key}\n{data}\n`
- When the program runs
- Then it prints `{aes128(key, data)}` and a newline
- And it exits with code `0`

| key | data | runs |
|---|---|---|
| `string(32, 32, "0123456789abcdef")` | `string(128, 128, "0123456789abcdef")` | 200 |

## B4: Encrypts a block of zeros with a key of zeros
- Given standard input that contains `00000000000000000000000000000000\n00000000000000000000000000000000\n`
- When the program runs
- Then it prints `66e94bd4ef8a2c3b884cfa59ca342b2e` and a newline
- And it exits with code `0`

### For any `key` and `data`
- Given standard input that contains `{key}\n{data}\n`
- When the program runs
- Then it prints `{aes128(key, data)}` and a newline
- And it exits with code `0`

| key | data | runs |
|---|---|---|
| `string(32, 32, "0123456789abcdef")` | `string(32, 32, "0123456789abcdef")` | 200 |

## B5: Accepts uppercase digits and input with no last newline
- Given standard input that contains `000102030405060708090A0B0C0D0E0F\n00112233445566778899AABBCCDDEEFF`
- When the program runs
- Then it prints `69c4e0d86a7b0430d8cdb78070b4c55a` and a newline
- And it exits with code `0`

### For any `key` and `data`
- Given standard input that contains `{key}\n{data}`
- When the program runs
- Then it prints `{aes128(key, data)}` and a newline
- And it exits with code `0`

| key | data | runs |
|---|---|---|
| `string(32, 32, "0123456789ABCDEF")` | `string(32, 32, "0123456789ABCDEF")` | 200 |

## B6: Rejects input that is not a key and blocks of hexadecimal digits
- Given standard input that contains `000102030405060708090a0b0c0d0e0f\n00112233445566778899aabbccddee\n`
- When the program runs
- Then it prints `aes: bad input` and a newline to standard error
- And it exits with code `3`

### For any `key` and `data`
- Given standard input that contains `{key}\n{data}\n`
- When the program runs
- Then it prints `aes: bad input` and a newline to standard error
- And it exits with code `3`

| key | data | runs |
|---|---|---|
| `string(32, 32, "0123456789abcdef")` | `string(0, 30, "0123456789abcdef")` | 100 |
| `string(30, 30, "0123456789abcdef")` | `string(32, 32, "0123456789abcdef")` | 100 |
| `string(32, 32, "0123456789abcdef")` | `string(33, 33, "0123456789abcdef")` | 100 |
| `string(32, 32, "g")` | `string(32, 32, "0123456789abcdef")` | 100 |

# Constraints
- Uses: standard input
