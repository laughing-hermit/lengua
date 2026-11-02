.arch armv8-a
.arch_extension crc
.arch_extension aes

// System calls (Linux AArch64)
.equ SYS_READ, 63
.equ SYS_WRITE, 64
.equ SYS_EXIT, 93
// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 2
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// CRC-32C starts from all ones and ends inverted.
.equ CRC_START, 0xFFFFFFFF
// Reads fill the buffer from where the last stopped; each block is folded once it's whole, and
// a full buffer, all folded, fills again from its front.
.equ BUFFER_FULL, 1048576
// A block: a chain of crc32cx takes its first HALF bytes while carry-less multiplies fold the
// HALF after them, 64 bytes of each a step.
.equ HALF, 16384
.equ BLOCK, 32768
.equ BLOCKS, BUFFER_FULL / BLOCK
.equ VECTORS, 64
.equ FOLDS, HALF / VECTORS - 1
// The bytes after the last whole block: 64 a step while that many are left, 8 to crc32cx, then
// 8 a step; and a step's room past the last byte, so an 8-byte step's check is one on its start
.equ RUN, 64
.equ RUNS, BLOCK / RUN
.equ STEP, 8
.equ RUN_STEPS, RUN / STEP
.equ INPUT_LEN, BUFFER_FULL + STEP
// The reads the main loop takes at most
.equ MAX_READS, 1099511627776
// Printing a number
.equ MAX_DIGITS, 19
.equ MAX_WRITES, 64
.equ OUT_BUF_LEN, 21
.equ NEWLINE, 10

.section .rodata

// What carry-less multiplies move by, each x to a power mod the CRC-32C polynomial,
// bit-reflected, so a fold, or a crc32cx of the product, finishes the move: a vector's low and
// high doublewords past 64 bytes more, x^(512+64-33) and x^(512-33); past 16 more, x^(128+64-33)
// and x^(128-33); and the chain's state past HALF bytes less 8, x^(8 * (HALF - 8) - 33), into a
// vector's low doubleword, whose crc32cx and the 8 bytes after it move it the rest.
.type fold_numbers, @object
fold_numbers:
    .quad 0x740eef02
    .quad 0x9e4addf8
    .quad 0xf20c0dfe
    .quad 0x493c7d27
    .quad 0x3c8b3748
    .quad 0
.size fold_numbers, . - fold_numbers

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// Standard input as it comes, with a step's room after it.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

// The checksum's digits and newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

// Reads standard input into the buffer as it comes, folding each block into the CRC-32C state
// once it's whole, so it works while more arrive; then takes the bytes after the last block and
// prints the checksum in decimal.
.type _start, @function
_start:
    mov w20, #CRC_START
    mov x21, #0
    mov x22, #0
    mov x19, #MAX_READS
.Lstart_read:  // bound: 1099511627776
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    add x1, x1, x21
    mov x2, #BUFFER_FULL
    sub x2, x2, x21
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lstart_failed
    cbz x0, .Lstart_done
    cmp x0, x2
    b.hi .Lstart_assert_count
    add x21, x21, x0
    mov w0, w20
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, x22
    mov x3, x21
    bl crc_blocks
    mov w20, w0
    mov x22, x1
    cmp x21, #BUFFER_FULL
    b.lo .Lstart_next
    mov x21, #0
    mov x22, #0
.Lstart_next:
    subs x19, x19, #1
    b.ne .Lstart_read
    b .Lstart_assert_reads
.Lstart_done:
    mov w0, w20
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, x22
    mov x3, x21
    bl crc_bytes
    mvn w0, w0
    bl write_i64
    tbnz x0, #63, .Lstart_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_failed:
    mov w0, #EXIT_IO_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_count:
    mov w0, #1
    bl assert_fail
.Lstart_assert_reads:
    mov w0, #5
    bl assert_fail
.size _start, . - _start

// Folds each whole block of the buffer not yet folded into a CRC-32C state.
// In: w0 = the state, x1 = the buffer, input, x2 = how many of its bytes are folded, a multiple
// of BLOCK, x3 = how many it holds, from x2 up to BUFFER_FULL.
// Out: w0 = the state after the blocks, x1 = how many bytes are folded after them.
.type crc_blocks, @function
crc_blocks:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cmp x3, #BUFFER_FULL
    b.hi .Lcrc_blocks_assert_length
    cmp x2, x3
    b.hi .Lcrc_blocks_assert_folded
    mov x19, x1
    mov x20, x2
    mov x21, x3
    mov x22, #BLOCKS
.Lcrc_blocks_block:  // bound: 32
    add x9, x20, #BLOCK
    cmp x9, x21
    b.hi .Lcrc_blocks_done
    add x1, x19, x20
    bl crc_block
    add x20, x20, #BLOCK
    subs x22, x22, #1
    b.ne .Lcrc_blocks_block
.Lcrc_blocks_done:
    mov x1, x20
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lcrc_blocks_assert_length:
    mov w0, #13
    bl assert_fail
.Lcrc_blocks_assert_folded:
    mov w0, #14
    bl assert_fail
.size crc_blocks, . - crc_blocks

