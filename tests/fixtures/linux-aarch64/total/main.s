.arch armv8-a

// File descriptors and os_open's mode
.equ STDOUT, 1
.equ STDERR, 2
.equ OPEN_READ, 0
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_OPEN_FAILED, 2
.equ EXIT_BAD_LINE, 3
.equ EXIT_READ_FAILED, 4
.equ EXIT_WRITE_FAILED, 5
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
// Sizes and limits
.equ ARGC_EXPECTED, 2
.equ FILE_MAX, 1048576
.equ SCAN_MAX, FILE_MAX + 1
.equ MAX_LINES, FILE_MAX
.equ MAX_READS, 1024
.equ MAX_WRITES, 64
.equ INT_DIGITS_LIMIT, 6
.equ MAX_CENTS, 9999999
.equ CENT_PLACES, 2
.equ NO_PLACES, 0
.equ NUM_BUF_LEN, 24
.equ MAX_NUM_CHARS, 24
.equ NEWLINE, 10
.equ COMMA, 44
.equ POINT, 46
.equ USAGE_MSG_LEN, 24
.equ OPEN_MSG_LEN, 24
.equ BAD_MSG_LEN, 16

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: total <file.csv>\n"
.size usage_msg, . - usage_msg

.type open_msg, @object
open_msg:
    .ascii "total: cannot open file\n"
.size open_msg, . - open_msg

.type bad_msg, @object
bad_msg:
    .ascii "total: bad line "
.size bad_msg, . - bad_msg

.section .bss

// The whole expenses file.
.type file_buf, @object
file_buf:
    .zero FILE_MAX
.size file_buf, . - file_buf

// Digits, a point and a newline.
.type num_buf, @object
num_buf:
    .zero NUM_BUF_LEN
.size num_buf, . - num_buf

