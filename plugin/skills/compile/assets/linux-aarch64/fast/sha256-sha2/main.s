.arch armv8-a
.arch_extension sha2

// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 2
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// fill's constants
.equ READ_TO_END, -1
// Sizes and limits: standard input is hashed in pieces of INPUT_LEN bytes, 64 blocks of 64
.equ INPUT_LEN, 4096
.equ BLOCK_BYTES, 64
.equ BLOCK_STEPS, 65
.equ MAX_CHUNKS, 268435456
.equ STATE_WORDS, 8
.equ STATE_LEN, 32
.equ SCHEDULE_LEN, 256
.equ EXTEND_STEPS, 12
.equ ROUND_STEPS, 16
.equ PADDED_LEN, 128
.equ LENGTH_AT_ONE, 56
.equ LENGTH_AT_TWO, 120
.equ MAX_COPY, 63
.equ PAD_BYTE, 0x80
.equ HEX_DIGITS_COUNT, 64
.equ NIBBLES, 8
.equ OUT_LEN, 65
.equ NEWLINE, 10
.equ MAX_WRITES, 64

.section .rodata

// The 64 round constants: the first 32 bits of the fractional parts of the cube roots of the
// first 64 primes.
.type k_table, @object
k_table:
    .long 0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5
    .long 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5
    .long 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3
    .long 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174
    .long 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc
    .long 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da
    .long 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7
    .long 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967
    .long 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13
    .long 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85
    .long 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3
    .long 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070
    .long 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5
    .long 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3
    .long 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208
    .long 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
.size k_table, . - k_table

// The first hash value: the first 32 bits of the fractional parts of the square roots of the
// first 8 primes.
.type initial_state, @object
initial_state:
    .long 0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a
    .long 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
.size initial_state, . - initial_state

.type hex_digits, @object
hex_digits:
    .ascii "0123456789abcdef"
.size hex_digits, . - hex_digits

.section .bss

// Standard input as it comes, one piece at a time.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

// The eight words of the hash value, a to h.
.type state, @object
state:
    .zero STATE_LEN
.size state, . - state

// The 64 words of one block's message schedule.
// shared: schedule_words writes it and rounds reads it, one block at a time
.type schedule, @object
schedule:
    .zero SCHEDULE_LEN
.size schedule, . - schedule

// The last one or two blocks: the message's tail, a 1 bit, zeros and its length in bits.
.type padded, @object
padded:
    .zero PADDED_LEN
.size padded, . - padded

