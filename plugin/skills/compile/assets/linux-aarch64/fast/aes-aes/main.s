.arch armv8-a
.arch_extension aes

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
.equ BLOCK_BYTES_PER_HEX, 32
.equ NEWLINE, 10
.equ BAD_MSG_LEN, 15
.equ MAX_WRITES, 64

.section .rodata

.type bad_msg, @object
bad_msg:
    .ascii "aes: bad input\n"
.size bad_msg, . - bad_msg

// Moves the last word of a round key, rotated by a byte, into all four words: the first step of
// the key schedule.
.type rotate_mask, @object
rotate_mask:
    .byte 13, 14, 15, 12, 13, 14, 15, 12, 13, 14, 15, 12, 13, 14, 15, 12
.size rotate_mask, . - rotate_mask

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

.section .bss

// All of standard input, plus one byte to notice input that is too long.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

// The key's 16 bytes.
// shared: main passes it to expand_key; parse_input decodes the key line into it
.type key, @object
key:
    .zero KEY_LEN
.size key, . - key

// The data's bytes, up to 4096 blocks.
// shared: parse_input decodes the data into it; main passes it to encrypt_blocks
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

// The ciphertext in hexadecimal and a newline.
// shared: print_hex fills it with digits and write_line adds the newline and writes it
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
// Reads the key and the data, encrypts every block and prints the ciphertext.
.type main, @function
main:
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, #INPUT_LEN & 0xffff
    movk x2, #INPUT_LEN >> 16, lsl #16
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lmain_failed
    mov x19, x0
    mov x9, #INPUT_LEN & 0xffff
    movk x9, #INPUT_LEN >> 16, lsl #16
    cmp x19, x9
    b.hs .Lmain_bad
    adrp x0, input
    add x0, x0, :lo12:input
    mov x1, x19
    bl parse_input
    tbnz x0, #63, .Lmain_bad
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
    tbnz x0, #63, .Lmain_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_bad:
    mov x0, #STDERR
    adrp x1, bad_msg
    add x1, x1, :lo12:bad_msg
    mov x2, #BAD_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lmain_failed
    mov w0, #EXIT_BAD_INPUT
    b .Lmain_exit
.Lmain_failed:
    mov w0, #EXIT_IO_FAILED
.Lmain_exit:
    bl os_exit
.size main, . - main

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

// Expands a key into the 11 round keys, with the CPU's AES instructions.
// In: x0 = the key, 16 bytes; x1 = where the round keys go, ROUND_KEYS_LEN bytes.
.type expand_key, @function
expand_key:
    cbz x0, .Lexpand_key_assert_key
    cbz x1, .Lexpand_key_assert_keys
    ldr q1, [x0]
    str q1, [x1]
    adrp x4, rotate_mask
    add x4, x4, :lo12:rotate_mask
    ldr q6, [x4]
    adrp x9, rcon_table
    add x9, x9, :lo12:rcon_table
    movi v31.16b, #0
    add x5, x1, #16
    mov x2, #0
    mov x3, #EXPAND_STEPS
.Lexpand_key_loop:  // bound: 10
    cmp x2, #RCON_TABLE_LEN - 16 + 1
    b.hs .Lexpand_key_assert_index
    ldr q5, [x9, x2]
    tbl v2.16b, { v1.16b }, v6.16b
    aese v2.16b, v31.16b
    eor v2.16b, v2.16b, v5.16b
    ext v3.16b, v31.16b, v1.16b, #12
    eor v1.16b, v1.16b, v3.16b
    ext v3.16b, v31.16b, v3.16b, #12
    eor v1.16b, v1.16b, v3.16b
    ext v3.16b, v31.16b, v3.16b, #12
    eor v1.16b, v1.16b, v3.16b
    eor v1.16b, v1.16b, v2.16b
    str q1, [x5, x2]
    add x2, x2, #16
    subs x3, x3, #1
    b.ne .Lexpand_key_loop
    ret
.Lexpand_key_assert_key:
    mov w0, #9
    bl assert_fail
.Lexpand_key_assert_keys:
    mov w0, #10
    bl assert_fail
.Lexpand_key_assert_index:
    mov w0, #11
    bl assert_fail
.size expand_key, . - expand_key

// Encrypts blocks of 16 bytes one at a time: nine rounds of AddRoundKey, SubBytes, ShiftRows and
// MixColumns, a last without MixColumns, and the last round key.
// In: x0 = the round keys; x1 = the blocks; x2 = where the encrypted blocks go; x3 = how many
// blocks, up to MAX_BLOCKS.
.type encrypt_blocks, @function
encrypt_blocks:
    cbz x0, .Lencrypt_blocks_assert_keys
    cbz x1, .Lencrypt_blocks_assert_blocks
    cbz x2, .Lencrypt_blocks_assert_out
    mov x4, #MAX_BLOCKS
    cmp x3, x4
    b.hi .Lencrypt_blocks_assert_count
    lsl x9, x3, #4
    mov x10, #0
    mov x11, #BLOCK_STEPS
.Lencrypt_blocks_block:  // bound: 4097
    cmp x10, x9
    b.hs .Lencrypt_blocks_done
    mov x4, #PLAIN_LEN - 16 + 1
    cmp x10, x4
    b.hs .Lencrypt_blocks_assert_index
    ldr q0, [x1, x10]
    mov x5, #0
    mov x6, #ROUNDS_MIDDLE
.Lencrypt_blocks_round:  // bound: 9
    cmp x5, #ROUND_KEYS_LEN - 16 + 1
    b.hs .Lencrypt_blocks_assert_round
    ldr q2, [x0, x5]
    aese v0.16b, v2.16b
    aesmc v0.16b, v0.16b
    add x5, x5, #16
    subs x6, x6, #1
    b.ne .Lencrypt_blocks_round
    ldr q2, [x0, #144]
    aese v0.16b, v2.16b
    ldr q3, [x0, #160]
    eor v0.16b, v0.16b, v3.16b
    mov x4, #CIPHER_LEN - 16 + 1
    cmp x10, x4
    b.hs .Lencrypt_blocks_assert_out_index
    str q0, [x2, x10]
    add x10, x10, #16
    subs x11, x11, #1
    b.ne .Lencrypt_blocks_block
    b .Lencrypt_blocks_assert_steps
.Lencrypt_blocks_done:
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
.Lencrypt_blocks_assert_round:
    mov w0, #17
    bl assert_fail
.Lencrypt_blocks_assert_out_index:
    mov w0, #18
    bl assert_fail
.Lencrypt_blocks_assert_steps:
    mov w0, #19
    bl assert_fail
.size encrypt_blocks, . - encrypt_blocks

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

// Writes x2 bytes from x1 to file descriptor x0, retrying partial writes.
// In: x0 = the file descriptor, x1 = the bytes, x2 = how many, at least 1.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_all, @function
write_all:
    stp x29, x30, [sp, #-16]!
    cbz x1, .Lwrite_all_assert_null
    cbz x2, .Lwrite_all_assert_empty
    mov x9, x0
    mov x10, #MAX_WRITES
.Lwrite_all_loop:  // bound: 64
    mov x0, x9
    bl os_write
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
    ldp x29, x30, [sp], #16
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

