.arch armv8-a

// SHA-256 in plain integer code (rotates, adds and xors) and NEON for the byte order and the
// copies, so it runs on every ARM64 computer. The form that uses the CPU's SHA2 instructions,
// from the very same spec, is plugin/skills/compile/assets/linux-aarch64/fast/sha256-sha2.

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
.equ EXTEND_STEPS, 48
.equ ROUND_STEPS, 64
.equ WORK_A_LEN, 272
.equ WORK_E_LEN, 272
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

// The working variable a of every round, newest first: the words of a list that grows downward.
// The state's a, b, c and d are its last four words before the first round; round t reads four
// words (a, b, c, d) and writes the next a in the word before them. After the 64th round the
// first four words are the new a, b, c and d. rounds owns it and lends sha_round the place of
// each round.
.type work_a, @object
work_a:
    .zero WORK_A_LEN
.size work_a, . - work_a

// The working variable e of every round, newest first, as work_a keeps a: e, f, g and h last.
.type work_e, @object
work_e:
    .zero WORK_E_LEN
.size work_e, . - work_e

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

// Lays out one block's 64-word message schedule, with plain integer code: the block's 16 words,
// big-endian (NEON reverses the bytes of four words at a time), then each of the other 48 from
// four before it: W[t] = sigma1(W[t-2]) + W[t-7] + sigma0(W[t-15]) + W[t-16].
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
    add x9, x2, #4
    add x10, x2, #36
    add x11, x2, #56
    add x12, x2, #64
    mov x3, #0
    mov x4, #EXTEND_STEPS
.Lschedule_words_loop:  // bound: 48
    cmp x3, #SCHEDULE_LEN - 64 - 4 + 1
    b.hs .Lschedule_words_assert_index
    ldr w5, [x11, x3]
    ror w6, w5, #17
    eor w6, w6, w5, ror #19
    eor w6, w6, w5, lsr #10
    ldr w5, [x9, x3]
    ror w7, w5, #7
    eor w7, w7, w5, ror #18
    eor w7, w7, w5, lsr #3
    add w6, w6, w7
    ldr w7, [x10, x3]
    add w6, w6, w7
    ldr w7, [x2, x3]
    add w6, w6, w7
    str w6, [x12, x3]
    add x3, x3, #4
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

// Runs the 64 rounds on the message schedule, one at a time, and adds the result to the state.
// The working variables of a round are the start of two lists that grow downward in memory
// (work_a and work_e), not eight registers that each round moves: the state's words are copied
// in as they stand, as the last four of each list, and the first four come out.
// In: x0 = the state, eight 32-bit words.
.type rounds, @function
rounds:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x0, .Lrounds_assert_state
    mov x19, x0
    adrp x21, work_a
    add x21, x21, :lo12:work_a
    adrp x22, work_e
    add x22, x22, :lo12:work_e
    ldr q0, [x19]
    str q0, [x21, #256]
    ldr q0, [x19, #16]
    str q0, [x22, #256]
    add x21, x21, #256
    add x22, x22, #256
    adrp x23, k_table
    add x23, x23, :lo12:k_table
    adrp x24, schedule
    add x24, x24, :lo12:schedule
    mov x20, #ROUND_STEPS
.Lrounds_loop:  // bound: 64
    sub x21, x21, #4
    sub x22, x22, #4
    mov x0, x21
    mov x1, x22
    mov x2, x23
    mov x3, x24
    bl sha_round
    add x23, x23, #4
    add x24, x24, #4
    subs x20, x20, #1
    b.ne .Lrounds_loop
    ldr q0, [x21]
    ldr q1, [x19]
    add v0.4s, v0.4s, v1.4s
    str q0, [x19]
    ldr q0, [x22]
    ldr q1, [x19, #16]
    add v0.4s, v0.4s, v1.4s
    str q0, [x19, #16]
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lrounds_assert_state:
    mov w0, #9
    bl assert_fail
.size rounds, . - rounds

// One round of the compression function, the sum of its two halves:
// T1 = h + Sigma1(e) + Ch(e, f, g) + K[t] + W[t] and T2 = Sigma0(a) + Maj(a, b, c), then
// a = T1 + T2 and e = d + T1.
// In: x0 = the place in work_a for the new a, the word before the round's a, b, c and d; x1 =
// the same in work_e, for the new e and the round's e, f, g and h; x2 = K[t]; x3 = W[t].
.type sha_round, @function
sha_round:
    ldp w4, w5, [x0, #4]
    ldp w6, w7, [x0, #12]
    ldp w8, w9, [x1, #4]
    ldp w10, w11, [x1, #12]
    ldr w12, [x2]
    ldr w13, [x3]
    ror w14, w8, #6
    eor w14, w14, w8, ror #11
    eor w14, w14, w8, ror #25
    eor w15, w9, w10
    and w15, w15, w8
    eor w15, w15, w10
    add w14, w14, w15
    add w14, w14, w11
    add w14, w14, w12
    add w14, w14, w13
    ror w15, w4, #2
    eor w15, w15, w4, ror #13
    eor w15, w15, w4, ror #22
    orr w12, w4, w5
    and w12, w12, w6
    and w13, w4, w5
    orr w12, w12, w13
    add w15, w15, w12
    add w7, w7, w14
    str w7, [x1]
    add w15, w15, w14
    str w15, [x0]
    ret
.size sha_round, . - sha_round

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

