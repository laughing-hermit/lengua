.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_WRITE, 64
.equ SYS_EXIT, 93
// File descriptors
.equ STDOUT, 1
.equ STDERR, 2
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_NOT_A_NUMBER, 2
.equ EXIT_WRITE_FAILED, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
// Sizes and limits
.equ ARGC_EXPECTED, 3
.equ MIN_VALUE, -1000000000
.equ MAX_VALUE, 1000000000
.equ MAX_CHARS, 20
.equ MAX_DIGITS, 19
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ USAGE_MSG_LEN, 19
.equ NAN_MSG_LEN, 20
.equ OUT_BUF_LEN, 21

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: add <a> <b>\n"
.size usage_msg, . - usage_msg

.type nan_msg, @object
nan_msg:
    .ascii "error: not a number\n"
.size nan_msg, . - nan_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// Sign, 19 digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

// Adds the two whole numbers its arguments hold, each from MIN_VALUE to MAX_VALUE, and prints
// the sum.
.type _start, @function
_start:
    ldr x0, [sp]
    cmp x0, #ARGC_EXPECTED
    b.ne .Lstart_usage
    ldr x19, [sp, #16]
    ldr x20, [sp, #24]
    cbz x19, .Lstart_assert_first
    cbz x20, .Lstart_assert_second
    mov x22, #MAX_VALUE & 0xffff
    movk x22, #MAX_VALUE >> 16, lsl #16
    mov x0, x19
    bl parse_i64
    tbnz x0, #63, .Lstart_not_a_number
    // From MIN_VALUE to MAX_VALUE: at most MAX_VALUE, and at least its negative.
    cmp x1, x22
    b.gt .Lstart_not_a_number
    cmn x1, x22
    b.lt .Lstart_not_a_number
    mov x21, x1
    mov x0, x20
    bl parse_i64
    tbnz x0, #63, .Lstart_not_a_number
    cmp x1, x22
    b.gt .Lstart_not_a_number
    cmn x1, x22
    b.lt .Lstart_not_a_number
    adds x0, x21, x1
    b.vs .Lstart_assert_sum
    bl write_i64
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_usage:
    mov x0, #STDERR
    adrp x1, usage_msg
    add x1, x1, :lo12:usage_msg
    mov x2, #USAGE_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_USAGE
    b .Lstart_exit
.Lstart_not_a_number:
    mov x0, #STDERR
    adrp x1, nan_msg
    add x1, x1, :lo12:nan_msg
    mov x2, #NAN_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_NOT_A_NUMBER
    b .Lstart_exit
.Lstart_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_first:
    mov w0, #1
    bl assert_fail
.Lstart_assert_second:
    mov w0, #2
    bl assert_fail
.Lstart_assert_sum:
    mov w0, #3
    bl assert_fail
.size _start, . - _start

// Parses a decimal whole number with an optional leading minus sign.
// In: x0 = a NUL-terminated string.
// Out: x0 = 0 and x1 = the number, or x0 = ERROR_INVALID when the text isn't a number
// between -9223372036854775807 and 9223372036854775807.
// returns: result
.type parse_i64, @function
parse_i64:
    cbz x0, .Lparse_assert_null
    mov x9, #0
    ldrb w10, [x0]
    cmp w10, #'-'
    b.ne .Lparse_digits
    mov x9, #1
    add x0, x0, #1
.Lparse_digits:
    ldrb w10, [x0]
    cbz w10, .Lparse_invalid
    mov x1, #0
    mov x2, #0
    mov x11, #10
    mov x3, #MAX_CHARS
.Lparse_loop:  // bound: 20
    cmp x2, #MAX_CHARS
    b.hs .Lparse_assert_index
    ldrb w10, [x0, x2]
    cbz w10, .Lparse_end
    sub w10, w10, #'0'
    cmp w10, #9
    b.hi .Lparse_invalid
    mul x12, x1, x11
    smulh x13, x1, x11
    cmp x13, x12, asr #63
    b.ne .Lparse_invalid
    adds x1, x12, x10
    b.vs .Lparse_invalid
    add x2, x2, #1
    subs x3, x3, #1
    b.ne .Lparse_loop
    b .Lparse_invalid
.Lparse_end:
    cbz x9, .Lparse_positive
    neg x1, x1
.Lparse_positive:
    mov x0, #0
    b .Lparse_return
.Lparse_invalid:
    mov x0, #ERROR_INVALID
.Lparse_return:
    ret
.Lparse_assert_null:
    mov w0, #4
    bl assert_fail
.Lparse_assert_index:
    mov w0, #5
    bl assert_fail
.size parse_i64, . - parse_i64

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
