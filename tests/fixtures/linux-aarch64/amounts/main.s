.arch armv8-a

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
.equ EXIT_NO_COLUMN, 1
.equ EXIT_BAD_AMOUNT, 2
.equ EXIT_MISSING_AMOUNT, 3
.equ EXIT_IO_FAILED, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_NOT_FOUND, -2
.equ ERROR_BAD, -22
.equ ERROR_MISSING, -61
// Sizes and limits
.equ MAX_INPUT, 1048576
.equ IN_BUF_LEN, MAX_INPUT + 1
.equ SCAN_LIMIT, MAX_INPUT + 1
.equ MAX_READS, MAX_INPUT + 1
.equ MAX_LINES, MAX_INPUT
.equ LINES_BASE, MAX_LINES + 2
.equ MAX_WHOLE_DIGITS, 6
.equ WHOLE_SCAN, MAX_WHOLE_DIGITS + 1
.equ FRACTION_DIGITS, 2
.equ MAX_DIGITS, 19
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ AMOUNT_LEN, 6
.equ AMOUNT_HEAD, 0x756f6d61
.equ AMOUNT_TAIL, 0x746e
.equ FIRST_ROW_LINE, 2
.equ NO_COLUMN_MSG_LEN, 24
.equ BAD_MSG_LEN, 26
.equ MISSING_MSG_LEN, 30
.equ OUT_BUF_LEN, 21
.equ CENTS_BUF_LEN, 23
.equ CENTS_POINT, CENTS_BUF_LEN - 4

.section .rodata

.type no_column_msg, @object
no_column_msg:
    .ascii "total: no amount column\n"
.size no_column_msg, . - no_column_msg

.type bad_msg, @object
bad_msg:
    .ascii "total: bad amount on line "
.size bad_msg, . - bad_msg

.type missing_msg, @object
missing_msg:
    .ascii "total: missing amount on line "
.size missing_msg, . - missing_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// All of standard input, plus one byte to notice input that is too long.
.type in_buf, @object
in_buf:
    .zero IN_BUF_LEN
.size in_buf, . - in_buf

// Sign, 19 digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

// Sign, 19 digits, a point and a newline, with a spare byte.
.type cents_buf, @object
cents_buf:
    .zero CENTS_BUF_LEN
.size cents_buf, . - cents_buf

.text
.globl _start

// Reads the CSV, finds the amount column, totals it and prints the total or an error.
.type _start, @function
_start:
    adrp x19, in_buf
    add x19, x19, :lo12:in_buf
    mov x0, x19
    mov x1, #IN_BUF_LEN & 0xffff
    movk x1, #IN_BUF_LEN >> 16, lsl #16
    bl read_input
    tbnz x0, #63, .Lstart_io_failed
    mov x20, x1
    mov x0, x19
    mov x1, x20
    bl find_column
    tbnz x0, #63, .Lstart_no_column
    mov x3, x2
    mov x2, x1
    mov x1, x20
    mov x0, x19
    bl sum_rows
    tbnz x0, #63, .Lstart_row_error
    mov x0, x1
    bl write_cents
    tbnz x0, #63, .Lstart_io_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_no_column:
    mov x0, #STDERR
    adrp x1, no_column_msg
    add x1, x1, :lo12:no_column_msg
    mov x2, #NO_COLUMN_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lstart_io_failed
    mov w0, #EXIT_NO_COLUMN
    b .Lstart_exit
.Lstart_row_error:
    mov x2, x1
    cmp x0, #ERROR_BAD
    b.eq .Lstart_bad
    cmp x0, #ERROR_MISSING
    b.eq .Lstart_missing
    b .Lstart_assert_error
.Lstart_bad:
    adrp x0, bad_msg
    add x0, x0, :lo12:bad_msg
    mov x1, #BAD_MSG_LEN
    bl report_line
    tbnz x0, #63, .Lstart_io_failed
    mov w0, #EXIT_BAD_AMOUNT
    b .Lstart_exit
.Lstart_missing:
    adrp x0, missing_msg
    add x0, x0, :lo12:missing_msg
    mov x1, #MISSING_MSG_LEN
    bl report_line
    tbnz x0, #63, .Lstart_io_failed
    mov w0, #EXIT_MISSING_AMOUNT
    b .Lstart_exit
.Lstart_io_failed:
    mov w0, #EXIT_IO_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_error:
    mov w0, #1
    bl assert_fail
.size _start, . - _start