// The digest in hexadecimal and a newline.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
// Hashes standard input a piece at a time, whole blocks as they come, then the tail with the
// padding, and prints the digest.
.type main, @function
main:
    adrp x0, initial_state
    add x0, x0, :lo12:initial_state
    ldr q0, [x0]
    ldr q1, [x0, #16]
    adrp x1, state
    add x1, x1, :lo12:state
    str q0, [x1]
    str q1, [x1, #16]
    mov x20, #0
    mov x19, #MAX_CHUNKS
.Lmain_read:  // bound: 268435456
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, #INPUT_LEN
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lmain_failed
    mov x21, x0
    cmp x21, #INPUT_LEN
    b.hi .Lmain_assert_count
    add x20, x20, x21
    adrp x0, state
    add x0, x0, :lo12:state
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, x21
    bl hash_blocks
    cmp x21, #INPUT_LEN
    b.lo .Lmain_finish
    subs x19, x19, #1
    b.ne .Lmain_read
    b .Lmain_assert_reads
.Lmain_finish:
    mov x9, #BLOCK_BYTES
    udiv x22, x21, x9
    msub x22, x22, x9, x21
    sub x23, x21, x22
    adrp x0, state
    add x0, x0, :lo12:state
    adrp x1, input
    add x1, x1, :lo12:input
    add x1, x1, x23
    mov x2, x22
    mov x3, x20
    bl finish
    adrp x0, state
    add x0, x0, :lo12:state
    bl print_digest
    tbnz x0, #63, .Lmain_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_failed:
    mov w0, #EXIT_IO_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_count:
    mov w0, #1
    bl assert_fail
.Lmain_assert_reads:
    mov w0, #2
    bl assert_fail
.size main, . - main

// Hashes every whole block of a piece of the message.
// In: x0 = the state; x1 = the bytes; x2 = how many, up to INPUT_LEN. A partial block at the
// end is left alone.
.type hash_blocks, @function
hash_blocks:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Lhash_blocks_assert_state
    cbz x1, .Lhash_blocks_assert_bytes
    cmp x2, #INPUT_LEN
    b.hi .Lhash_blocks_assert_length
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, #BLOCK_STEPS
.Lhash_blocks_loop:  // bound: 65
    cmp x21, #BLOCK_BYTES
    b.lo .Lhash_blocks_done
    mov x0, x20
    bl schedule_words
    mov x0, x19
    bl rounds
    add x20, x20, #BLOCK_BYTES
    sub x21, x21, #BLOCK_BYTES
    subs x22, x22, #1
    b.ne .Lhash_blocks_loop
    b .Lhash_blocks_assert_blocks
.Lhash_blocks_done:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lhash_blocks_assert_state:
    mov w0, #3
    bl assert_fail
.Lhash_blocks_assert_bytes:
    mov w0, #4
    bl assert_fail
.Lhash_blocks_assert_length:
    mov w0, #5
    bl assert_fail
.Lhash_blocks_assert_blocks:
    mov w0, #6
    bl assert_fail
.size hash_blocks, . - hash_blocks

// Lays out one block's 64-word message schedule: the block's 16 words, then the other 48, four
// at a time.
// In: x0 = the block, 64 bytes.
.type schedule_words, @function
schedule_words:
    cbz x0, .Lschedule_words_assert_block
    adrp x2, schedule
    add x2, x2, :lo12:schedule
    ldr q0, [x0]
    rev32 v0.16b, v0.16b
    str q0, [x2]
    ldr q0, [x0, #16]
    rev32 v0.16b, v0.16b
    str q0, [x2, #16]
    ldr q0, [x0, #32]
    rev32 v0.16b, v0.16b
    str q0, [x2, #32]
    ldr q0, [x0, #48]
    rev32 v0.16b, v0.16b
    str q0, [x2, #48]
    add x9, x2, #16
    add x10, x2, #32
    add x11, x2, #48
    add x12, x2, #64
    mov x3, #0
    mov x4, #EXTEND_STEPS
.Lschedule_words_loop:  // bound: 12
    cmp x3, #SCHEDULE_LEN - 80 + 1
    b.hs .Lschedule_words_assert_index
    ldr q0, [x2, x3]
    ldr q1, [x9, x3]
    ldr q2, [x10, x3]
    ldr q3, [x11, x3]
    sha256su0 v0.4s, v1.4s
    sha256su1 v0.4s, v2.4s, v3.4s
    str q0, [x12, x3]
    add x3, x3, #16
    subs x4, x4, #1
    b.ne .Lschedule_words_loop
    ret
.Lschedule_words_assert_block:
    mov w0, #7
    bl assert_fail
.Lschedule_words_assert_index:
    mov w0, #8
    bl assert_fail
.size schedule_words, . - schedule_words

// Runs the 64 rounds on the message schedule, four at a time with the CPU's SHA instructions,
// and adds the result to the state.
// In: x0 = the state, eight 32-bit words.
.type rounds, @function
rounds:
    cbz x0, .Lrounds_assert_state
    ldr q0, [x0]
    ldr q1, [x0, #16]
    mov v6.16b, v0.16b
    mov v7.16b, v1.16b
    adrp x2, schedule
    add x2, x2, :lo12:schedule
    adrp x3, k_table
    add x3, x3, :lo12:k_table
    mov x4, #0
    mov x5, #ROUND_STEPS
.Lrounds_loop:  // bound: 16
    cmp x4, #SCHEDULE_LEN - 16 + 1
    b.hs .Lrounds_assert_index
    ldr q4, [x2, x4]
    ldr q5, [x3, x4]
    add v4.4s, v4.4s, v5.4s
    mov v2.16b, v0.16b
    sha256h q0, q1, v4.4s
    sha256h2 q1, q2, v4.4s
    add x4, x4, #16
    subs x5, x5, #1
    b.ne .Lrounds_loop
    add v0.4s, v0.4s, v6.4s
    add v1.4s, v1.4s, v7.4s
    str q0, [x0]
    str q1, [x0, #16]
    ret
.Lrounds_assert_state:
    mov w0, #9
    bl assert_fail
.Lrounds_assert_index:
    mov w0, #10
    bl assert_fail
.size rounds, . - rounds

// Pads the message's last bytes with a 1 bit, zeros and its length in bits, and hashes the one or
// two blocks that makes. It runs once, so padded still holds zeros.
// In: x0 = the state; x1 = the last bytes; x2 = how many, fewer than 64; x3 = the message's
// length in bytes.
.type finish, @function
finish:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Lfinish_assert_state
    cbz x1, .Lfinish_assert_tail
    cmp x2, #BLOCK_BYTES
    b.hs .Lfinish_assert_count
    mov x19, x0
    mov x20, x2
    mov x21, x3
    adrp x4, padded
    add x4, x4, :lo12:padded
    mov x5, #0
    mov x6, #MAX_COPY
.Lfinish_copy:  // bound: 63
    cmp x5, x20
    b.hs .Lfinish_copied
    cmp x5, #PADDED_LEN
    b.hs .Lfinish_assert_copy
    ldrb w7, [x1, x5]
    strb w7, [x4, x5]
    add x5, x5, #1
    subs x6, x6, #1
    b.ne .Lfinish_copy
.Lfinish_copied:
    cmp x5, #PADDED_LEN
    b.hs .Lfinish_assert_pad
    mov w7, #PAD_BYTE
    strb w7, [x4, x5]
    mov x10, #LENGTH_AT_ONE
    cmp x20, #LENGTH_AT_ONE
    b.lo .Lfinish_length
    mov x10, #LENGTH_AT_TWO
.Lfinish_length:
    lsl x21, x21, #3
    rev x21, x21
    cmp x10, #PADDED_LEN - 8 + 1
    b.hs .Lfinish_assert_length
    str x21, [x4, x10]
    mov x0, x19
    mov x1, x4
    add x2, x10, #8
    bl hash_blocks
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lfinish_assert_state:
    mov w0, #11
    bl assert_fail
.Lfinish_assert_tail:
    mov w0, #12
    bl assert_fail
.Lfinish_assert_count:
    mov w0, #13
    bl assert_fail
.Lfinish_assert_copy:
    mov w0, #14
    bl assert_fail
.Lfinish_assert_pad:
    mov w0, #15
    bl assert_fail
.Lfinish_assert_length:
    mov w0, #16
    bl assert_fail
.size finish, . - finish

// Prints the state's eight words as 64 hexadecimal digits and a newline.
// In: x0 = the state.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_digest, @function
print_digest:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lprint_digest_assert_state
    adrp x4, hex_digits
    add x4, x4, :lo12:hex_digits
    adrp x5, out
    add x5, x5, :lo12:out
    mov x1, #0
    mov x9, #0
    mov x10, #STATE_WORDS
.Lprint_digest_word:  // bound: 8
    cmp x9, #STATE_WORDS
    b.hs .Lprint_digest_assert_word
    ldr w2, [x0, x9, lsl #2]
    mov x11, #NIBBLES
.Lprint_digest_nibble:  // bound: 8
    lsr w3, w2, #28
    lsl w2, w2, #4
    cmp x3, #16
    b.hs .Lprint_digest_assert_nibble
    ldrb w3, [x4, x3]
    cmp x1, #HEX_DIGITS_COUNT
    b.hs .Lprint_digest_assert_out
    strb w3, [x5, x1]
    add x1, x1, #1
    subs x11, x11, #1
    b.ne .Lprint_digest_nibble
    add x9, x9, #1
    subs x10, x10, #1
    b.ne .Lprint_digest_word
    mov w3, #NEWLINE
    strb w3, [x5, #HEX_DIGITS_COUNT]
    mov x0, #STDOUT
    mov x1, x5
    mov x2, #OUT_LEN
    bl write_all
    tbnz x0, #63, .Lprint_digest_return
    mov x0, #0
.Lprint_digest_return:
    ldp x29, x30, [sp], #16
    ret
.Lprint_digest_assert_state:
    mov w0, #17
    bl assert_fail
.Lprint_digest_assert_word:
    mov w0, #18
    bl assert_fail
.Lprint_digest_assert_nibble:
    mov w0, #19
    bl assert_fail
.Lprint_digest_assert_out:
    mov w0, #20
    bl assert_fail
.size print_digest, . - print_digest

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
    mov w0, #30
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #31
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #32
    bl assert_fail
.size write_all, . - write_all

