---
type: lengua/program-spec
title: aes
description: Encrypts blocks of hexadecimal data with AES-128 in ECB mode, using the CPU's AES instructions.
---
# Purpose
Encrypts data with AES-128 (FIPS 197) and shows how lengua checks a program built on the CPU's
AES instructions. Standard input holds two lines: the first is the 128-bit key, 32 hexadecimal
digits; the second is the data, from `32` to `131072` hexadecimal digits in a multiple of 32, that
is 1 to 4096 blocks of 16 bytes. The last newline may be missing, and hexadecimal digits may be
lowercase or uppercase. The program encrypts each block separately with the key (ECB mode, as the
standard's known-answer examples do, which is not a mode for protecting real data) and prints the
ciphertext as lowercase hexadecimal digits followed by a newline. Input that isn't of this form
makes it print `aes: bad input` to standard error and exit with code 3. If reading the input or
writing the output fails, the program exits with code 2.

# Behaviors
## B1: Encrypts the example of FIPS 197's appendix C.1
- Given standard input holding `000102030405060708090a0b0c0d0e0f\n00112233445566778899aabbccddeeff\n`
- When the program runs
- Then it prints `69c4e0d86a7b0430d8cdb78070b4c55a` followed by a newline
- And it exits with code `0`

## B2: Encrypts the example of FIPS 197's appendix B
- Given standard input holding `2b7e151628aed2a6abf7158809cf4f3c\n3243f6a8885a308d313198a2e0370734\n`
- When the program runs
- Then it prints `3925841d02dc09fbdc118597196a0b32` followed by a newline
- And it exits with code `0`

## B3: Encrypts four blocks, as NIST SP 800-38A's ECB-AES128 example does
- Given standard input holding `2b7e151628aed2a6abf7158809cf4f3c\n6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e5130c81c46a35ce411e5fbc1191a0a52eff69f2445df4f9b17ad2b417be66c3710\n`
- When the program runs
- Then it prints `3ad77bb40d7a3660a89ecaf32466ef97f5d3d58503b9699de785895a96fdbaaf43b1cd7f598ece23881b00e3ed0306887b0c785e27e8ad3f8223207104725dd4` followed by a newline
- And it exits with code `0`

## B4: Encrypts a block of zeros with a key of zeros
- Given standard input holding `00000000000000000000000000000000\n00000000000000000000000000000000\n`
- When the program runs
- Then it prints `66e94bd4ef8a2c3b884cfa59ca342b2e` followed by a newline
- And it exits with code `0`

## B5: Takes uppercase digits and a missing last newline
- Given standard input holding `000102030405060708090A0B0C0D0E0F\n00112233445566778899AABBCCDDEEFF`
- When the program runs
- Then it prints `69c4e0d86a7b0430d8cdb78070b4c55a` followed by a newline
- And it exits with code `0`

## B6: Rejects input that isn't a key and blocks of hexadecimal digits
- Given standard input holding `000102030405060708090a0b0c0d0e0f\n00112233445566778899aabbccddee\n`
- When the program runs
- Then it prints `aes: bad input` followed by a newline to standard error
- And it exits with code `3`

### For any `bad`
- Given standard input holding `{bad}`
- When the program runs
- Then it prints `aes: bad input` followed by a newline to standard error
- And it exits with code `3`

| bad | runs |
|---|---|
| `choice("", "\n", "00", "000102030405060708090a0b0c0d0e0f", "000102030405060708090a0b0c0d0e0f\n", "000102030405060708090a0b0c0d0e0f\n\n", "000102030405060708090a0b0c0d0e0f 00112233445566778899aabbccddeeff", "000102030405060708090a0b0c0d0e\n00112233445566778899aabbccddeeff\n", "000102030405060708090a0b0c0d0e0f\n00112233445566778899aabbccddeef\n", "000102030405060708090a0b0c0d0e0f\n00112233445566778899aabbccddeeffaa\n", "000102030405060708090a0b0c0d0e0f\n00112233445566778899aabbccddeefg\n", "g00102030405060708090a0b0c0d0e0f\n00112233445566778899aabbccddeeff\n", "000102030405060708090a0b0c0d0e0f\r\n00112233445566778899aabbccddeeff\n")` | 13 |

# Constraints
- Uses: standard input