// Reads all of standard input into a buffer.
// In: x0 = the buffer, x1 = its size, one more than the largest input.
// Out: x0 = 0 and x1 = the bytes read, or a negative error code (also when the input fills
// the buffer).
// returns: result
.type read_input, @function
read_input:
    cbz x0, .Lread_assert_null
    mov x9, #IN_BUF_LEN & 0xffff
    movk x9, #IN_BUF_LEN >> 16, lsl #16
    cmp x1, x9
    b.hi .Lread_assert_size
    mov x10, x1
    mov x11, x0
    mov x12, #0
    mov x13, #MAX_READS & 0xffff
    movk x13, #MAX_READS >> 16, lsl #16
.Lread_loop:  // bound: 1048577
    sub x2, x10, x12
    cbz x2, .Lread_too_long
    add x1, x11, x12
    mov x0, #STDIN
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lread_return
    cbz x0, .Lread_done
    add x12, x12, x0
    cmp x12, x10
    b.hi .Lread_assert_count
    subs x13, x13, #1
    b.ne .Lread_loop
    b .Lread_assert_reads
.Lread_too_long:
    mov x0, #ERROR_IO
    b .Lread_return
.Lread_done:
    mov x1, x12
    mov x0, #0
.Lread_return:
    ret
.Lread_assert_null:
    mov w0, #2
    bl assert_fail
.Lread_assert_size:
    mov w0, #3
    bl assert_fail
.Lread_assert_count:
    mov w0, #4
    bl assert_fail
.Lread_assert_reads:
    mov w0, #5
    bl assert_fail
.size read_input, . - read_input

// Finds the column named "amount" in the header line.
// In: x0 = the input, x1 = its length.
// Out: x0 = 0, x1 = the column's index and x2 = where the first row starts, or
// x0 = ERROR_NOT_FOUND when the header has no amount column.
// returns: result
.type find_column, @function
find_column:
    cbz x0, .Lfind_column_assert_null
    mov x3, #0
    mov x4, #0
    mov x5, #0
    mov x6, #-1
    mov x7, #SCAN_LIMIT & 0xffff
    movk x7, #SCAN_LIMIT >> 16, lsl #16
.Lfind_column_loop:  // bound: 1048577
    cmp x3, x1
    b.hs .Lfind_column_not_found
    ldrb w9, [x0, x3]
    cmp w9, #','
    b.eq .Lfind_column_end_field
    cmp w9, #NEWLINE
    b.ne .Lfind_column_next
