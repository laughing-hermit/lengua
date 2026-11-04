.arch armv8-a

// AES-128 in plain integer code and a table of 256 words, so it runs on every ARM64 computer. The
// form that uses the CPU's AES instructions, from the very same spec, is
// plugin/skills/compile/assets/linux-aarch64/fast/aes-aes.

// System calls (Linux AArch64)
.equ SYS_READ, 63
.equ SYS_WRITE, 64
.equ SYS_EXIT, 93
// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
.equ STDERR, 2
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 2
.equ EXIT_BAD_INPUT, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
// fill's constants
.equ MAX_FILL_READS, 1099511627776
.equ READ_TO_END, -1
// Sizes and limits: a key line of 32 digits, a newline, 32 to 131072 digits and a newline
.equ KEY_HEX, 32
.equ DATA_AT, 33
.equ MIN_INPUT, 65
.equ HEX_MAX, 131072
.equ DATA_BYTES, 65536
.equ DATA_STEPS, 65537
.equ MAX_BLOCKS, 4096
.equ BLOCK_STEPS, 4097
.equ INPUT_LEN, 131107
.equ KEY_LEN, 16
.equ PLAIN_LEN, 65536
.equ CIPHER_LEN, 65536
.equ ROUND_KEYS_LEN, 176
.equ RCON_TABLE_LEN, 160
.equ OUT_LEN, 131073
.equ EXPAND_STEPS, 10
.equ ROUNDS_MIDDLE, 9
.equ CURRENT_LEN, 32
.equ TABLE_ENTRIES, 256
.equ BLOCK_BYTES_PER_HEX, 32
.equ NEWLINE, 10
.equ BAD_MSG_LEN, 15
.equ MAX_WRITES, 64

.section .rodata

.type bad_msg, @object
bad_msg:
    .ascii "aes: bad input\n"
.size bad_msg, . - bad_msg

