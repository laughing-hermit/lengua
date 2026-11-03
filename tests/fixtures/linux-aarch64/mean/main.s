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
.equ EXIT_OUT_OF_RANGE, 3
.equ EXIT_WRITE_FAILED, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_RANGE, -34
.equ ERROR_INVALID, -22
// Sizes and limits: numbers in cents, up to 1000000.00
.equ ARGC_EXPECTED, 3
.equ MAX_ARG_LEN, 131072
.equ MAX_WHOLE, 1000000
.equ MAX_CENTS, 100000000
.equ CENTS_PER_UNIT, 100
.equ MAX_DIGITS, 19
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ USAGE_MSG_LEN, 20
.equ NAN_MSG_LEN, 20
.equ RANGE_MSG_LEN, 20
.equ NEWLINE_MSG_LEN, 1
.equ OUT_BUF_LEN, 24

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: mean <a> <b>\n"
.size usage_msg, . - usage_msg

.type nan_msg, @object
nan_msg:
    .ascii "mean: not a number: "
.size nan_msg, . - nan_msg

.type range_msg, @object
range_msg:
    .ascii "mean: out of range: "
.size range_msg, . - range_msg

.type newline_msg, @object
newline_msg:
    .ascii "\n"
.size newline_msg, . - newline_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// Sign, digits, a point, two decimals and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

