.arch armv8-a
.arch_extension crc

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
// A read's bytes at most; 64 a step while that many are left, 8 to crc32cx, then 8 a step;
// and a step's room past the last byte, so an 8-byte step's check is one on its start
.equ BUFFER_FULL, 1048576
.equ BLOCK, 64
.equ BLOCKS, BUFFER_FULL / BLOCK
.equ STEP, 8
.equ BLOCK_STEPS, BLOCK / STEP
.equ INPUT_LEN, BUFFER_FULL + STEP
// The reads the main loop takes at most
.equ MAX_READS, 1099511627776
// Printing a number
.equ MAX_DIGITS, 19
.equ MAX_WRITES, 64
.equ OUT_BUF_LEN, 21
.equ NEWLINE, 10

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// One read of standard input, with a step's room after it.
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

// Reads standard input a read at a time and takes each read's bytes into the CRC-32C state as
// they come, so it works while more arrive; then prints the checksum in decimal.
.type _start, @function
_start:
    mov w20, #CRC_START
    mov x19, #MAX_READS
.Lstart_read:  // bound: 1099511627776
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, #BUFFER_FULL
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lstart_failed
    cbz x0, .Lstart_done
    cmp x0, #BUFFER_FULL
    b.hi .Lstart_assert_count
    mov x2, x0
    mov w0, w20
    adrp x1, input
    add x1, x1, :lo12:input
    bl crc_bytes
    mov w20, w0
    subs x19, x19, #1
    b.ne .Lstart_read
    b .Lstart_assert_reads
.Lstart_done:
    mvn w0, w20
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

// Takes bytes into a CRC-32C state with crc32cx, 8 bytes each: 64 bytes a step, loaded in
// pairs, while that many are left, then 8 bytes a step, then the rest one at a time. A core
// passes each crc32cx's result straight to the next, so one chain keeps its CRC unit busy.
// In: w0 = the state, x1 = the bytes, in a buffer of INPUT_LEN, x2 = how many, at most
// BUFFER_FULL.
// Out: w0 = the state after them.
.type crc_bytes, @function
crc_bytes:
    cmp x2, #BUFFER_FULL
    b.hi .Lcrc_bytes_assert_length
    mov x10, #0
    mov x11, #BLOCKS
.Lcrc_bytes_blocks:  // bound: 16384
    add x12, x10, #BLOCK
    cmp x12, x2
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
    b.ne .Lcrc_bytes_blocks
.Lcrc_bytes_words:
    mov x11, #BLOCK_STEPS
.Lcrc_bytes_steps:  // bound: 8
    add x12, x10, #STEP
    cmp x12, x2
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
    cmp x10, x2
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