// SubBytes and MixColumns in one table, for the byte in row 0 of a column. S is the S-box; each
// word holds, from its lowest byte, 2 * S[x], S[x], S[x] and 3 * S[x] (products in GF(2^8)), so
// the bytes in rows 1, 2 and 3 use the same word rotated left by 8, 16 and 24 bits, and S[x]
// itself is the word's byte 1.
.type te0, @object
te0:
    .long 0xa56363c6, 0x847c7cf8, 0x997777ee, 0x8d7b7bf6
    .long 0x0df2f2ff, 0xbd6b6bd6, 0xb16f6fde, 0x54c5c591
    .long 0x50303060, 0x03010102, 0xa96767ce, 0x7d2b2b56
    .long 0x19fefee7, 0x62d7d7b5, 0xe6abab4d, 0x9a7676ec
    .long 0x45caca8f, 0x9d82821f, 0x40c9c989, 0x877d7dfa
    .long 0x15fafaef, 0xeb5959b2, 0xc947478e, 0x0bf0f0fb
    .long 0xecadad41, 0x67d4d4b3, 0xfda2a25f, 0xeaafaf45
    .long 0xbf9c9c23, 0xf7a4a453, 0x967272e4, 0x5bc0c09b
    .long 0xc2b7b775, 0x1cfdfde1, 0xae93933d, 0x6a26264c
    .long 0x5a36366c, 0x413f3f7e, 0x02f7f7f5, 0x4fcccc83
    .long 0x5c343468, 0xf4a5a551, 0x34e5e5d1, 0x08f1f1f9
    .long 0x937171e2, 0x73d8d8ab, 0x53313162, 0x3f15152a
    .long 0x0c040408, 0x52c7c795, 0x65232346, 0x5ec3c39d
    .long 0x28181830, 0xa1969637, 0x0f05050a, 0xb59a9a2f
    .long 0x0907070e, 0x36121224, 0x9b80801b, 0x3de2e2df
    .long 0x26ebebcd, 0x6927274e, 0xcdb2b27f, 0x9f7575ea
    .long 0x1b090912, 0x9e83831d, 0x742c2c58, 0x2e1a1a34
    .long 0x2d1b1b36, 0xb26e6edc, 0xee5a5ab4, 0xfba0a05b
    .long 0xf65252a4, 0x4d3b3b76, 0x61d6d6b7, 0xceb3b37d
    .long 0x7b292952, 0x3ee3e3dd, 0x712f2f5e, 0x97848413
    .long 0xf55353a6, 0x68d1d1b9, 0x00000000, 0x2cededc1
    .long 0x60202040, 0x1ffcfce3, 0xc8b1b179, 0xed5b5bb6
    .long 0xbe6a6ad4, 0x46cbcb8d, 0xd9bebe67, 0x4b393972
    .long 0xde4a4a94, 0xd44c4c98, 0xe85858b0, 0x4acfcf85
    .long 0x6bd0d0bb, 0x2aefefc5, 0xe5aaaa4f, 0x16fbfbed
    .long 0xc5434386, 0xd74d4d9a, 0x55333366, 0x94858511
    .long 0xcf45458a, 0x10f9f9e9, 0x06020204, 0x817f7ffe
    .long 0xf05050a0, 0x443c3c78, 0xba9f9f25, 0xe3a8a84b
    .long 0xf35151a2, 0xfea3a35d, 0xc0404080, 0x8a8f8f05
    .long 0xad92923f, 0xbc9d9d21, 0x48383870, 0x04f5f5f1
    .long 0xdfbcbc63, 0xc1b6b677, 0x75dadaaf, 0x63212142
    .long 0x30101020, 0x1affffe5, 0x0ef3f3fd, 0x6dd2d2bf
    .long 0x4ccdcd81, 0x140c0c18, 0x35131326, 0x2fececc3
    .long 0xe15f5fbe, 0xa2979735, 0xcc444488, 0x3917172e
    .long 0x57c4c493, 0xf2a7a755, 0x827e7efc, 0x473d3d7a
    .long 0xac6464c8, 0xe75d5dba, 0x2b191932, 0x957373e6
    .long 0xa06060c0, 0x98818119, 0xd14f4f9e, 0x7fdcdca3
    .long 0x66222244, 0x7e2a2a54, 0xab90903b, 0x8388880b
    .long 0xca46468c, 0x29eeeec7, 0xd3b8b86b, 0x3c141428
    .long 0x79dedea7, 0xe25e5ebc, 0x1d0b0b16, 0x76dbdbad
    .long 0x3be0e0db, 0x56323264, 0x4e3a3a74, 0x1e0a0a14
    .long 0xdb494992, 0x0a06060c, 0x6c242448, 0xe45c5cb8
    .long 0x5dc2c29f, 0x6ed3d3bd, 0xefacac43, 0xa66262c4
    .long 0xa8919139, 0xa4959531, 0x37e4e4d3, 0x8b7979f2
    .long 0x32e7e7d5, 0x43c8c88b, 0x5937376e, 0xb76d6dda
    .long 0x8c8d8d01, 0x64d5d5b1, 0xd24e4e9c, 0xe0a9a949
    .long 0xb46c6cd8, 0xfa5656ac, 0x07f4f4f3, 0x25eaeacf
    .long 0xaf6565ca, 0x8e7a7af4, 0xe9aeae47, 0x18080810
    .long 0xd5baba6f, 0x887878f0, 0x6f25254a, 0x722e2e5c
    .long 0x241c1c38, 0xf1a6a657, 0xc7b4b473, 0x51c6c697
    .long 0x23e8e8cb, 0x7cdddda1, 0x9c7474e8, 0x211f1f3e
    .long 0xdd4b4b96, 0xdcbdbd61, 0x868b8b0d, 0x858a8a0f
    .long 0x907070e0, 0x423e3e7c, 0xc4b5b571, 0xaa6666cc
    .long 0xd8484890, 0x05030306, 0x01f6f6f7, 0x120e0e1c
    .long 0xa36161c2, 0x5f35356a, 0xf95757ae, 0xd0b9b969
    .long 0x91868617, 0x58c1c199, 0x271d1d3a, 0xb99e9e27
    .long 0x38e1e1d9, 0x13f8f8eb, 0xb398982b, 0x33111122
    .long 0xbb6969d2, 0x70d9d9a9, 0x898e8e07, 0xa7949433
    .long 0xb69b9b2d, 0x221e1e3c, 0x92878715, 0x20e9e9c9
    .long 0x49cece87, 0xff5555aa, 0x78282850, 0x7adfdfa5
    .long 0x8f8c8c03, 0xf8a1a159, 0x80898909, 0x170d0d1a
    .long 0xdabfbf65, 0x31e6e6d7, 0xc6424284, 0xb86868d0
    .long 0xc3414182, 0xb0999929, 0x772d2d5a, 0x110f0f1e
    .long 0xcbb0b07b, 0xfc5454a8, 0xd6bbbb6d, 0x3a16162c
.size te0, . - te0

// The round constants, one in each word.
.type rcon_table, @object
rcon_table:
    .long 0x01, 0x01, 0x01, 0x01
    .long 0x02, 0x02, 0x02, 0x02
    .long 0x04, 0x04, 0x04, 0x04
    .long 0x08, 0x08, 0x08, 0x08
    .long 0x10, 0x10, 0x10, 0x10
    .long 0x20, 0x20, 0x20, 0x20
    .long 0x40, 0x40, 0x40, 0x40
    .long 0x80, 0x80, 0x80, 0x80
    .long 0x1b, 0x1b, 0x1b, 0x1b
    .long 0x36, 0x36, 0x36, 0x36
