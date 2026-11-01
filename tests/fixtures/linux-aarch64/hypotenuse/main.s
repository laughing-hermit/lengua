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
.equ ERROR_INVALID, -22
.equ ERROR_RANGE, -34
// Sizes and limits
.equ ARGC_EXPECTED, 3
.equ MAX_ARG_LEN, 131072
.equ MAX_PLACES, 3
.equ MAX_WHOLE, 1000000
.equ UNITS, 1000
.equ MAX_UNITS, 1000000000
.equ MAX_WRITES, 64
.equ MAX_INT_DIGITS, 16
.equ NEWLINE, 10
.equ USAGE_MSG_LEN, 26
.equ NAN_MSG_LEN, 28
.equ RANGE_MSG_LEN, 25
.equ OUT_BUF_LEN, 21
.equ NEWLINE_AT, 20

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: hypotenuse <a> <b>\n"
.size usage_msg, . - usage_msg

.type nan_msg, @object
nan_msg:
    .ascii "error: side is not a number\n"
.size nan_msg, . - nan_msg

.type range_msg, @object
range_msg:
    .ascii "error: side out of range\n"
.size range_msg, . - range_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// Up to 16 whole digits, a point, 3 decimals and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

// Checks the argument count, parses both sides, prints the hypotenuse and exits.
.type _start, @function
_start:
    ldr x0, [sp]
    cmp x0, #ARGC_EXPECTED
    b.ne .Lstart_usage
    ldr x19, [sp, #16]
    ldr x20, [sp, #24]
    cbz x19, .Lstart_assert_argv1
    cbz x20, .Lstart_assert_argv2
    mov x0, x19
    bl parse_side
    tbnz x0, #63, .Lstart_bad_side
    mov x21, x1
    mov x0, x20
    bl parse_side
    tbnz x0, #63, .Lstart_bad_side
    mov x0, x21
    bl print_hypotenuse
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_bad_side:
    cmp x0, #ERROR_INVALID
    b.eq .Lstart_not_a_number
    cmp x0, #ERROR_RANGE
    b.eq .Lstart_out_of_range
    b .Lstart_assert_error
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
.Lstart_out_of_range:
    mov x0, #STDERR
    adrp x1, range_msg
    add x1, x1, :lo12:range_msg
    mov x2, #RANGE_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_OUT_OF_RANGE
    b .Lstart_exit
.Lstart_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_argv1:
    mov w0, #1
    bl assert_fail
.Lstart_assert_argv2:
    mov w0, #2
    bl assert_fail
.Lstart_assert_error:
    mov w0, #3
    bl assert_fail
.size _start, . - _start

// Parses a side: digits, optionally a point and 1 to 3 more digits.
// In: x0 = a NUL-terminated string.
// Out: x0 = 0 and x1 = the side in thousandths, from 1 to MAX_UNITS; or x0 = ERROR_INVALID
// when the text isn't a number, or ERROR_RANGE when the number is out of range.
// returns: result
.type parse_side, @function
parse_side:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lparse_side_assert_null
    mov x1, #0
    mov x2, #0
    mov x3, #0
    mov x4, #0
    mov x5, #0
    mov x6, #0
    mov x9, #MAX_WHOLE & 0xffff
    movk x9, #MAX_WHOLE >> 16, lsl #16
    mov x10, #10
    mov x7, #MAX_ARG_LEN
.Lparse_side_loop:  // bound: 131072
    cmp x6, #MAX_ARG_LEN
    b.hs .Lparse_side_assert_index
    ldrb w11, [x0, x6]
    cbz w11, .Lparse_side_end
    add x6, x6, #1
    cmp w11, #'.'
    b.eq .Lparse_side_point
    sub w11, w11, #'0'
    cmp w11, #9
    b.hi .Lparse_side_invalid
    cbnz x5, .Lparse_side_fraction
    add x4, x4, #1
    cmp x1, x9
    b.hi .Lparse_side_next
    madd x1, x1, x10, x11
    b .Lparse_side_next
.Lparse_side_fraction:
    add x3, x3, #1
    cmp x3, #MAX_PLACES
    b.hi .Lparse_side_invalid
    madd x2, x2, x10, x11
    b .Lparse_side_next
.Lparse_side_point:
    cbnz x5, .Lparse_side_invalid
    mov x5, #1
.Lparse_side_next:
    subs x7, x7, #1
    b.ne .Lparse_side_loop
    b .Lparse_side_assert_length
.Lparse_side_end:
    mov x0, x1
    mov x1, x2
    mov x2, x3
    mov x3, x4
    mov x4, x5
    bl to_units
    tbnz x0, #63, .Lparse_side_return
    b .Lparse_side_return
.Lparse_side_invalid:
    mov x0, #ERROR_INVALID
.Lparse_side_return:
    ldp x29, x30, [sp], #16
    ret
.Lparse_side_assert_null:
    mov w0, #4
    bl assert_fail
.Lparse_side_assert_index:
    mov w0, #5
    bl assert_fail
.Lparse_side_assert_length:
    mov w0, #6
    bl assert_fail
.size parse_side, . - parse_side

// Prints the hypotenuse of two sides with 3 decimals, then a newline, to standard output:
// each side over 1000 as a double, the square root of the sum of their squares, times 1000,
// rounded to the nearest whole number, ties to even.
// In: x0, x1 = the sides in thousandths, from 1 to MAX_UNITS.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_hypotenuse, @function
print_hypotenuse:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lprint_hypotenuse_assert_side_a
    cbz x1, .Lprint_hypotenuse_assert_side_b
    mov x2, #UNITS
    scvtf d2, x2
    scvtf d0, x0
    fdiv d0, d0, d2
    scvtf d1, x1
    fdiv d1, d1, d2
    fmul d0, d0, d0
    fmul d1, d1, d1
    fadd d0, d0, d1
    fsqrt d0, d0
    fmul d0, d0, d2
    frintn d0, d0
    fcvtzs x0, d0
    bl write_fixed3
    tbnz x0, #63, .Lprint_hypotenuse_return
    mov x0, #0
.Lprint_hypotenuse_return:
    ldp x29, x30, [sp], #16
    ret
.Lprint_hypotenuse_assert_side_a:
    mov w0, #15
    bl assert_fail
.Lprint_hypotenuse_assert_side_b:
    mov w0, #18
    bl assert_fail
.size print_hypotenuse, . - print_hypotenuse

// Checks a parsed side's shape and turns it into thousandths.
// In: x0 = the whole part (above MAX_WHOLE when larger), x1 = the decimals' value,
// x2 = how many decimals, x3 = how many whole digits, x4 = 1 when there was a point.
// Out: x0 = 0 and x1 = the side in thousandths, from 1 to MAX_UNITS; or x0 = ERROR_INVALID
// or ERROR_RANGE.
// returns: result
.type to_units, @function
to_units:
    cmp x2, #MAX_PLACES
    b.hi .Lto_units_assert_places
    cbz x3, .Lto_units_invalid
    cbz x4, .Lto_units_scale
    cbz x2, .Lto_units_invalid
.Lto_units_scale:
    mov x10, #10
    mov x9, #MAX_PLACES
.Lto_units_pad:  // bound: 3
    cmp x2, #MAX_PLACES
    b.hs .Lto_units_padded
    mul x1, x1, x10
    add x2, x2, #1
    subs x9, x9, #1
    b.ne .Lto_units_pad
.Lto_units_padded:
    cmp x1, #UNITS
    b.hs .Lto_units_assert_fraction
    mov x11, #MAX_WHOLE & 0xffff
    movk x11, #MAX_WHOLE >> 16, lsl #16
    cmp x0, x11
    b.hi .Lto_units_range
    mov x12, #UNITS
    madd x0, x0, x12, x1
    cbz x0, .Lto_units_range
    mov x13, #MAX_UNITS & 0xffff
    movk x13, #MAX_UNITS >> 16, lsl #16
    cmp x0, x13
    b.hi .Lto_units_range
    mov x1, x0
    mov x0, #0
    b .Lto_units_return
.Lto_units_range:
    mov x0, #ERROR_RANGE
    b .Lto_units_return
.Lto_units_invalid:
    mov x0, #ERROR_INVALID
.Lto_units_return:
    ret
.Lto_units_assert_places:
    mov w0, #16
    bl assert_fail
.Lto_units_assert_fraction:
    mov w0, #17
    bl assert_fail
.size to_units, . - to_units

// Writes a number of thousandths with 3 decimals, then a newline, to standard output.
// In: x0 = the number, at least 0.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_fixed3, @function
write_fixed3:
    stp x29, x30, [sp, #-16]!
    tbnz x0, #63, .Lwrite_fixed3_assert_negative
    adrp x1, out_buf
    add x1, x1, :lo12:out_buf
    mov w2, #NEWLINE
    strb w2, [x1, #NEWLINE_AT]
    mov x3, #NEWLINE_AT
    mov x4, #10
    mov x5, #MAX_PLACES
.Lwrite_fixed3_fraction:  // bound: 3
    udiv x6, x0, x4
    msub x7, x6, x4, x0
    mov x0, x6
    add w7, w7, #'0'
    sub x3, x3, #1
    cmp x3, #OUT_BUF_LEN
    b.hs .Lwrite_fixed3_assert_fraction_index
    strb w7, [x1, x3]
    subs x5, x5, #1
    b.ne .Lwrite_fixed3_fraction
    sub x3, x3, #1
    cmp x3, #OUT_BUF_LEN
    b.hs .Lwrite_fixed3_assert_point_index
    mov w7, #'.'
    strb w7, [x1, x3]
    mov x5, #MAX_INT_DIGITS
.Lwrite_fixed3_digit:  // bound: 16
    udiv x6, x0, x4
    msub x7, x6, x4, x0
    mov x0, x6
    add w7, w7, #'0'
    sub x3, x3, #1
    cmp x3, #OUT_BUF_LEN
    b.hs .Lwrite_fixed3_assert_digit_index
    strb w7, [x1, x3]
    cbz x0, .Lwrite_fixed3_output
    subs x5, x5, #1
    b.ne .Lwrite_fixed3_digit
    b .Lwrite_fixed3_assert_digits
.Lwrite_fixed3_output:
    add x1, x1, x3
    mov x2, #OUT_BUF_LEN
    sub x2, x2, x3
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lwrite_fixed3_return
    mov x0, #0
.Lwrite_fixed3_return:
    ldp x29, x30, [sp], #16
    ret
.Lwrite_fixed3_assert_negative:
    mov w0, #7
    bl assert_fail
.Lwrite_fixed3_assert_fraction_index:
    mov w0, #8
    bl assert_fail
.Lwrite_fixed3_assert_point_index:
    mov w0, #9
    bl assert_fail
.Lwrite_fixed3_assert_digit_index:
    mov w0, #10
    bl assert_fail
.Lwrite_fixed3_assert_digits:
    mov w0, #11
    bl assert_fail
.size write_fixed3, . - write_fixed3

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
    mov w0, #12
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #13
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #14
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
