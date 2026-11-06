.arch armv8-a

// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_READ_FAILED, 3
.equ EXIT_WRITE_FAILED, 4
// fill's reads at most, and what makes it read to the end, as assets/linux-aarch64/fill.s has
// them
.equ ERROR_IO, -5
.equ READ_TO_END, -1
// A buffer-full of input, the most the main loop takes at a time, and its 16-byte steps
.equ BUF_LEN, 4096
.equ CHUNK, 16
.equ BUF_CHUNKS, BUF_LEN / CHUNK
// Where the last step may start, and one past it
.equ STEP_LIMIT, BUF_LEN - CHUNK + 1
// The buffer-fulls the main loop takes at most
.equ MAX_FULLS, 1099511627776
.equ MAX_WRITES, 64
.equ MAX_DIGITS, 19
.equ NEWLINE, 10
.equ OUT_BUF_LEN, 21

.section .rodata

// Each lane's place in a vector, 0 to 15: a lane is a byte of the input where its place is
// below the bytes left.
.type places, @object
places:
    .byte 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15
.size places, . - places

.section .bss

// One buffer-full of standard input.
// shared: _start fills it, and count_lines reads it
.type buffer, @object
buffer:
    .zero BUF_LEN
.size buffer, . - buffer

// Sign, 19 digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
// Counts the newlines of standard input of any length, a buffer-full at a time through fill, and
// prints the count.
.type main, @function
main:
    mov x21, #0
    mov x19, #MAX_FULLS
.Lmain_full:  // bound: 1099511627776
    mov x0, #STDIN
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, #BUF_LEN
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lmain_read_failed
    cbz x0, .Lmain_print
    cmp x0, #BUF_LEN
    b.hi .Lmain_assert_count
    mov x20, x0
    bl count_lines
    add x21, x21, x0
    cmp x20, #BUF_LEN
    b.lo .Lmain_print
    subs x19, x19, #1
    b.ne .Lmain_full
.Lmain_print:
    mov x0, x21
    bl write_i64
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_read_failed:
    mov w0, #EXIT_READ_FAILED
    b .Lmain_exit
.Lmain_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_count:
    mov w0, #1
    bl assert_fail
.size main, . - main

// Counts the newlines in a piece of the buffer, 16 bytes a step with NEON, each step's bytes past
// the end masked off, lanes whose place isn't below the bytes left, so the last partial step
// needs no code of its own. A comparison's lanes, all ones or zero, become 1 or 0 with ushr by
// 7, and addv adds them.
// In: x0 = how many bytes the buffer holds, 1 to BUF_LEN.
// Out: x0 = the newlines among them.
.type count_lines, @function
count_lines:
    cmp x0, #BUF_LEN
    b.hi .Lcount_lines_assert_length
    mov x1, x0
    adrp x0, buffer
    add x0, x0, :lo12:buffer
    adrp x3, places
    add x3, x3, :lo12:places
    ldr q16, [x3]
    movi v17.16b, #NEWLINE
    mov x8, #0
    mov x10, #0
    mov x12, #STEP_LIMIT
    mov x11, #BUF_CHUNKS
.Lcount_lines_loop:  // bound: 256
    cmp x10, x1
    b.hs .Lcount_lines_return
    cmp x10, x12
    b.hs .Lcount_lines_assert_index
    // This step's bytes before the end, min(x1 - x10, 16), and the lanes that hold them.
    sub x4, x1, x10
    mov x5, #CHUNK
    cmp x4, x5
    csel x4, x4, x5, lo
    dup v22.16b, w4
    cmhi v22.16b, v22.16b, v16.16b
    ldr q0, [x0, x10]
    cmeq v1.16b, v0.16b, v17.16b
    and v1.16b, v1.16b, v22.16b
    ushr v1.16b, v1.16b, #7
    addv b1, v1.16b
    umov w6, v1.b[0]
    add x8, x8, x6
    add x10, x10, #CHUNK
    subs x11, x11, #1
    b.ne .Lcount_lines_loop
.Lcount_lines_return:
    mov x0, x8
    ret
.Lcount_lines_assert_length:
    mov w0, #2
    bl assert_fail
.Lcount_lines_assert_index:
    mov w0, #3
    bl assert_fail
.size count_lines, . - count_lines

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
    mov w0, #10
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #11
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #12
    bl assert_fail
.size write_all, . - write_all

