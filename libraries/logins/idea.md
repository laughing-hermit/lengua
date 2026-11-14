---
type: lengua Library
description: "The library keeps the logins of a program."
module: main
source: src/main.rs
uses: [../hashes/idea]
used_by: []
label: checked
idea_sha256: 8169b1c552876ac2888f0255e84d0a731d37bc1d5b28454a04cb817b9435612f
source_sha256: e2b0e3b205edd229f6b2ed37160c95c4d9e1042ca6c0a051b56f693296d0620d
cargo_sha256: ff91ffc8456ebd07524899f47e314b3dc0a9530740a83b1b43bebc26d957dece
world_sha256: 43e5e032ac9bee5f73ecde94bf9f71093003bcc169f5f7d6e9e2310be5b0b626
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T07:49:59Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T07:49:59Z }
title: logins
status: draft
---

# Purpose

The library keeps the logins of a program. It makes the hash of a password and compares a
password with a hash. It signs and checks the tokens of a session. It uses the library
`hashes` for SHA-256, so it is a `lengua` library that uses another `lengua` library. The hash
of a password is PBKDF2 with HMAC-SHA256 of RFC 8018, and the HMAC is the HMAC of RFC 2104.

A library has no grants, so the program gives the salt, the key of the tokens and the time. A
program makes the salt from random numbers, reads the key as a secret, and reads the time from
the clock.

# Behaviours

## Sign a message with a key

Given a key and a message,
When a program calls `hmac_sha256_hex`,
Then it gives the HMAC-SHA256 of the message as 64 hexadecimal digits.

### Example: a known message

- Call: `hmac_sha256_hex(b"key", b"The quick brown fox jumps over the lazy dog")`
- Expect value: `"f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"`

### Example: a key of one block

- Call: `hmac_sha256_hex(&[b'a'; 64], b"abc")`
- Expect value: `"6608ac82dca1cb1fddbb5d81e3d9877642b744f565cd9697ac27daa250c80d28"`

### Example: a key longer than one block

- Call: `hmac_sha256_hex(&[b'a'; 65], b"abc")`
- Expect value: `"c0d2f0e7f578e80e4996cf2ffb922ea70fe1094e693f2cd75bbba0281add9da5"`

## Make the hash of a password

Given a password, a salt and a number of rounds,
When a program calls `hash_password`,
Then it gives the name of the method, the rounds, the salt and the hash, with `$` between them.

### Example: one round

- Call: `hash_password("password", "salt", 1)`
- Expect value: `"pbkdf2-sha256$1$salt$120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"`

### Example: two rounds

- Call: `hash_password("password", "salt", 2)`
- Expect value: `"pbkdf2-sha256$2$salt$ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43"`

### Example: no rounds is one round

- Call: `hash_password("password", "salt", 0)`
- Expect value: `"pbkdf2-sha256$1$salt$120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"`

## Compare a password with its hash

Given the hash of a password,
When a program calls `verify_password` with a password,
Then it gives true only for the same password.

### Example: the same password

- Call: `verify_password("secret", "pbkdf2-sha256$3$00112233$4e0dfd9673ea5f07ff0fe734c98c147c26f7d0db792713af81da00de56c7f423")`
- Expect value: `true`

### Example: a wrong password

- Call: `verify_password("Secret", "pbkdf2-sha256$3$00112233$4e0dfd9673ea5f07ff0fe734c98c147c26f7d0db792713af81da00de56c7f423")`
- Expect value: `false`

### Example: a hash of another method

- Call: `verify_password("secret", "md5$3$00112233$4e0dfd9673ea5f07ff0fe734c98c147c26f7d0db792713af81da00de56c7f423")`
- Expect value: `false`

### Example: a hash with too many parts

- Call: `verify_password("secret", "pbkdf2-sha256$3$00112233$4e0d$x")`
- Expect value: `false`

### Example: rounds that are not a number

- Call: `verify_password("secret", "pbkdf2-sha256$x$00112233$4e0d")`
- Expect value: `false`

## Compare two texts

Given two texts,
When a program calls `same`,
Then it gives true when they are equal, and it looks at each letter, so the time does not show
where they are different.

### Cases: equal texts

- Range `word`: text of 0 to 12 characters from "abc"
- Call: `same("{word}", "{word}")`
- Expect value: `true`

### Example: a short text

- Call: `same("ab", "abc")`
- Expect value: `false`

### Example: one letter is different

- Call: `same("abd", "abc")`
- Expect value: `false`

## Sign the token of a session

Given a name, a time and a key,
When a program calls `sign_token`,
Then it gives the name, the time and the HMAC of the two, with a dot between them.

### Example: a token for Ann

- Call: `sign_token("ann", 1767229200, "test-SESSION_KEY")`
- Expect value: `"ann.1767229200.7600c8ab4a9f796bbef41d93945dfd379637ea30660a254e5a52e3b3e33ffa24"`

## Check the token of a session

Given a token, the time now and the key,
When a program calls `check_token`,
Then it gives the name when the signature is good and the time of the token is later than the time now.

### Example: a good token

- Call: `check_token("ann.1767229200.7600c8ab4a9f796bbef41d93945dfd379637ea30660a254e5a52e3b3e33ffa24", 1767225600, "test-SESSION_KEY")`
- Expect value: `Ok("ann")`

### Example: a token at its time

- Call: `check_token("ann.1767229200.7600c8ab4a9f796bbef41d93945dfd379637ea30660a254e5a52e3b3e33ffa24", 1767229200, "test-SESSION_KEY")`
- Expect value: `Err(Expired)`

### Example: a token with another key

- Call: `check_token("ann.1767229200.7600c8ab4a9f796bbef41d93945dfd379637ea30660a254e5a52e3b3e33ffa24", 1767225600, "other")`
- Expect value: `Err(Signature)`

### Example: a token with no time

- Call: `check_token("ann.7600c8ab", 1767225600, "test-SESSION_KEY")`
- Expect value: `Err(Form)`

### Example: a token with no name

- Call: `check_token(&sign_token("", 9, "k"), 1, "k")`
- Expect value: `Err(Form)`

### Example: a time that is not a number

- Call: `check_token("ann.soon.7600c8ab", 1, "k")`
- Expect value: `Err(Form)`

## Write bytes as hexadecimal digits

Given bytes,
When a program calls `to_hex`,
Then it gives two hexadecimal digits in lower case for each byte.

### Example: three bytes

- Call: `to_hex(&[0, 171, 255])`
- Expect value: `"00abff"`

# Constraints

- Memory: 16 MiB
- Time: 5 s
- Exports: `Error`, `to_hex`, `hmac_sha256_hex`, `hash_password`, `same`, `verify_password`, `sign_token`, `check_token`
- Uses: `../hashes`

# Structure

- `hmac`: gives the HMAC-SHA256 of a message with a key, as 32 bytes.
- `to_hex`: writes bytes as hexadecimal digits in lower case.
- `hmac_sha256_hex`: gives the HMAC-SHA256 of a message with a key, as 64 hexadecimal digits.
- `pbkdf2`: gives the 32 bytes of PBKDF2 with HMAC-SHA256 for a password, a salt and rounds.
- `hash_password`: gives the stored form of the hash of a password.
- `same`: compares two texts, letter by letter to their end.
- `verify_password`: compares a password with the stored form of its hash.
- `sign_token`: gives a token with a name, a time and their signature.
- `check_token`: gives the name of a token, or the error of a bad token.

# Links

* Uses: [hashes](../hashes/idea.md).
* Index: [logins](index.md).