// Folds one block into a CRC-32C state as two parts side by side, so the CRC unit and the
// vector units work at once: a chain of crc32cx takes the block's first HALF bytes, 64 a step,
// while carry-less multiplies fold the HALF after them into four vectors of 16 bytes, 64 bytes a
// step. Then crc_join joins the two.
// In: w0 = the state; x1 = the block's bytes, BLOCK of them.
// Out: w0 = the state after them.
.type crc_block, @function
crc_block:
    stp x29, x30, [sp, #-16]!
    cbz x1, .Lcrc_block_assert_bytes
    adrp x9, fold_numbers
    add x9, x9, :lo12:fold_numbers
    ld1 { v24.2d, v25.2d, v26.2d }, [x9]
    add x6, x1, #HALF
    ld1 { v0.16b, v1.16b, v2.16b, v3.16b }, [x6], #64
    mov x7, #FOLDS
.Lcrc_block_step:  // bound: 255
    ld1 { v4.16b, v5.16b, v6.16b, v7.16b }, [x6], #64
    ldp x9, x10, [x1]
    pmull v16.1q, v0.1d, v24.1d
    pmull2 v0.1q, v0.2d, v24.2d
    pmull v17.1q, v1.1d, v24.1d
    pmull2 v1.1q, v1.2d, v24.2d
    crc32cx w0, w0, x9
    crc32cx w0, w0, x10
    ldp x11, x12, [x1, #16]
    pmull v18.1q, v2.1d, v24.1d
    pmull2 v2.1q, v2.2d, v24.2d
    pmull v19.1q, v3.1d, v24.1d
    pmull2 v3.1q, v3.2d, v24.2d
    crc32cx w0, w0, x11
    crc32cx w0, w0, x12
    ldp x13, x14, [x1, #32]
    eor v0.16b, v0.16b, v16.16b
    eor v1.16b, v1.16b, v17.16b
    eor v2.16b, v2.16b, v18.16b
    eor v3.16b, v3.16b, v19.16b
    crc32cx w0, w0, x13
    crc32cx w0, w0, x14
    ldp x15, x16, [x1, #48]
    eor v0.16b, v0.16b, v4.16b
    eor v1.16b, v1.16b, v5.16b
    eor v2.16b, v2.16b, v6.16b
    eor v3.16b, v3.16b, v7.16b
    crc32cx w0, w0, x15
    crc32cx w0, w0, x16
    add x1, x1, #VECTORS
    subs x7, x7, #1
    b.ne .Lcrc_block_step
    ldp x9, x10, [x1]
    crc32cx w0, w0, x9
    crc32cx w0, w0, x10
    ldp x11, x12, [x1, #16]
    crc32cx w0, w0, x11
    crc32cx w0, w0, x12
    ldp x13, x14, [x1, #32]
    crc32cx w0, w0, x13
    crc32cx w0, w0, x14
    ldp x15, x16, [x1, #48]
    crc32cx w0, w0, x15
    crc32cx w0, w0, x16
    bl crc_join
    ldp x29, x30, [sp], #16
    ret
.Lcrc_block_assert_bytes:
    mov w0, #17
    bl assert_fail
.size crc_block, . - crc_block

// Joins a block's two parts into the CRC-32C state after it: folds the four vectors into the
// last, puts the chain's state there too, moved past the vectors' bytes by a carry-less
// multiply, and takes the CRC of the last vector's 16 bytes from a state of 0.
// In: w0 = the chain's state after the block's first half; v0 to v3 = the vectors its second
// half folded into; v25 = what moves a vector past 16 bytes more; v26 = what moves the chain's
// state past HALF bytes less 8, in its low doubleword.
// Out: w0 = the state after the block.
.type crc_join, @function
crc_join:
    pmull v16.1q, v0.1d, v25.1d
    pmull2 v17.1q, v0.2d, v25.2d
    eor v1.16b, v1.16b, v16.16b
    eor v1.16b, v1.16b, v17.16b
    pmull v16.1q, v1.1d, v25.1d
    pmull2 v17.1q, v1.2d, v25.2d
    eor v2.16b, v2.16b, v16.16b
    eor v2.16b, v2.16b, v17.16b
    pmull v16.1q, v2.1d, v25.1d
    pmull2 v17.1q, v2.2d, v25.2d
    eor v3.16b, v3.16b, v16.16b
    eor v3.16b, v3.16b, v17.16b
    fmov d16, x0
    pmull v16.1q, v16.1d, v26.1d
    eor v3.16b, v3.16b, v16.16b
    fmov x9, d3
    umov x10, v3.d[1]
    mov w0, #0
    crc32cx w0, w0, x9
    crc32cx w0, w0, x10
    ret
.size crc_join, . - crc_join

// Takes the bytes from x2 to x3 of the buffer into a CRC-32C state with crc32cx, 8 bytes each:
// 64 bytes a step, loaded in pairs, while that many are left, then 8 bytes a step, then the rest
// one at a time. A core passes each crc32cx's result straight to the next, so one chain keeps
// its CRC unit busy.
// In: w0 = the state, x1 = the buffer, input, x2 = the first byte's place, x3 = the end's, from
// x2 up to BUFFER_FULL and less than a block past x2.
// Out: w0 = the state after them.
.type crc_bytes, @function
crc_bytes:
    cmp x3, #BUFFER_FULL
    b.hi .Lcrc_bytes_assert_length
    cmp x2, x3
    b.hi .Lcrc_bytes_assert_start
    sub x9, x3, x2
    cmp x9, #BLOCK
    b.hs .Lcrc_bytes_assert_block
    mov x10, x2
    mov x11, #RUNS
.Lcrc_bytes_runs:  // bound: 512
    add x12, x10, #RUN
    cmp x12, x3
    b.hi .Lcrc_bytes_words
    add x13, x1, x10
    ldp x14, x15, [x13]
    crc32cx w0, w0, x14
    crc32cx w0, w0, x15
    ldp x14, x15, [x13, #16]
    crc32cx w0, w0, x14
    crc32cx w0, w0, x15
    ldp x14, x15, [x13, #32]
    crc32cx w0, w0, x14
    crc32cx w0, w0, x15
    ldp x14, x15, [x13, #48]
    crc32cx w0, w0, x14
    crc32cx w0, w0, x15
    mov x10, x12
    subs x11, x11, #1
    b.ne .Lcrc_bytes_runs
.Lcrc_bytes_words:
    mov x11, #RUN_STEPS
.Lcrc_bytes_steps:  // bound: 8
    add x12, x10, #STEP
    cmp x12, x3
    b.hi .Lcrc_bytes_tail
    cmp x10, #BUFFER_FULL
    b.hi .Lcrc_bytes_assert_step
    ldr x13, [x1, x10]
    crc32cx w0, w0, x13
    add x10, x10, #STEP
    subs x11, x11, #1
    b.ne .Lcrc_bytes_steps
.Lcrc_bytes_tail:
    mov x11, #STEP
.Lcrc_bytes_bytes:  // bound: 8
    cmp x10, x3
    b.hs .Lcrc_bytes_return
    cmp x10, #BUFFER_FULL
    b.hs .Lcrc_bytes_assert_byte
    ldrb w13, [x1, x10]
    crc32cb w0, w0, w13
    add x10, x10, #1
    subs x11, x11, #1
    b.ne .Lcrc_bytes_bytes
.Lcrc_bytes_return:
    ret
.Lcrc_bytes_assert_length:
    mov w0, #2
    bl assert_fail
.Lcrc_bytes_assert_step:
    mov w0, #3
    bl assert_fail
.Lcrc_bytes_assert_byte:
    mov w0, #4
    bl assert_fail
.Lcrc_bytes_assert_start:
    mov w0, #15
    bl assert_fail
.Lcrc_bytes_assert_block:
    mov w0, #16
    bl assert_fail
.size crc_bytes, . - crc_bytes

// Writes a whole number in decimal, then a newline, to standard output.
// In: x0 = the number, never -9223372036854775808.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_i64, @function
write_i64:
    stp x29, x30, [sp, #-16]!
    mov x9, x0
    cmp x0, #0
    b.ge .Lwrite_i64_digits
    negs x0, x0
    b.vs .Lwrite_i64_assert_min
.Lwrite_i64_digits:
    adrp x1, out_buf
    add x1, x1, :lo12:out_buf
    mov w10, #NEWLINE
    strb w10, [x1, #OUT_BUF_LEN - 1]
    mov x2, #OUT_BUF_LEN - 1
    mov x11, #10
    mov x3, #MAX_DIGITS
.Lwrite_i64_digit:  // bound: 19
    udiv x12, x0, x11
    msub x13, x12, x11, x0
    add w13, w13, #'0'
    sub x2, x2, #1
    cmp x2, #OUT_BUF_LEN
    b.hs .Lwrite_i64_assert_digit_index
    strb w13, [x1, x2]
    mov x0, x12
    cbz x0, .Lwrite_i64_sign
    subs x3, x3, #1
    b.ne .Lwrite_i64_digit
    b .Lwrite_i64_assert_digits
.Lwrite_i64_sign:
    tbz x9, #63, .Lwrite_i64_output
    sub x2, x2, #1
    cmp x2, #OUT_BUF_LEN
    b.hs .Lwrite_i64_assert_sign_index
    mov w10, #'-'
    strb w10, [x1, x2]
.Lwrite_i64_output:
    add x1, x1, x2
    mov x10, #OUT_BUF_LEN
    sub x2, x10, x2
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lwrite_i64_return
    mov x0, #0
.Lwrite_i64_return:
    ldp x29, x30, [sp], #16
    ret
.Lwrite_i64_assert_min:
    mov w0, #6
    bl assert_fail
.Lwrite_i64_assert_digit_index:
    mov w0, #7
    bl assert_fail
.Lwrite_i64_assert_digits:
    mov w0, #8
    bl assert_fail
.Lwrite_i64_assert_sign_index:
    mov w0, #9
    bl assert_fail
.size write_i64, . - write_i64

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
    mov w0, #10
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #11
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #12
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