.text
// Opens the file named by the argument, reads it, adds its amounts and prints the total.
.type main, @function
main:
    cmp x0, #ARGC_EXPECTED
    b.ne .Lmain_usage
    ldr x0, [x1, #8]
    cbz x0, .Lmain_assert_argv
    mov x1, #OPEN_READ
    bl os_open
    tbnz x0, #63, .Lmain_open_failed
    mov x19, x0
    adrp x1, file_buf
    add x1, x1, :lo12:file_buf
    mov x2, #FILE_MAX
    bl read_all
    tbnz x0, #63, .Lmain_read_failed
    mov x20, x0
    mov x0, x19
    bl os_close
    tbnz x0, #63, .Lmain_read_failed
    adrp x0, file_buf
    add x0, x0, :lo12:file_buf
    mov x1, x20
    bl sum_lines
    tbnz x0, #63, .Lmain_bad_line
    mov x0, x1
    mov x1, #CENT_PLACES
    mov x2, #STDOUT
    bl write_decimal
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_usage:
    adrp x0, usage_msg
    add x0, x0, :lo12:usage_msg
    mov x1, #USAGE_MSG_LEN
    bl report
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_USAGE
    b .Lmain_exit
.Lmain_open_failed:
    adrp x0, open_msg
    add x0, x0, :lo12:open_msg
    mov x1, #OPEN_MSG_LEN
    bl report
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_OPEN_FAILED
    b .Lmain_exit
.Lmain_bad_line:
    mov x0, x1
    bl report_bad_line
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_BAD_LINE
    b .Lmain_exit
.Lmain_read_failed:
    mov w0, #EXIT_READ_FAILED
    b .Lmain_exit
.Lmain_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_argv:
    mov w0, #1
    bl assert_fail
.size main, . - main

// Prints a message to standard error.
// In: x0 = the message; x1 = its length.
// Out: x0 = 0, or a negative error code.
// returns: result
.type report, @function
report:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lreport_assert_null
    mov x2, x1
    mov x1, x0
    mov x0, #STDERR
    bl write_all
    tbnz x0, #63, .Lreport_return
    mov x0, #0
.Lreport_return:
    ldp x29, x30, [sp], #16
    ret
.Lreport_assert_null:
    mov w0, #26
    bl assert_fail
.size report, . - report

// Prints "total: bad line " and a line number, then a newline, to standard error.
// In: x0 = the line number.
// Out: x0 = 0, or a negative error code.
// returns: result
.type report_bad_line, @function
report_bad_line:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    mov x19, x0
    adrp x0, bad_msg
    add x0, x0, :lo12:bad_msg
    mov x1, #BAD_MSG_LEN
    bl report
    tbnz x0, #63, .Lreport_bad_line_return
    mov x0, x19
    mov x1, #NO_PLACES
    mov x2, #STDERR
    bl write_decimal
    tbnz x0, #63, .Lreport_bad_line_return
    mov x0, #0
.Lreport_bad_line_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.size report_bad_line, . - report_bad_line

// Reads from a file descriptor until the input ends or the buffer is full.
// In: x0 = the file descriptor, x1 = the buffer, x2 = its size, at least 1.
// Out: x0 = the bytes read, or a negative error code.
// returns: result
.type read_all, @function
read_all:
    stp x29, x30, [sp, #-16]!
    cbz x1, .Lread_all_assert_null
    cbz x2, .Lread_all_assert_empty
    mov x9, x0
    mov x11, x2
    mov x12, #0
    mov x13, #MAX_READS
.Lread_all_loop:  // bound: 1024
    sub x2, x11, x12
    mov x0, x9
    bl os_read
    tbnz x0, #63, .Lread_all_return
    cbz x0, .Lread_all_done
    cmp x0, x2
    b.hi .Lread_all_assert_count
    add x1, x1, x0
    add x12, x12, x0
    cmp x12, x11
    b.eq .Lread_all_done
    subs x13, x13, #1
    b.ne .Lread_all_loop
    mov x0, #ERROR_IO
    b .Lread_all_return
.Lread_all_done:
    mov x0, x12
.Lread_all_return:
    ldp x29, x30, [sp], #16
    ret
.Lread_all_assert_null:
    mov w0, #2
    bl assert_fail
.Lread_all_assert_empty:
    mov w0, #3
    bl assert_fail
.Lread_all_assert_count:
    mov w0, #4
    bl assert_fail
.size read_all, . - read_all

// Adds the amounts of every line.
// In: x0 = the file's bytes, x1 = how many, at most FILE_MAX.
// Out: x0 = 0 and x1 = the total in cents, or x0 = ERROR_INVALID and x1 = the bad line's number.
// returns: result
.type sum_lines, @function
sum_lines:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x0, .Lsum_lines_assert_null
    mov x9, #FILE_MAX
    cmp x1, x9
    b.hi .Lsum_lines_assert_len
    mov x19, x0
    mov x20, x1
    mov x21, #0
    mov x22, #0
    mov x23, #MAX_LINES
.Lsum_lines_loop:  // bound: 1048576
    cbz x20, .Lsum_lines_done
    add x22, x22, #1
    mov x0, x19
    mov x1, x20
    bl parse_line
    tbnz x0, #63, .Lsum_lines_bad
    cmp x0, x20
    b.hi .Lsum_lines_assert_consumed
    add x19, x19, x0
    sub x20, x20, x0
    adds x21, x21, x1
    b.vs .Lsum_lines_assert_total
    subs x23, x23, #1
    b.ne .Lsum_lines_loop
    b .Lsum_lines_assert_lines
.Lsum_lines_done:
    mov x0, #0
    mov x1, x21
    b .Lsum_lines_return
.Lsum_lines_bad:
    mov x1, x22
.Lsum_lines_return:
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lsum_lines_assert_null:
    mov w0, #5
    bl assert_fail
.Lsum_lines_assert_len:
    mov w0, #6
    bl assert_fail
.Lsum_lines_assert_consumed:
    mov w0, #7
    bl assert_fail
.Lsum_lines_assert_total:
    mov w0, #8
    bl assert_fail
.Lsum_lines_assert_lines:
    mov w0, #9
    bl assert_fail
.size sum_lines, . - sum_lines

// Reads one line: a date, a comma, an amount, then a newline or the end of the file.
// In: x0 = the line's first byte, x1 = the bytes left in the file, at least 1.
// Out: x0 = the bytes the line takes, newline included, and x1 = its amount in cents,
// or x0 = ERROR_INVALID.
// returns: result
.type parse_line, @function
parse_line:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Lparse_line_assert_null
    mov x19, x0
    mov x20, x1
    bl skip_date
    tbnz x0, #63, .Lparse_line_return
    add x21, x0, #1
    cmp x21, x20
    b.hi .Lparse_line_assert_comma
    add x0, x19, x21
    sub x1, x20, x21
    bl parse_amount
    tbnz x0, #63, .Lparse_line_return
    add x21, x21, x0
    cmp x21, x20
    b.hs .Lparse_line_end
    ldrb w2, [x19, x21]
    cmp w2, #NEWLINE
    b.ne .Lparse_line_invalid
    add x21, x21, #1
.Lparse_line_end:
    cmp x21, x20
    b.hi .Lparse_line_assert_amount
    mov x0, x21
    b .Lparse_line_return
.Lparse_line_invalid:
    mov x0, #ERROR_INVALID
.Lparse_line_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lparse_line_assert_null:
    mov w0, #10
    bl assert_fail
.Lparse_line_assert_comma:
    mov w0, #11
    bl assert_fail
.Lparse_line_assert_amount:
    mov w0, #12
    bl assert_fail
.size parse_line, . - parse_line

// Finds the comma that ends a line's date.
// In: x0 = the line's first byte, x1 = the bytes left in the file, at most FILE_MAX.
// Out: x0 = the comma's index, or ERROR_INVALID when a newline or the end comes first.
// returns: result
.type skip_date, @function
skip_date:
    cbz x0, .Lskip_date_assert_null
    mov x9, #FILE_MAX
    cmp x1, x9
    b.hi .Lskip_date_assert_len
    mov x2, #0
    mov x10, #SCAN_MAX & 0xffff
    movk x10, #SCAN_MAX >> 16, lsl #16
.Lskip_date_loop:  // bound: 1048577
    cmp x2, x1
    b.hs .Lskip_date_invalid
    ldrb w3, [x0, x2]
    cmp w3, #NEWLINE
    b.eq .Lskip_date_invalid
    cmp w3, #COMMA
    b.ne .Lskip_date_next
    mov x0, x2
    b .Lskip_date_return
.Lskip_date_next:
    add x2, x2, #1
    subs x10, x10, #1
    b.ne .Lskip_date_loop
    b .Lskip_date_assert_scan
.Lskip_date_invalid:
    mov x0, #ERROR_INVALID
.Lskip_date_return:
    ret
.Lskip_date_assert_null:
    mov w0, #13
    bl assert_fail
.Lskip_date_assert_len:
    mov w0, #14
    bl assert_fail
.Lskip_date_assert_scan:
    mov w0, #15
    bl assert_fail
.size skip_date, . - skip_date

// Reads an amount of 1 to 5 digits, a point and 2 digits.
// In: x0 = its first byte, x1 = the bytes left in the file, at most FILE_MAX.
// Out: x0 = the bytes it takes and x1 = the amount in cents, or x0 = ERROR_INVALID.
// returns: result
.type parse_amount, @function
parse_amount:
    cbz x0, .Lparse_amount_assert_null
    mov x9, #FILE_MAX
    cmp x1, x9
    b.hi .Lparse_amount_assert_len
    mov x9, #0
    mov x2, #0
    mov x11, #10
    mov x10, #INT_DIGITS_LIMIT
.Lparse_amount_loop:  // bound: 6
    cmp x2, x1
    b.hs .Lparse_amount_invalid
    ldrb w3, [x0, x2]
    cmp w3, #POINT
    b.eq .Lparse_amount_point
    sub w3, w3, #'0'
    cmp w3, #9
    b.hi .Lparse_amount_invalid
    madd x9, x9, x11, x3
    add x2, x2, #1
    subs x10, x10, #1
    b.ne .Lparse_amount_loop
    b .Lparse_amount_invalid
.Lparse_amount_point:
    cbz x2, .Lparse_amount_invalid
    add x2, x2, #1
    cmp x2, x1
    b.hs .Lparse_amount_invalid
    ldrb w3, [x0, x2]
    sub w3, w3, #'0'
    cmp w3, #9
    b.hi .Lparse_amount_invalid
    madd x9, x9, x11, x3
    add x2, x2, #1
    cmp x2, x1
    b.hs .Lparse_amount_invalid
    ldrb w3, [x0, x2]
    sub w3, w3, #'0'
    cmp w3, #9
    b.hi .Lparse_amount_invalid
    madd x9, x9, x11, x3
    add x2, x2, #1
    mov x12, #MAX_CENTS & 0xffff
    movk x12, #MAX_CENTS >> 16, lsl #16
    cmp x9, x12
    b.hi .Lparse_amount_assert_cents
    mov x1, x9
    mov x0, x2
    b .Lparse_amount_return
.Lparse_amount_invalid:
    mov x0, #ERROR_INVALID
.Lparse_amount_return:
    ret
.Lparse_amount_assert_null:
    mov w0, #16
    bl assert_fail
.Lparse_amount_assert_len:
    mov w0, #17
    bl assert_fail
.Lparse_amount_assert_cents:
    mov w0, #18
    bl assert_fail
.size parse_amount, . - parse_amount

// Writes a whole number of units as a decimal with a given number of places, then a newline.
// In: x0 = the number, x1 = the places (0 or 2), x2 = the file descriptor.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_decimal, @function
write_decimal:
    stp x29, x30, [sp, #-16]!
    cmp x1, #CENT_PLACES
    b.hi .Lwrite_decimal_assert_places
    mov x11, x2
    mov x4, x1
    adrp x5, num_buf
    add x5, x5, :lo12:num_buf
    mov x3, #NUM_BUF_LEN - 1
    mov w6, #NEWLINE
    strb w6, [x5, #NUM_BUF_LEN - 1]
    mov x10, #10
    mov x9, #MAX_NUM_CHARS
.Lwrite_decimal_loop:  // bound: 24
    udiv x12, x0, x10
    msub x13, x12, x10, x0
    add w13, w13, #'0'
    sub x3, x3, #1
    cmp x3, #NUM_BUF_LEN
    b.hs .Lwrite_decimal_assert_index
    strb w13, [x5, x3]
    mov x0, x12
    sub x4, x4, #1
    cbnz x4, .Lwrite_decimal_more
    sub x3, x3, #1
    cmp x3, #NUM_BUF_LEN
    b.hs .Lwrite_decimal_assert_point
    mov w6, #POINT
    strb w6, [x5, x3]
.Lwrite_decimal_more:
    cbnz x0, .Lwrite_decimal_next
    tbnz x4, #63, .Lwrite_decimal_output
.Lwrite_decimal_next:
    subs x9, x9, #1
    b.ne .Lwrite_decimal_loop
    b .Lwrite_decimal_assert_chars
.Lwrite_decimal_output:
    add x1, x5, x3
    mov x2, #NUM_BUF_LEN
    sub x2, x2, x3
    mov x0, x11
    bl write_all
    tbnz x0, #63, .Lwrite_decimal_return
    mov x0, #0
.Lwrite_decimal_return:
    ldp x29, x30, [sp], #16
    ret
.Lwrite_decimal_assert_places:
    mov w0, #19
    bl assert_fail
.Lwrite_decimal_assert_index:
    mov w0, #20
    bl assert_fail
.Lwrite_decimal_assert_chars:
    mov w0, #21
    bl assert_fail
.Lwrite_decimal_assert_point:
    mov w0, #25
    bl assert_fail
.size write_decimal, . - write_decimal

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
    mov w0, #22
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #23
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #24
    bl assert_fail
.size write_all, . - write_all