// Parses both numbers in cents and prints their mean with two decimals, a half cent rounded
// away from zero.
.type _start, @function
_start:
    ldr x0, [sp]
    cmp x0, #ARGC_EXPECTED
    b.ne .Lstart_usage
    ldr x19, [sp, #16]
    ldr x20, [sp, #24]
    cbz x19, .Lstart_assert_argv_a
    cbz x20, .Lstart_assert_argv_b
    mov x0, x19
    bl parse_cents
    tbnz x0, #63, .Lstart_a_bad
    mov x21, x1
    mov x0, x20
    bl parse_cents
    tbnz x0, #63, .Lstart_bad
    // The sum, a cent further from zero, halved toward zero: the mean, its half cent rounded
    // away from zero.
    add x9, x21, x1
    add x10, x9, #1
    sub x11, x9, #1
    cmp x9, #0
    csel x9, x11, x10, lt
    mov x12, #2
    sdiv x0, x9, x12
    bl print_cents
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_a_bad:
    mov x20, x19
.Lstart_bad:
    mov x21, x0
    mov x1, x20
    bl report_bad_number
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_OUT_OF_RANGE
    cmp x21, #ERROR_RANGE
    b.eq .Lstart_exit
    mov w0, #EXIT_NOT_A_NUMBER
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
.Lstart_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_argv_a:
    mov w0, #1
    bl assert_fail
.Lstart_assert_argv_b:
    mov w0, #2
    bl assert_fail
.size _start, . - _start

// Prints the message for an argument parse_cents rejected, and the argument.
// In: x0 = ERROR_INVALID or ERROR_RANGE; x1 = the argument.
// Out: x0 = 0, or a negative error code.
// returns: result
.type report_bad_number, @function
report_bad_number:
    stp x29, x30, [sp, #-16]!
    cbz x1, .Lreport_bad_assert_null
    mov x2, x1
    cmp x0, #ERROR_INVALID
    b.eq .Lreport_bad_not_a_number
    cmp x0, #ERROR_RANGE
    b.eq .Lreport_bad_out_of_range
    b .Lreport_bad_assert_error
.Lreport_bad_not_a_number:
    adrp x0, nan_msg
    add x0, x0, :lo12:nan_msg
    mov x1, #NAN_MSG_LEN
    b .Lreport_bad_write
.Lreport_bad_out_of_range:
    adrp x0, range_msg
    add x0, x0, :lo12:range_msg
    mov x1, #RANGE_MSG_LEN
.Lreport_bad_write:
    bl report_error
    tbnz x0, #63, .Lreport_bad_return
    mov x0, #0
.Lreport_bad_return:
    ldp x29, x30, [sp], #16
    ret
.Lreport_bad_assert_null:
    mov w0, #3
    bl assert_fail
.Lreport_bad_assert_error:
    mov w0, #18
    bl assert_fail
.size report_bad_number, . - report_bad_number

// Parses a decimal number of cents: an optional minus sign, digits, and optionally a point and
// one or two digits.
// In: x0 = the text, ending with 0.
// Out: x0 = 0 and x1 = the cents; x0 = ERROR_INVALID when it isn't such a number, ERROR_RANGE
// when it is one past MAX_CENTS from 0.
// returns: result
.type parse_cents, @function
parse_cents:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    cbz x0, .Lparse_assert_null
    mov x19, #0
    ldrb w9, [x0]
    cmp w9, #'-'
    b.ne .Lparse_whole
    mov x19, #1
    add x0, x0, #1
.Lparse_whole:
    mov x10, #0
    mov x11, #0
    mov x12, #MAX_WHOLE & 0xffff
    movk x12, #MAX_WHOLE >> 16, lsl #16
    mov x13, #10
    mov x14, #MAX_ARG_LEN
    mov x15, #MAX_ARG_LEN
.Lparse_loop:  // bound: 131072
    cmp x11, x14
    b.hs .Lparse_assert_index
    ldrb w9, [x0, x11]
    sub w9, w9, #'0'
    cmp w9, #9
    b.hi .Lparse_whole_end
    // Past the limit the number stays out of range, however many digits follow.
    cmp x10, x12
    b.hi .Lparse_next
    madd x10, x10, x13, x9
.Lparse_next:
    add x11, x11, #1
    subs x15, x15, #1
    b.ne .Lparse_loop
    b .Lparse_invalid
.Lparse_whole_end:
    cbz x11, .Lparse_invalid
    mov x13, #CENTS_PER_UNIT
    mul x20, x10, x13
    add x0, x0, x11
    bl parse_fraction
    tbnz x0, #63, .Lparse_return
    add x10, x20, x1
    mov x12, #MAX_CENTS & 0xffff
    movk x12, #MAX_CENTS >> 16, lsl #16
    cmp x10, x12
    b.hi .Lparse_out_of_range
    cbz x19, .Lparse_positive
    neg x10, x10
.Lparse_positive:
    mov x1, x10
    mov x0, #0
    b .Lparse_return
.Lparse_out_of_range:
    mov x0, #ERROR_RANGE
    b .Lparse_return
.Lparse_invalid:
    mov x0, #ERROR_INVALID
.Lparse_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lparse_assert_null:
    mov w0, #4
    bl assert_fail
.Lparse_assert_index:
    mov w0, #5
    bl assert_fail
.size parse_cents, . - parse_cents

// Parses what follows a number's whole part: nothing, or a point and one or two digits.
// In: x0 = where it starts.
// Out: x0 = 0 and x1 = the cents it adds; x0 = ERROR_INVALID for anything else.
// returns: result
.type parse_fraction, @function
parse_fraction:
    cbz x0, .Lparse_fraction_assert_null
    mov x1, #0
    ldrb w2, [x0]
    cmp w2, #'0'
    b.lo .Lparse_fraction_not_digit
    cmp w2, #'9'
    b.ls .Lparse_fraction_assert_digit
.Lparse_fraction_not_digit:
    cbz w2, .Lparse_fraction_done
    cmp w2, #'.'
    b.ne .Lparse_fraction_invalid
    ldrb w2, [x0, #1]
    sub w2, w2, #'0'
    cmp w2, #9
    b.hi .Lparse_fraction_invalid
    mov w3, #10
    mul w1, w2, w3
    ldrb w2, [x0, #2]
    cbz w2, .Lparse_fraction_done
    sub w2, w2, #'0'
    cmp w2, #9
    b.hi .Lparse_fraction_invalid
    add w1, w1, w2
    ldrb w2, [x0, #3]
    cbnz w2, .Lparse_fraction_invalid
.Lparse_fraction_done:
    mov x0, #0
    b .Lparse_fraction_return
.Lparse_fraction_invalid:
    mov x0, #ERROR_INVALID
.Lparse_fraction_return:
    ret
.Lparse_fraction_assert_null:
    mov w0, #19
    bl assert_fail
.Lparse_fraction_assert_digit:
    mov w0, #20
    bl assert_fail
.size parse_fraction, . - parse_fraction

// Prints a number of cents as a decimal with two places, then a newline.
// In: x0 = the cents, never -9223372036854775808.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_cents, @function
print_cents:
    stp x29, x30, [sp, #-16]!
    mov x9, x0
    cmp x0, #0
    b.ge .Lprint_cents_digits
    negs x0, x0
    b.vs .Lprint_cents_assert_min
.Lprint_cents_digits:
    adrp x1, out_buf
    add x1, x1, :lo12:out_buf
    mov w10, #NEWLINE
    strb w10, [x1, #OUT_BUF_LEN - 1]
    mov x11, #10
    udiv x12, x0, x11
    msub x13, x12, x11, x0
    add w13, w13, #'0'
    strb w13, [x1, #OUT_BUF_LEN - 2]
    mov x0, x12
    udiv x12, x0, x11
    msub x13, x12, x11, x0
    add w13, w13, #'0'
    strb w13, [x1, #OUT_BUF_LEN - 3]
    mov x0, x12
    mov w10, #'.'
    strb w10, [x1, #OUT_BUF_LEN - 4]
    mov x2, #OUT_BUF_LEN - 4
    mov x3, #MAX_DIGITS
.Lprint_cents_digit:  // bound: 19
    udiv x12, x0, x11
    msub x13, x12, x11, x0
    add w13, w13, #'0'
    sub x2, x2, #1
    cmp x2, #OUT_BUF_LEN
    b.hs .Lprint_cents_assert_digit_index
    strb w13, [x1, x2]
    mov x0, x12
    cbz x0, .Lprint_cents_sign
    subs x3, x3, #1
    b.ne .Lprint_cents_digit
    b .Lprint_cents_assert_digits
.Lprint_cents_sign:
    tbz x9, #63, .Lprint_cents_output
    sub x2, x2, #1
    cmp x2, #OUT_BUF_LEN
    b.hs .Lprint_cents_assert_sign_index
    mov w10, #'-'
    strb w10, [x1, x2]
.Lprint_cents_output:
    add x1, x1, x2
    mov x10, #OUT_BUF_LEN
    sub x2, x10, x2
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lprint_cents_return
    mov x0, #0
.Lprint_cents_return:
    ldp x29, x30, [sp], #16
    ret
.Lprint_cents_assert_min:
    mov w0, #6
    bl assert_fail
.Lprint_cents_assert_digit_index:
    mov w0, #7
    bl assert_fail
.Lprint_cents_assert_digits:
    mov w0, #8
    bl assert_fail
.Lprint_cents_assert_sign_index:
    mov w0, #9
    bl assert_fail
.size print_cents, . - print_cents

// Prints a message, an argument and a newline to standard error.
// In: x0 = the message; x1 = its length; x2 = the argument, ending with 0.
// Out: x0 = 0, or a negative error code.
// returns: result
.type report_error, @function
report_error:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    cbz x0, .Lreport_error_assert_message
    cbz x2, .Lreport_error_assert_argument
    mov x19, x2
    mov x2, x1
    mov x1, x0
    mov x0, #STDERR
    bl write_all
    tbnz x0, #63, .Lreport_error_return
    mov x0, x19
    bl str_len
    cbz x0, .Lreport_error_newline
    mov x2, x0
    mov x1, x19
    mov x0, #STDERR
    bl write_all
    tbnz x0, #63, .Lreport_error_return
.Lreport_error_newline:
    mov x0, #STDERR
    adrp x1, newline_msg
    add x1, x1, :lo12:newline_msg
    mov x2, #NEWLINE_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lreport_error_return
    mov x0, #0
.Lreport_error_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lreport_error_assert_message:
    mov w0, #21
    bl assert_fail
.Lreport_error_assert_argument:
    mov w0, #22
    bl assert_fail
.size report_error, . - report_error

// The length of a text that ends with 0.
// In: x0 = the text.
// Out: x0 = its length, up to MAX_ARG_LEN.
.type str_len, @function
str_len:
    cbz x0, .Lstr_len_assert_null
    mov x1, x0
    mov x0, #0
    mov x3, #MAX_ARG_LEN
    mov x2, #MAX_ARG_LEN
.Lstr_len_loop:  // bound: 131072
    cmp x0, x3
    b.hs .Lstr_len_assert_index
    ldrb w4, [x1, x0]
    cbz w4, .Lstr_len_return
    add x0, x0, #1
    subs x2, x2, #1
    b.ne .Lstr_len_loop
    b .Lstr_len_assert_long
.Lstr_len_return:
    ret
.Lstr_len_assert_null:
    mov w0, #23
    bl assert_fail
.Lstr_len_assert_index:
    mov w0, #24
    bl assert_fail
.Lstr_len_assert_long:
    mov w0, #25
    bl assert_fail
.size str_len, . - str_len

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