.size rcon_table, . - rcon_table

.type hex_digits, @object
hex_digits:
    .ascii "0123456789abcdef"
.size hex_digits, . - hex_digits

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// All of standard input, plus one byte to notice input that is too long.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

// The key's 16 bytes.
// shared: _start passes it to expand_key; parse_input decodes the key line into it
.type key, @object
key:
    .zero KEY_LEN
.size key, . - key

// The data's bytes, up to 4096 blocks.
// shared: parse_input decodes the data into it; _start passes it to encrypt_blocks
.type plain, @object
plain:
    .zero PLAIN_LEN
.size plain, . - plain

// The same blocks encrypted.
.type cipher, @object
cipher:
    .zero CIPHER_LEN
.size cipher, . - cipher

// The 11 round keys of 16 bytes.
.type round_keys, @object
round_keys:
    .zero ROUND_KEYS_LEN
.size round_keys, . - round_keys

// The block being encrypted: its four words twice over, so that the four words from any word on
// (the ones ShiftRows brings into a column) are next to each other.
// shared: encrypt_block sets it, aes_round replaces it, and column and last_column read it
.type current, @object
current:
    .zero CURRENT_LEN
.size current, . - current

// The ciphertext in hexadecimal and a newline.
// shared: print_hex fills it with digits and write_line adds the newline and writes it
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
.globl _start

// Reads the key and the data, encrypts every block and prints the ciphertext.
.type _start, @function
_start:
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, #INPUT_LEN & 0xffff
    movk x2, #INPUT_LEN >> 16, lsl #16
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lstart_failed
    mov x19, x0
    mov x9, #INPUT_LEN & 0xffff
    movk x9, #INPUT_LEN >> 16, lsl #16
    cmp x19, x9
    b.hs .Lstart_bad
    adrp x0, input
    add x0, x0, :lo12:input
    mov x1, x19
    bl parse_input
    tbnz x0, #63, .Lstart_bad
    mov x20, x0
    adrp x0, key
    add x0, x0, :lo12:key
    adrp x1, round_keys
    add x1, x1, :lo12:round_keys
    bl expand_key
    adrp x0, round_keys
    add x0, x0, :lo12:round_keys
    adrp x1, plain
    add x1, x1, :lo12:plain
    adrp x2, cipher
    add x2, x2, :lo12:cipher
    mov x3, x20
    bl encrypt_blocks
    adrp x0, cipher
    add x0, x0, :lo12:cipher
    lsl x1, x20, #4
    bl print_hex
    tbnz x0, #63, .Lstart_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_bad:
    mov x0, #STDERR
    adrp x1, bad_msg
    add x1, x1, :lo12:bad_msg
    mov x2, #BAD_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lstart_failed
    mov w0, #EXIT_BAD_INPUT
    b .Lstart_exit
.Lstart_failed:
    mov w0, #EXIT_IO_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.size _start, . - _start

