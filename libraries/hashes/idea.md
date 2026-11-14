---
type: lengua Library
description: "The library gives the SHA - 256 hash and the CRC - 32 value of bytes."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: baddaca4c73ce45ebeeab840ea91609fda77131a8b5bdb4abc027836dc5c1d46
source_sha256: a3905c876e2530a6623fc371bf3ca67c3ab6bf0eb307dd0141bf332b6268936e
cargo_sha256: 890fdf2c6e2572c0d927bc0dfc0eefeff7350c267296e1c5301e215d2b259841
world_sha256: 43e5e032ac9bee5f73ecde94bf9f71093003bcc169f5f7d6e9e2310be5b0b626
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T06:04:17Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T06:04:17Z }
title: hashes
status: draft
---

# Purpose

The library gives the SHA-256 hash and the CRC-32 value of bytes. It is a `lengua` library on the
standard library only, so a program can use it in place of a crate. The values are the values
of the standards: FIPS 180-4 for SHA-256, and the CRC-32 of ISO 3309 and `zlib`.

# Behaviours

## Hash no bytes

Given no bytes,
When a program calls `sha256_hex`,
Then it gives the hash of no bytes as 64 hexadecimal digits.

### Example: no bytes

- Call: `sha256_hex(b"")`
- Expect value: `"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"`

## Hash three letters

Given the bytes `abc`,
When a program calls `sha256_hex`,
Then it gives the hash that FIPS 180-4 gives for `abc`.

### Example: the bytes `abc`

- Call: `sha256_hex(b"abc")`
- Expect value: `"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"`

## Hash a message of two blocks

Given the 56 bytes of the second message of FIPS 180-4,
When a program calls `sha256_hex`,
Then it gives the hash of the two blocks.

### Example: 56 bytes

- Call: `sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")`
- Expect value: `"248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"`

### Example: 64 bytes

- Call: `sha256_hex(&[b'a'; 64])`
- Expect value: `"ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"`

## Give the hash as bytes

Given the bytes `abc`,
When a program calls `sha256`,
Then it gives 32 bytes, and the first byte is 186.

### Example: the first and the last byte

- Call: `{ let hash = sha256(b"abc"); (hash.len(), hash[0], hash[31]) }`
- Expect value: `(32, 186, 173)`

## Find the CRC-32 value

Given the nine digits `123456789`,
When a program calls `crc32`,
Then it gives the check value of CRC-32, 3421780262.

### Example: nine digits

- Call: `crc32(b"123456789")`
- Expect value: `3421780262`

### Example: one letter

- Call: `crc32(b"a")`
- Expect value: `3904355907`

### Example: no bytes

- Call: `crc32(b"")`
- Expect value: `0`

# Constraints

- Memory: 16 MiB
- Time: 5 s
- Exports: `sha256`, `sha256_hex`, `crc32`

# Structure

- `sha256`: gives the SHA-256 hash of bytes, as 32 bytes.
- `sha256_hex`: gives the SHA-256 hash of bytes, as 64 hexadecimal digits in lower case.
- `padded`: gives the message with the `0x80` byte, the zero bytes and the length in bits, so
  that its length is a multiple of 64.
- `schedule`: gives the 64 words of the schedule of one block.
- `compress`: adds one block of 64 bytes to the state of the hash.
- `crc32`: gives the CRC-32 value of bytes, with the polynomial `0xedb88320`.

# Links

* Index: [hashes](index.md).