.Lfind_column_end_field:
    sub x10, x3, x4
    cmp x10, #AMOUNT_LEN
    b.ne .Lfind_column_after
    cmp x4, x1
    b.hs .Lfind_column_assert_start
    add x11, x0, x4
    ldr w10, [x11]
    mov x12, #AMOUNT_HEAD & 0xffff
    movk x12, #AMOUNT_HEAD >> 16, lsl #16
    cmp w10, w12
    b.ne .Lfind_column_after
    ldrh w10, [x11, #4]
    mov x12, #AMOUNT_TAIL
    cmp w10, w12
    b.ne .Lfind_column_after
    tbz x6, #63, .Lfind_column_after
    mov x6, x5
.Lfind_column_after:
    add x5, x5, #1
    add x4, x3, #1
    cmp w9, #NEWLINE
    b.eq .Lfind_column_header_end
.Lfind_column_next:
    add x3, x3, #1
    subs x7, x7, #1
    b.ne .Lfind_column_loop
    b .Lfind_column_assert_scan
.Lfind_column_header_end:
    tbnz x6, #63, .Lfind_column_not_found
    mov x1, x6
    add x2, x3, #1
    mov x0, #0
    b .Lfind_column_return
.Lfind_column_not_found:
    mov x0, #ERROR_NOT_FOUND
.Lfind_column_return:
    ret
.Lfind_column_assert_null:
    mov w0, #6
    bl assert_fail
.Lfind_column_assert_start:
    mov w0, #7
    bl assert_fail
.Lfind_column_assert_scan:
    mov w0, #8
    bl assert_fail
.size find_column, . - find_column

// Adds up the amount field of every row.
// In: x0 = the input, x1 = its length, x2 = the amount column, x3 = where the rows start.
// Out: x0 = 0 and x1 = the total in cents, or x0 = ERROR_BAD or ERROR_MISSING and
// x1 = the line number of the row at fault.
// returns: result
.type sum_rows, @function
sum_rows:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x0, .Lsum_assert_null
    cmp x3, x1
    b.hi .Lsum_assert_start
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, x3
    mov x23, #0
    mov x24, #MAX_LINES
.Lsum_line:  // bound: 1048576
    cmp x22, x20
    b.hs .Lsum_done
    mov x0, x19
    mov x1, x20
    mov x2, x21
    mov x3, x22
    bl row_amount
    tbnz x0, #63, .Lsum_fault
    adds x23, x23, x1
    b.vs .Lsum_assert_overflow
    mov x22, x2
    subs x24, x24, #1
    b.ne .Lsum_line
    b .Lsum_assert_lines
.Lsum_done:
    mov x0, #0
    mov x1, x23
    b .Lsum_return
.Lsum_fault:
    mov x9, #LINES_BASE & 0xffff
    movk x9, #LINES_BASE >> 16, lsl #16
    sub x1, x9, x24
.Lsum_return:
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lsum_assert_null:
    mov w0, #9
    bl assert_fail
.Lsum_assert_start:
    mov w0, #10
    bl assert_fail
.Lsum_assert_overflow:
    mov w0, #11
    bl assert_fail
.Lsum_assert_lines:
    mov w0, #12
    bl assert_fail
.size sum_rows, . - sum_rows

// Reads one row's amount and finds where the next row starts.
// In: x0 = the input, x1 = its length, x2 = the amount column, x3 = where the row starts.
// Out: x0 = 0, x1 = the amount in cents and x2 = where the next row starts, or
// x0 = ERROR_MISSING or ERROR_BAD.
// returns: result
.type row_amount, @function
row_amount:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Lrow_assert_null
    cmp x3, x1
    b.hs .Lrow_assert_start
    mov x19, x0
    mov x20, x1
    bl find_field
    tbnz x0, #63, .Lrow_return
    mov x2, x1
    mov x1, x20
    mov x0, x19
    bl parse_amount
    tbnz x0, #63, .Lrow_return
    mov x21, x1
    mov x1, x20
    mov x0, x19
    bl skip_line
    mov x2, x0
    mov x1, x21
    mov x0, #0
.Lrow_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lrow_assert_null:
    mov w0, #35
    bl assert_fail
.Lrow_assert_start:
    mov w0, #36
    bl assert_fail
.size row_amount, . - row_amount

// Finds where a row's field starts by skipping the fields before it.
// In: x0 = the input, x1 = its length, x2 = the field's index, x3 = where the row starts.
// Out: x0 = 0 and x1 = where the field starts, or x0 = ERROR_MISSING when the row ends
// first.
// returns: result
.type find_field, @function
find_field:
    cbz x0, .Lfind_field_assert_null
    cmp x3, x1
    b.hi .Lfind_field_assert_start
    mov x4, x2
    cbz x4, .Lfind_field_found
    mov x9, #SCAN_LIMIT & 0xffff
    movk x9, #SCAN_LIMIT >> 16, lsl #16
.Lfind_field_loop:  // bound: 1048577
    cmp x3, x1
    b.hs .Lfind_field_missing
    ldrb w5, [x0, x3]
    add x3, x3, #1
    cmp w5, #NEWLINE
    b.eq .Lfind_field_missing
    cmp w5, #','
    b.ne .Lfind_field_next
    subs x4, x4, #1
    b.eq .Lfind_field_found
.Lfind_field_next:
    subs x9, x9, #1
    b.ne .Lfind_field_loop
    b .Lfind_field_assert_scan
.Lfind_field_found:
    mov x1, x3
    mov x0, #0
    b .Lfind_field_return
.Lfind_field_missing:
    mov x0, #ERROR_MISSING
.Lfind_field_return:
    ret
.Lfind_field_assert_null:
    mov w0, #13
    bl assert_fail
.Lfind_field_assert_start:
    mov w0, #14
    bl assert_fail
.Lfind_field_assert_scan:
    mov w0, #15
    bl assert_fail
.size find_field, . - find_field

// Parses an amount field: an optional minus sign, then the cents.
// In: x0 = the input, x1 = its length, x2 = where the field starts.
// Out: x0 = 0, x1 = the amount in cents and x2 = where the field ends, or
// x0 = ERROR_BAD.
// returns: result
.type parse_amount, @function
parse_amount:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    cbz x0, .Lparse_assert_null
    cmp x2, x1
    b.hi .Lparse_assert_start
    mov x19, #0
    cmp x2, x1
    b.hs .Lparse_unsigned
    ldrb w9, [x0, x2]
    cmp w9, #'-'
    b.ne .Lparse_unsigned
    mov x19, #1
    add x2, x2, #1
.Lparse_unsigned:
    bl parse_cents
    tbnz x0, #63, .Lparse_return
    cbz x19, .Lparse_return
    neg x1, x1
.Lparse_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lparse_assert_null:
    mov w0, #16
    bl assert_fail
.Lparse_assert_start:
    mov w0, #37
    bl assert_fail
.size parse_amount, . - parse_amount

// Parses an amount without its sign: 1 to 6 digits without a leading zero (except a lone 0),
// a point and two digits, ended by a comma, a newline or the input's end.
// In: x0 = the input, x1 = its length, x2 = where the digits start.
// Out: x0 = 0, x1 = the cents and x2 = where the field ends, or x0 = ERROR_BAD.
// returns: result
.type parse_cents, @function
parse_cents:
    cbz x0, .Lcents_parse_assert_null
    mov x3, x2
    mov x4, x2
    mov x5, #0
    mov x10, #10
    mov x6, #WHOLE_SCAN
.Lcents_parse_whole:  // bound: 7
    cmp x3, x1
    b.hs .Lcents_parse_bad
    ldrb w7, [x0, x3]
    cmp w7, #'.'
    b.eq .Lcents_parse_point
    sub w7, w7, #'0'
    cmp w7, #9
    b.hi .Lcents_parse_bad
    madd x5, x5, x10, x7
    add x3, x3, #1
    subs x6, x6, #1
    b.ne .Lcents_parse_whole
    b .Lcents_parse_bad
.Lcents_parse_point:
    subs x9, x3, x4
    b.eq .Lcents_parse_bad
    cmp x9, #1
    b.eq .Lcents_parse_fraction
    cmp x4, x1
    b.hs .Lcents_parse_assert_start
    ldrb w7, [x0, x4]
    cmp w7, #'0'
    b.eq .Lcents_parse_bad
.Lcents_parse_fraction:
    add x3, x3, #1
    mov x6, #FRACTION_DIGITS
.Lcents_parse_fraction_loop:  // bound: 2
    cmp x3, x1
    b.hs .Lcents_parse_bad
    ldrb w7, [x0, x3]
    sub w7, w7, #'0'
    cmp w7, #9
    b.hi .Lcents_parse_bad
    madd x5, x5, x10, x7
    add x3, x3, #1
    subs x6, x6, #1
    b.ne .Lcents_parse_fraction_loop
    cmp x3, x1
    b.hs .Lcents_parse_ok
    ldrb w7, [x0, x3]
    cmp w7, #','
    b.eq .Lcents_parse_ok
    cmp w7, #NEWLINE
    b.ne .Lcents_parse_bad
.Lcents_parse_ok:
    mov x1, x5
    mov x2, x3
    mov x0, #0
    b .Lcents_parse_return
.Lcents_parse_bad:
    mov x0, #ERROR_BAD
.Lcents_parse_return:
    ret
.Lcents_parse_assert_null:
    mov w0, #38
    bl assert_fail
.Lcents_parse_assert_start:
    mov w0, #18
    bl assert_fail
.size parse_cents, . - parse_cents

// Finds where the next line starts.
// In: x0 = the input, x1 = its length, x2 = a position inside a line.
// Out: x0 = the position after the line's newline, or the input's length.
.type skip_line, @function
skip_line:
    cbz x0, .Lskip_assert_null
    cmp x2, x1
    b.hi .Lskip_assert_start
    mov x3, x2
    mov x9, #SCAN_LIMIT & 0xffff
    movk x9, #SCAN_LIMIT >> 16, lsl #16
.Lskip_loop:  // bound: 1048577
    cmp x3, x1
    b.hs .Lskip_end
    ldrb w4, [x0, x3]
    add x3, x3, #1
    cmp w4, #NEWLINE
    b.eq .Lskip_end
    subs x9, x9, #1
    b.ne .Lskip_loop
    b .Lskip_assert_scan
.Lskip_end:
    mov x0, x3
    ret
.Lskip_assert_null:
    mov w0, #19
    bl assert_fail
.Lskip_assert_start:
    mov w0, #20
    bl assert_fail
.Lskip_assert_scan:
    mov w0, #21
    bl assert_fail
.size skip_line, . - skip_line

// Prints an amount in cents with two decimals, then a newline, to standard output.
// In: x0 = the cents, never -9223372036854775808.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_cents, @function
write_cents:
    stp x29, x30, [sp, #-16]!
    mov x9, x0
    cmp x0, #0
    b.ge .Lcents_digits
    negs x0, x0
    b.vs .Lcents_assert_min
.Lcents_digits:
    adrp x5, cents_buf
    add x5, x5, :lo12:cents_buf
    mov w6, #NEWLINE
    strb w6, [x5, #CENTS_BUF_LEN - 1]
    mov x3, #CENTS_BUF_LEN - 1
    mov x10, #10
    mov x4, #MAX_DIGITS
.Lcents_digit:  // bound: 19
    udiv x12, x0, x10
    msub x13, x12, x10, x0
    add w13, w13, #'0'
    sub x3, x3, #1
    cmp x3, #CENTS_BUF_LEN
    b.hs .Lcents_assert_index
    strb w13, [x5, x3]
    mov x0, x12
    cmp x3, #CENTS_POINT + 1
    b.ne .Lcents_more
    mov w6, #'.'
    strb w6, [x5, #CENTS_POINT]
    sub x3, x3, #1
.Lcents_more:
    cbnz x0, .Lcents_next
    cmp x3, #CENTS_POINT
    b.lo .Lcents_sign
.Lcents_next:
    subs x4, x4, #1
    b.ne .Lcents_digit
    b .Lcents_assert_digits
.Lcents_sign:
    tbz x9, #63, .Lcents_output
    sub x3, x3, #1
    cmp x3, #CENTS_BUF_LEN
    b.hs .Lcents_assert_sign_index
    mov w6, #'-'
    strb w6, [x5, x3]
.Lcents_output:
    add x1, x5, x3
    mov x2, #CENTS_BUF_LEN
    sub x2, x2, x3
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lcents_return
    mov x0, #0
.Lcents_return:
    ldp x29, x30, [sp], #16
    ret
.Lcents_assert_min:
    mov w0, #22
    bl assert_fail
.Lcents_assert_index:
    mov w0, #23
    bl assert_fail
.Lcents_assert_digits:
    mov w0, #24
    bl assert_fail
.Lcents_assert_sign_index:
    mov w0, #34
    bl assert_fail
.size write_cents, . - write_cents

// Prints a message, then a line number and a newline, to standard error.
// In: x0 = the message, x1 = its length, x2 = the line number, at least 2.
// Out: x0 = 0, or a negative error code.
// returns: result
.type report_line, @function
report_line:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    cbz x0, .Lreport_assert_null
    cmp x2, #FIRST_ROW_LINE
    b.lo .Lreport_assert_line
    mov x19, x2
    mov x2, x1
    mov x1, x0
    mov x0, #STDERR
    bl write_all
    tbnz x0, #63, .Lreport_return
    mov x0, x19
    mov x1, #STDERR
    bl write_i64
    tbnz x0, #63, .Lreport_return
    mov x0, #0
.Lreport_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lreport_assert_null:
    mov w0, #25
    bl assert_fail
.Lreport_assert_line:
    mov w0, #26
    bl assert_fail
.size report_line, . - report_line

// Writes a whole number in decimal, then a newline.
// In: x0 = the number, never -9223372036854775808, x1 = the file descriptor.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_i64, @function
write_i64:
    stp x29, x30, [sp, #-16]!
    mov x11, x1
    mov x9, x0
    cmp x0, #0
    b.ge .Lwrite_i64_digits
    negs x0, x0
    b.vs .Lwrite_i64_assert_min
.Lwrite_i64_digits:
    adrp x5, out_buf
    add x5, x5, :lo12:out_buf
    mov w6, #NEWLINE
    strb w6, [x5, #OUT_BUF_LEN - 1]
    mov x3, #OUT_BUF_LEN - 1
    mov x10, #10
    mov x4, #MAX_DIGITS
.Lwrite_i64_digit:  // bound: 19
    udiv x12, x0, x10
    msub x13, x12, x10, x0
    add w13, w13, #'0'
    sub x3, x3, #1
    cmp x3, #OUT_BUF_LEN
    b.hs .Lwrite_i64_assert_digit_index
    strb w13, [x5, x3]
    mov x0, x12
    cbz x0, .Lwrite_i64_sign
    subs x4, x4, #1
    b.ne .Lwrite_i64_digit
    b .Lwrite_i64_assert_digits
.Lwrite_i64_sign:
    tbz x9, #63, .Lwrite_i64_output
    sub x3, x3, #1
    cmp x3, #OUT_BUF_LEN
    b.hs .Lwrite_i64_assert_sign_index
    mov w6, #'-'
    strb w6, [x5, x3]
.Lwrite_i64_output:
    add x1, x5, x3
    mov x2, #OUT_BUF_LEN
    sub x2, x2, x3
    mov x0, x11
    bl write_all
    tbnz x0, #63, .Lwrite_i64_return
    mov x0, #0
.Lwrite_i64_return:
    ldp x29, x30, [sp], #16
    ret
.Lwrite_i64_assert_min:
    mov w0, #27
    bl assert_fail
.Lwrite_i64_assert_digit_index:
    mov w0, #28
    bl assert_fail
.Lwrite_i64_assert_digits:
    mov w0, #29
    bl assert_fail
.Lwrite_i64_assert_sign_index:
    mov w0, #30
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