// Checks the input's layout and decodes the key and the data.
// In: x0 = the input; x1 = its length, less than INPUT_LEN.
// Out: x0 = how many blocks the data holds, or ERROR_INVALID.
// returns: result
.type parse_input, @function
parse_input:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Lparse_input_assert_null
    mov x9, #INPUT_LEN & 0xffff
    movk x9, #INPUT_LEN >> 16, lsl #16
    cmp x1, x9
    b.hs .Lparse_input_assert_length
    mov x19, x0
    mov x20, x1
    mov x0, #ERROR_INVALID
    cmp x20, #MIN_INPUT
    b.lo .Lparse_input_return
    ldrb w2, [x19, #KEY_HEX]
    cmp w2, #NEWLINE
    b.ne .Lparse_input_return
    mov x21, x20
    sub x3, x20, #1
    cmp x3, x9
    b.hs .Lparse_input_assert_end
    ldrb w2, [x19, x3]
    cmp w2, #NEWLINE
    b.ne .Lparse_input_digits
    sub x21, x21, #1
.Lparse_input_digits:
    sub x21, x21, #DATA_AT
    mov x4, #BLOCK_BYTES_PER_HEX
    udiv x5, x21, x4
    msub x5, x5, x4, x21
    cbnz x5, .Lparse_input_return
    mov x4, #HEX_MAX
    cmp x21, x4
    b.hi .Lparse_input_return
    adrp x0, key
    add x0, x0, :lo12:key
    mov x1, x19
    mov x2, #KEY_HEX
    bl decode_hex
    tbnz x0, #63, .Lparse_input_return
    adrp x0, plain
    add x0, x0, :lo12:plain
    add x1, x19, #DATA_AT
    mov x2, x21
    bl decode_hex
    tbnz x0, #63, .Lparse_input_return
    lsr x0, x21, #5
.Lparse_input_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lparse_input_assert_null:
    mov w0, #1
    bl assert_fail
.Lparse_input_assert_length:
    mov w0, #2
    bl assert_fail
.Lparse_input_assert_end:
    mov w0, #28
    bl assert_fail
.size parse_input, . - parse_input

// Turns hexadecimal digits into bytes.
// In: x0 = where the bytes go; x1 = the digits; x2 = how many, an even number up to HEX_MAX.
// Out: x0 = 0, or ERROR_INVALID when a character isn't a hexadecimal digit.
// returns: result
.type decode_hex, @function
decode_hex:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x0, .Ldecode_hex_assert_dest
    cbz x1, .Ldecode_hex_assert_digits
    mov x9, #HEX_MAX
    cmp x2, x9
    b.hi .Ldecode_hex_assert_count
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, #0
    mov x23, #DATA_STEPS & 0xffff
    movk x23, #DATA_STEPS >> 16, lsl #16
.Ldecode_hex_loop:  // bound: 65537
    lsl x3, x22, #1
    cmp x3, x21
    b.hs .Ldecode_hex_done
    mov x4, #HEX_MAX
    cmp x3, x4
    b.hs .Ldecode_hex_assert_index
    ldrb w0, [x20, x3]
    add x5, x3, #1
    cmp x5, x4
    b.hs .Ldecode_hex_assert_odd
    ldrb w1, [x20, x5]
    bl hex_pair
    tbnz x0, #63, .Ldecode_hex_return
    mov x4, #DATA_BYTES
    cmp x22, x4
    b.hs .Ldecode_hex_assert_dest_index
    strb w0, [x19, x22]
    add x22, x22, #1
    subs x23, x23, #1
    b.ne .Ldecode_hex_loop
    b .Ldecode_hex_assert_steps
.Ldecode_hex_done:
    mov x0, #0
.Ldecode_hex_return:
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Ldecode_hex_assert_dest:
    mov w0, #3
    bl assert_fail
.Ldecode_hex_assert_digits:
    mov w0, #4
    bl assert_fail
.Ldecode_hex_assert_count:
    mov w0, #5
    bl assert_fail
.Ldecode_hex_assert_index:
    mov w0, #6
    bl assert_fail
.Ldecode_hex_assert_odd:
    mov w0, #29
    bl assert_fail
.Ldecode_hex_assert_dest_index:
    mov w0, #7
    bl assert_fail
.Ldecode_hex_assert_steps:
    mov w0, #8
    bl assert_fail
.size decode_hex, . - decode_hex

// Turns two hexadecimal digits into a byte.
// In: w0 = the first digit's character; w1 = the second's.
// Out: x0 = the byte, 0 to 255, or ERROR_INVALID.
// returns: result
.type hex_pair, @function
hex_pair:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    mov w19, w1
    bl hex_value
    tbnz x0, #63, .Lhex_pair_return
    lsl w20, w0, #4
    mov w0, w19
    bl hex_value
    tbnz x0, #63, .Lhex_pair_return
    orr w0, w0, w20
.Lhex_pair_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.size hex_pair, . - hex_pair

// The value of a hexadecimal digit: 0 to 9, a to f or A to F.
// In: w0 = the character.
// Out: x0 = 0 to 15, or ERROR_INVALID.
// returns: result
.type hex_value, @function
hex_value:
    sub w1, w0, #'0'
    cmp w1, #9
    b.ls .Lhex_value_digit
    sub w1, w0, #'a'
    cmp w1, #5
    b.ls .Lhex_value_letter
    sub w1, w0, #'A'
    cmp w1, #5
    b.hi .Lhex_value_invalid
.Lhex_value_letter:
    add w0, w1, #10
    b .Lhex_value_return
.Lhex_value_digit:
    mov w0, w1
    b .Lhex_value_return
.Lhex_value_invalid:
    mov x0, #ERROR_INVALID
.Lhex_value_return:
    ret
.size hex_value, . - hex_value

// Expands a key into the 11 round keys, a word at a time: each round key's first word is the
// previous round key's first word xor SubWord(RotWord(its last word)) xor the round constant, and
// each other word the word before it xor the one four words back.
// In: x0 = the key, 16 bytes; x1 = where the round keys go, ROUND_KEYS_LEN bytes.
.type expand_key, @function
expand_key:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Lexpand_key_assert_key
    cbz x1, .Lexpand_key_assert_keys
    ldr q0, [x0]
    str q0, [x1]
    mov x19, x1
    adrp x22, rcon_table
    add x22, x22, :lo12:rcon_table
    mov x21, #EXPAND_STEPS
.Lexpand_key_loop:  // bound: 10
    ldr w0, [x19, #12]
    ror w0, w0, #8
    bl sub_word
    ldr w1, [x22]
    eor w0, w0, w1
    ldp w1, w2, [x19]
    eor w0, w0, w1
    str w0, [x19, #16]
    eor w0, w0, w2
    str w0, [x19, #20]
    ldp w1, w2, [x19, #8]
    eor w0, w0, w1
    str w0, [x19, #24]
    eor w0, w0, w2
    str w0, [x19, #28]
    add x19, x19, #16
    add x22, x22, #16
    subs x21, x21, #1
    b.ne .Lexpand_key_loop
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lexpand_key_assert_key:
    mov w0, #9
    bl assert_fail
.Lexpand_key_assert_keys:
    mov w0, #10
    bl assert_fail
.size expand_key, . - expand_key

// SubWord: the S-box applied to each byte of a word, which is byte 1 of the table's words.
// In: w0 = the word.
// Out: w0 = the word with each byte replaced.
.type sub_word, @function
sub_word:
    adrp x9, te0
    add x9, x9, :lo12:te0
    and w3, w0, #255
    cmp x3, #TABLE_ENTRIES
    b.hs .Lsub_word_assert_row0
    ldr w4, [x9, x3, lsl #2]
    ubfx w4, w4, #8, #8
    ubfx w3, w0, #8, #8
    cmp x3, #TABLE_ENTRIES
    b.hs .Lsub_word_assert_row1
    ldr w5, [x9, x3, lsl #2]
    ubfx w5, w5, #8, #8
    orr w4, w4, w5, lsl #8
    ubfx w3, w0, #16, #8
    cmp x3, #TABLE_ENTRIES
    b.hs .Lsub_word_assert_row2
    ldr w5, [x9, x3, lsl #2]
    ubfx w5, w5, #8, #8
    orr w4, w4, w5, lsl #16
    lsr w3, w0, #24
    cmp x3, #TABLE_ENTRIES
    b.hs .Lsub_word_assert_row3
    ldr w5, [x9, x3, lsl #2]
    ubfx w5, w5, #8, #8
    orr w4, w4, w5, lsl #24
    mov w0, w4
    ret
.Lsub_word_assert_row0:
    mov w0, #35
    bl assert_fail
.Lsub_word_assert_row1:
    mov w0, #36
    bl assert_fail
.Lsub_word_assert_row2:
    mov w0, #37
    bl assert_fail
.Lsub_word_assert_row3:
    mov w0, #38
    bl assert_fail
.size sub_word, . - sub_word

// Encrypts blocks of 16 bytes one at a time.
// In: x0 = the round keys; x1 = the blocks; x2 = where the encrypted blocks go; x3 = how many
// blocks, up to MAX_BLOCKS.
.type encrypt_blocks, @function
encrypt_blocks:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x0, .Lencrypt_blocks_assert_keys
    cbz x1, .Lencrypt_blocks_assert_blocks
    cbz x2, .Lencrypt_blocks_assert_out
    mov x4, #MAX_BLOCKS
    cmp x3, x4
    b.hi .Lencrypt_blocks_assert_count
    mov x19, x0
    mov x20, x1
    mov x21, x2
    lsl x22, x3, #4
    mov x23, #0
    mov x24, #BLOCK_STEPS
.Lencrypt_blocks_block:  // bound: 4097
    cmp x23, x22
    b.hs .Lencrypt_blocks_done
    mov x4, #PLAIN_LEN - 16 + 1
    cmp x23, x4
    b.hs .Lencrypt_blocks_assert_index
    mov x0, x19
    add x1, x20, x23
    add x2, x21, x23
    bl encrypt_block
    add x23, x23, #16
    subs x24, x24, #1
    b.ne .Lencrypt_blocks_block
    b .Lencrypt_blocks_assert_steps
.Lencrypt_blocks_done:
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lencrypt_blocks_assert_keys:
    mov w0, #12
    bl assert_fail
.Lencrypt_blocks_assert_blocks:
    mov w0, #13
    bl assert_fail
.Lencrypt_blocks_assert_out:
    mov w0, #14
    bl assert_fail
.Lencrypt_blocks_assert_count:
    mov w0, #15
    bl assert_fail
.Lencrypt_blocks_assert_index:
    mov w0, #16
    bl assert_fail
.Lencrypt_blocks_assert_steps:
    mov w0, #19
    bl assert_fail
.size encrypt_blocks, . - encrypt_blocks

// Encrypts one block: the first round key added, nine rounds and the last, which has no
// MixColumns.
// In: x0 = the round keys; x1 = the block, 16 bytes; x2 = where the encrypted block goes.
.type encrypt_block, @function
encrypt_block:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Lencrypt_block_assert_keys
    cbz x1, .Lencrypt_block_assert_block
    cbz x2, .Lencrypt_block_assert_out
    mov x19, x0
    mov x20, x2
    ldr q0, [x1]
    ldr q1, [x19]
    eor v0.16b, v0.16b, v1.16b
    adrp x9, current
    add x9, x9, :lo12:current
    str q0, [x9]
    str q0, [x9, #16]
    mov x21, #ROUNDS_MIDDLE
.Lencrypt_block_round:  // bound: 9
    add x19, x19, #16
    mov x0, x19
    bl aes_round
    subs x21, x21, #1
    b.ne .Lencrypt_block_round
    add x19, x19, #16
    mov x0, x19
    mov x1, x20
    bl last_round
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lencrypt_block_assert_keys:
    mov w0, #17
    bl assert_fail
.Lencrypt_block_assert_block:
    mov w0, #18
    bl assert_fail
.Lencrypt_block_assert_out:
    mov w0, #39
    bl assert_fail
.size encrypt_block, . - encrypt_block

// One round that isn't the last: the four columns of the new block, each from the old, then both
// copies of the block replaced.
// In: x0 = the round key, 16 bytes.
.type aes_round, @function
aes_round:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x0, .Laes_round_assert_key
    mov x19, x0
    mov x0, #0
    ldr w1, [x19]
    bl column
    mov w20, w0
    mov x0, #4
    ldr w1, [x19, #4]
    bl column
    mov w21, w0
    mov x0, #8
    ldr w1, [x19, #8]
    bl column
    mov w22, w0
    mov x0, #12
    ldr w1, [x19, #12]
    bl column
    mov w23, w0
    adrp x9, current
    add x9, x9, :lo12:current
    stp w20, w21, [x9]
    stp w22, w23, [x9, #8]
    stp w20, w21, [x9, #16]
    stp w22, w23, [x9, #24]
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Laes_round_assert_key:
    mov w0, #40
    bl assert_fail
.size aes_round, . - aes_round

// One column of a round that isn't the last, with SubBytes, ShiftRows, MixColumns and the round
// key's word in one: the byte of row r comes from the word r columns on, goes through the table
// and is rotated left by 8 * r bits.
// In: x0 = 4 times the column, 0 to 12; w1 = the round key's word for that column.
// Out: w0 = the column's new word.
.type column, @function
column:
    adrp x8, current
    add x8, x8, :lo12:current
    adrp x9, te0
    add x9, x9, :lo12:te0
    add x10, x8, #4
    add x11, x8, #8
    add x12, x8, #12
    cmp x0, #CURRENT_LEN - 16 + 1
    b.hs .Lcolumn_assert_index
    ldr w2, [x8, x0]
    and w3, w2, #255
    cmp x3, #TABLE_ENTRIES
    b.hs .Lcolumn_assert_row0
    ldr w4, [x9, x3, lsl #2]
    ldr w2, [x10, x0]
    ubfx w3, w2, #8, #8
    cmp x3, #TABLE_ENTRIES
    b.hs .Lcolumn_assert_row1
    ldr w5, [x9, x3, lsl #2]
    ror w5, w5, #24
    eor w4, w4, w5
    ldr w2, [x11, x0]
    ubfx w3, w2, #16, #8
    cmp x3, #TABLE_ENTRIES
    b.hs .Lcolumn_assert_row2
    ldr w5, [x9, x3, lsl #2]
    ror w5, w5, #16
    eor w4, w4, w5
    ldr w2, [x12, x0]
    lsr w3, w2, #24
    cmp x3, #TABLE_ENTRIES
    b.hs .Lcolumn_assert_row3
    ldr w5, [x9, x3, lsl #2]
    ror w5, w5, #8
    eor w4, w4, w5
    eor w0, w4, w1
    ret
.Lcolumn_assert_index:
    mov w0, #41
    bl assert_fail
.Lcolumn_assert_row0:
    mov w0, #42
    bl assert_fail
.Lcolumn_assert_row1:
    mov w0, #43
    bl assert_fail
.Lcolumn_assert_row2:
    mov w0, #44
    bl assert_fail
.Lcolumn_assert_row3:
    mov w0, #45
    bl assert_fail
.size column, . - column

// The last round: SubBytes, ShiftRows and the last round key, the four columns written to the
// encrypted block.
// In: x0 = the last round key, 16 bytes; x1 = where the encrypted block goes.
.type last_round, @function
last_round:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    cbz x0, .Llast_round_assert_key
    cbz x1, .Llast_round_assert_out
    mov x19, x0
    mov x20, x1
    mov x0, #0
    ldr w1, [x19]
    bl last_column
    str w0, [x20]
    mov x0, #4
    ldr w1, [x19, #4]
    bl last_column
    str w0, [x20, #4]
    mov x0, #8
    ldr w1, [x19, #8]
    bl last_column
    str w0, [x20, #8]
    mov x0, #12
    ldr w1, [x19, #12]
    bl last_column
    str w0, [x20, #12]
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Llast_round_assert_key:
    mov w0, #46
    bl assert_fail
.Llast_round_assert_out:
    mov w0, #47
    bl assert_fail
.size last_round, . - last_round

// One column of the last round: the bytes ShiftRows brings into it go through the S-box, which is
// byte 1 of the table's words, and the round key's word is added.
// In: x0 = 4 times the column, 0 to 12; w1 = the round key's word for that column.
// Out: w0 = the column's new word.
.type last_column, @function
last_column:
    adrp x8, current
    add x8, x8, :lo12:current
    adrp x9, te0
    add x9, x9, :lo12:te0
    add x10, x8, #4
    add x11, x8, #8
    add x12, x8, #12
    cmp x0, #CURRENT_LEN - 16 + 1
    b.hs .Llast_column_assert_index
    ldr w2, [x8, x0]
    and w3, w2, #255
    cmp x3, #TABLE_ENTRIES
    b.hs .Llast_column_assert_row0
    ldr w4, [x9, x3, lsl #2]
    ubfx w4, w4, #8, #8
    ldr w2, [x10, x0]
    ubfx w3, w2, #8, #8
    cmp x3, #TABLE_ENTRIES
    b.hs .Llast_column_assert_row1
    ldr w5, [x9, x3, lsl #2]
    ubfx w5, w5, #8, #8
    orr w4, w4, w5, lsl #8
    ldr w2, [x11, x0]
    ubfx w3, w2, #16, #8
    cmp x3, #TABLE_ENTRIES
    b.hs .Llast_column_assert_row2
    ldr w5, [x9, x3, lsl #2]
    ubfx w5, w5, #8, #8
    orr w4, w4, w5, lsl #16
    ldr w2, [x12, x0]
    lsr w3, w2, #24
    cmp x3, #TABLE_ENTRIES
    b.hs .Llast_column_assert_row3
    ldr w5, [x9, x3, lsl #2]
    ubfx w5, w5, #8, #8
    orr w4, w4, w5, lsl #24
    eor w0, w4, w1
    ret
.Llast_column_assert_index:
    mov w0, #48
    bl assert_fail
.Llast_column_assert_row0:
    mov w0, #49
    bl assert_fail
.Llast_column_assert_row1:
    mov w0, #50
    bl assert_fail
.Llast_column_assert_row2:
    mov w0, #51
    bl assert_fail
.Llast_column_assert_row3:
    mov w0, #52
    bl assert_fail
.size last_column, . - last_column

// Prints bytes as lowercase hexadecimal digits and a newline.
// In: x0 = the bytes; x1 = how many, up to DATA_BYTES.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_hex, @function
print_hex:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lprint_hex_assert_bytes
    mov x4, #DATA_BYTES
    cmp x1, x4
    b.hi .Lprint_hex_assert_count
    adrp x5, hex_digits
    add x5, x5, :lo12:hex_digits
    adrp x6, out
    add x6, x6, :lo12:out
    mov x2, #0
    mov x11, #DATA_STEPS & 0xffff
    movk x11, #DATA_STEPS >> 16, lsl #16
.Lprint_hex_loop:  // bound: 65537
    cmp x2, x1
    b.hs .Lprint_hex_done
    cmp x2, x4
    b.hs .Lprint_hex_assert_index
    ldrb w3, [x0, x2]
    lsr w7, w3, #4
    lsl w3, w3, #28
    lsr w3, w3, #28
    cmp x7, #16
    b.hs .Lprint_hex_assert_high
    ldrb w7, [x5, x7]
    lsl x10, x2, #1
    mov x9, #HEX_MAX
    cmp x10, x9
    b.hs .Lprint_hex_assert_out
    strb w7, [x6, x10]
    cmp x3, #16
    b.hs .Lprint_hex_assert_low
    ldrb w7, [x5, x3]
    add x12, x10, #1
    cmp x12, x9
    b.hs .Lprint_hex_assert_odd
    strb w7, [x6, x12]
    add x2, x2, #1
    subs x11, x11, #1
    b.ne .Lprint_hex_loop
    b .Lprint_hex_assert_steps
.Lprint_hex_done:
    lsl x0, x1, #1
    bl write_line
    tbnz x0, #63, .Lprint_hex_return
.Lprint_hex_return:
    ldp x29, x30, [sp], #16
    ret
.Lprint_hex_assert_bytes:
    mov w0, #20
    bl assert_fail
.Lprint_hex_assert_count:
    mov w0, #21
    bl assert_fail
.Lprint_hex_assert_index:
    mov w0, #22
    bl assert_fail
.Lprint_hex_assert_high:
    mov w0, #23
    bl assert_fail
.Lprint_hex_assert_out:
    mov w0, #24
    bl assert_fail
.Lprint_hex_assert_low:
    mov w0, #25
    bl assert_fail
.Lprint_hex_assert_odd:
    mov w0, #30
    bl assert_fail
.Lprint_hex_assert_steps:
    mov w0, #26
    bl assert_fail
.size print_hex, . - print_hex

// Writes the hex digits in out, a newline after them, to standard output.
// In: x0 = how many digits, 1 to HEX_MAX.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_line, @function
write_line:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lwrite_line_assert_empty
    mov x9, #HEX_MAX
    cmp x0, x9
    b.hi .Lwrite_line_assert_end
    adrp x6, out
    add x6, x6, :lo12:out
    mov w7, #NEWLINE
    strb w7, [x6, x0]
    add x2, x0, #1
    mov x1, x6
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lwrite_line_return
    mov x0, #0
.Lwrite_line_return:
    ldp x29, x30, [sp], #16
    ret
.Lwrite_line_assert_empty:
    mov w0, #34
    bl assert_fail
.Lwrite_line_assert_end:
    mov w0, #27
    bl assert_fail
.size write_line, . - write_line

// Reads from file descriptor x0 into the x2 bytes at x1 until they're full, the input ends,
// or, when w3 is a byte (0 to 255), the last byte read is w3; with w3 = -1 it reads to the
// end. However the input arrives, it reads the same.
// In: x0 = the file descriptor, x1 = the buffer, x2 = its size, at least 1, w3 = the end
// byte or -1.
// Out: x0 = how many bytes it read, or a negative error code.
// returns: result
.type fill, @function
fill:
    cbz x1, .Lfill_assert_null
    cbz x2, .Lfill_assert_empty
    mov x9, x0
    mov x10, x2
    mov w11, w3
    mov x12, #MAX_FILL_READS
.Lfill_loop:  // bound: 1099511627776
    mov x0, x9
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lfill_return
    cbz x0, .Lfill_done
    cmp x0, x2
    b.hi .Lfill_assert_count
    add x1, x1, x0
    subs x2, x2, x0
    b.eq .Lfill_done
    ldurb w13, [x1, #-1]
    cmp w13, w11
    b.eq .Lfill_done
    subs x12, x12, #1
    b.ne .Lfill_loop
    mov x0, #ERROR_IO
    b .Lfill_return
.Lfill_done:
    sub x0, x10, x2
.Lfill_return:
    ret
.Lfill_assert_null:
    mov w0, #990
    bl assert_fail
.Lfill_assert_empty:
    mov w0, #991
    bl assert_fail
.Lfill_assert_count:
    mov w0, #992
    bl assert_fail
.size fill, . - fill

// Writes x2 bytes from x1 to file descriptor x0, retrying partial writes.
// In: x0 = the file descriptor, x1 = the bytes, x2 = how many, at least 1.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_all, @function
write_all:
    cbz x1, .Lwrite_all_assert_null
    cbz x2, .Lwrite_all_assert_empty
    mov x9, x0
    mov x10, #MAX_WRITES
.Lwrite_all_loop:  // bound: 64
    mov x0, x9
    mov x8, #SYS_WRITE
    svc #0
    tbnz x0, #63, .Lwrite_all_return
    cmp x0, x2
    b.hi .Lwrite_all_assert_count
    add x1, x1, x0
    subs x2, x2, x0
    b.eq .Lwrite_all_done
    subs x10, x10, #1
    b.ne .Lwrite_all_loop
    mov x0, #ERROR_IO
    b .Lwrite_all_return
.Lwrite_all_done:
    mov x0, #0
.Lwrite_all_return:
    ret
.Lwrite_all_assert_null:
    mov w0, #31
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #32
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #33
    bl assert_fail
.size write_all, . - write_all

// lengua's assertion handler. check compares it word for word: copy it exactly.
// In: w0 = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
// standard error and ends the program, every thread of it, with code 134.
.type assert_fail, @function
assert_fail:
    sub sp, sp, #32
    adrp x1, assert_message
    add x1, x1, :lo12:assert_message
    ldp x2, x3, [x1]
    stp x2, x3, [sp]
    ldr x2, [x1, #16]
    str x2, [sp, #16]
    ldr w2, [x1, #24]
    str w2, [sp, #24]
    ldrb w2, [x1, #28]
    strb w2, [sp, #28]
    mov w2, #100
    udiv w3, w0, w2
    msub w0, w3, w2, w0
    add w3, w3, #48
    strb w3, [sp, #18]
    mov w2, #10
    udiv w3, w0, w2
    msub w0, w3, w2, w0
    add w3, w3, #48
    strb w3, [sp, #19]
    add w0, w0, #48
    strb w0, [sp, #20]
    mov x0, #2
    mov x1, sp
    mov x2, #29
    mov x8, #64
    svc #0
    mov x0, #134
    mov x8, #94
    svc #0
.size assert_fail, . - assert_fail
