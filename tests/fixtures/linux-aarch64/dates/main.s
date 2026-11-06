.arch armv8-a

// File descriptors and clocks
.equ STDOUT, 1
.equ STDERR, 2
.equ CLOCK_REALTIME, 0
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_INVALID_DATE, 1
.equ EXIT_WRITE_FAILED, 2
.equ EXIT_USAGE, 3
.equ EXIT_CLOCK_FAILED, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
// Argument counts, including the program's path
.equ ARGC_TIME, 1
.equ ARGC_DATE, 2
// Calendar
.equ SECONDS_PER_DAY, 86400
.equ SECONDS_PER_HOUR, 3600
.equ SECONDS_PER_MINUTE, 60
.equ HOURS_PER_DAY, 24
.equ NS_PER_SECOND, 1000000000
.equ MIN_YEAR, 1970
.equ MAX_YEAR, 9999
.equ MONTHS, 12
.equ FEBRUARY, 2
.equ MIN_MONTH_DAYS, 28
.equ MAX_MONTH_DAYS, 31
// Bit n set for each month n with 30 days: April, June, September, November.
.equ SHORT_MONTHS, 0xA50
.equ DAYS_PER_ERA, 146097
.equ YEARS_PER_ERA, 400
.equ DAYS_PER_YEAR, 365
.equ YEARS_PER_CENTURY, 100
.equ MARCH_SHIFT, 9
.equ MONTH_DAYS_FACTOR, 153
.equ MONTH_DAYS_ROUND, 2
.equ MONTH_DAYS_DIVISOR, 5
.equ EPOCH_DAYS, 719468
// Sizes and limits
.equ DATE_LEN, 10
.equ MAX_WRITES, 64
.equ MAX_DIGITS, 19
.equ NEWLINE, 10
.equ COLON, 58
.equ DASH, 45
.equ DECIMAL_BASE, 10
.equ USAGE_MSG_LEN, 26
.equ INVALID_MSG_LEN, 34
.equ OUT_BUF_LEN, 21
.equ TIME_BUF_LEN, 9
.equ TIMESPEC_LEN, 16

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: clock [YYYY-MM-DD]\n"
.size usage_msg, . - usage_msg

.type invalid_msg, @object
invalid_msg:
    .ascii "invalid date: expected YYYY-MM-DD\n"
.size invalid_msg, . - invalid_msg

// What each of a date's ten characters must be: a digit (d) or a dash.
.type date_pattern, @object
date_pattern:
    .ascii "dddd-dd-dd"
.size date_pattern, . - date_pattern

.section .bss

// Sign, 19 digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

// "hh:mm:ss" and a newline.
.type time_buf, @object
time_buf:
    .zero TIME_BUF_LEN
.size time_buf, . - time_buf

// The clock as clock_gettime gives it: seconds, then nanoseconds.
.type timespec, @object
timespec:
    .zero TIMESPEC_LEN
.size timespec, . - timespec

.text
// With no arguments prints the time of day; with a date prints the days until it.
.type main, @function
main:
    cmp x0, #ARGC_DATE
    b.eq .Lmain_date
    cmp x0, #ARGC_TIME
    b.ne .Lmain_usage
    bl read_clock
    tbnz x0, #63, .Lmain_clock_failed
    mov x0, x1
    bl print_time
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_date:
    ldr x0, [x1, #8]
    cbz x0, .Lmain_assert_argv
    bl parse_date
    tbnz x0, #63, .Lmain_invalid
    mov x19, x1
    bl read_day
    tbnz x0, #63, .Lmain_clock_failed
    subs x0, x19, x1
    b.vs .Lmain_assert_days
    bl write_i64
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_invalid:
    mov x0, #STDERR
    adrp x1, invalid_msg
    add x1, x1, :lo12:invalid_msg
    mov x2, #INVALID_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_INVALID_DATE
    b .Lmain_exit
.Lmain_usage:
    mov x0, #STDERR
    adrp x1, usage_msg
    add x1, x1, :lo12:usage_msg
    mov x2, #USAGE_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_USAGE
    b .Lmain_exit
.Lmain_clock_failed:
    mov w0, #EXIT_CLOCK_FAILED
    b .Lmain_exit
.Lmain_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_argv:
    mov w0, #1
    bl assert_fail
.Lmain_assert_days:
    mov w0, #2
    bl assert_fail
.size main, . - main

// Reads today's date.
// Out: x0 = 0 and x1 = whole days since 1970-01-01, or a negative error code when the clock
// can't be read.
// returns: result
.type read_day, @function
read_day:
    stp x29, x30, [sp, #-16]!
    bl read_clock
    tbnz x0, #63, .Lread_day_return
    tbnz x1, #63, .Lread_day_assert_negative
    mov x9, #SECONDS_PER_DAY & 0xffff
    movk x9, #SECONDS_PER_DAY >> 16, lsl #16
    udiv x10, x1, x9
    msub x11, x10, x9, x1
    cmp x11, x9
    b.hs .Lread_day_assert_remainder
    mov x1, x10
    mov x0, #0
.Lread_day_return:
    ldp x29, x30, [sp], #16
    ret
.Lread_day_assert_negative:
    mov w0, #21
    bl assert_fail
.Lread_day_assert_remainder:
    mov w0, #22
    bl assert_fail
.size read_day, . - read_day

// Reads the time of day.
// Out: x0 = 0 and x1 = whole seconds since 1970, or a negative error code when the clock
// can't be read or reads before 1970.
// returns: result
.type read_clock, @function
read_clock:
    stp x29, x30, [sp, #-16]!
    mov x0, #CLOCK_REALTIME
    adrp x1, timespec
    add x1, x1, :lo12:timespec
    bl os_clock
    tbnz x0, #63, .Lread_clock_return
    adrp x9, timespec
    add x9, x9, :lo12:timespec
    ldr x10, [x9, #8]
    mov x11, #NS_PER_SECOND & 0xffff
    movk x11, #NS_PER_SECOND >> 16, lsl #16
    cmp x10, x11
    b.hs .Lread_clock_assert_ns
    ldr x1, [x9]
    tbnz x1, #63, .Lread_clock_invalid
    mov x0, #0
    b .Lread_clock_return
.Lread_clock_invalid:
    mov x0, #ERROR_INVALID
.Lread_clock_return:
    ldp x29, x30, [sp], #16
    ret
.Lread_clock_assert_ns:
    mov w0, #3
    bl assert_fail
.size read_clock, . - read_clock

// Prints the time of day of a moment as hh:mm:ss and a newline to standard output.
// In: x0 = whole seconds since 1970, 0 or more.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_time, @function
print_time:
    stp x29, x30, [sp, #-16]!
    tbnz x0, #63, .Lprint_time_assert_negative
    mov x9, #SECONDS_PER_DAY & 0xffff
    movk x9, #SECONDS_PER_DAY >> 16, lsl #16
    udiv x10, x0, x9
    msub x11, x10, x9, x0
    mov x9, #SECONDS_PER_HOUR
    udiv x12, x11, x9
    msub x13, x12, x9, x11
    cmp x12, #HOURS_PER_DAY
    b.hs .Lprint_time_assert_hours
    adrp x1, time_buf
    add x1, x1, :lo12:time_buf
    mov x9, #DECIMAL_BASE
    udiv x14, x12, x9
    msub x15, x14, x9, x12
    add w14, w14, #'0'
    strb w14, [x1]
    add w15, w15, #'0'
    strb w15, [x1, #1]
    mov w14, #COLON
    strb w14, [x1, #2]
    mov x9, #SECONDS_PER_MINUTE
    udiv x12, x13, x9
    msub x13, x12, x9, x13
    mov x9, #DECIMAL_BASE
    udiv x14, x12, x9
    msub x15, x14, x9, x12
    add w14, w14, #'0'
    strb w14, [x1, #3]
    add w15, w15, #'0'
    strb w15, [x1, #4]
    mov w14, #COLON
    strb w14, [x1, #5]
    udiv x14, x13, x9
    msub x15, x14, x9, x13
    add w14, w14, #'0'
    strb w14, [x1, #6]
    add w15, w15, #'0'
    strb w15, [x1, #7]
    mov w14, #NEWLINE
    strb w14, [x1, #8]
    mov x0, #STDOUT
    mov x2, #TIME_BUF_LEN
    bl write_all
    tbnz x0, #63, .Lprint_time_return
    mov x0, #0
.Lprint_time_return:
    ldp x29, x30, [sp], #16
    ret
.Lprint_time_assert_negative:
    mov w0, #4
    bl assert_fail
.Lprint_time_assert_hours:
    mov w0, #5
    bl assert_fail
.size print_time, . - print_time

// Parses a date written YYYY-MM-DD and turns it into days since 1970-01-01.
// In: x0 = a NUL-terminated string.
// Out: x0 = 0 and x1 = the day's number, or x0 = ERROR_INVALID when the text isn't a date
// from 1970-01-01 to 9999-12-31 on the calendar.
// returns: result
.type parse_date, @function
parse_date:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lparse_date_assert_null
    mov x9, x0
    mov x10, #0
    mov x11, #0
    mov x12, #0
    mov x13, #0
    adrp x14, date_pattern
    add x14, x14, :lo12:date_pattern
    mov x15, #DATE_LEN
.Lparse_date_loop:  // bound: 10
    cmp x11, #DATE_LEN
    b.hs .Lparse_date_assert_index
    ldrb w16, [x9, x11]
    ldrb w17, [x14, x11]
    cmp w17, #DASH
    b.ne .Lparse_date_digit
    cmp w16, #DASH
    b.ne .Lparse_date_invalid
    mov x13, x12
    mov x12, x10
    mov x10, #0
    b .Lparse_date_next
.Lparse_date_digit:
    sub w16, w16, #'0'
    cmp w16, #9
    b.hi .Lparse_date_invalid
    mov x17, #DECIMAL_BASE
    madd x10, x10, x17, x16
.Lparse_date_next:
    add x11, x11, #1
    subs x15, x15, #1
    b.ne .Lparse_date_loop
    ldrb w16, [x9, #DATE_LEN]
    cbnz w16, .Lparse_date_invalid
    mov x2, x10
    mov x0, x13
    mov x1, x12
    bl civil_days
    tbnz x0, #63, .Lparse_date_return
    mov x0, #0
    b .Lparse_date_return
.Lparse_date_invalid:
    mov x0, #ERROR_INVALID
.Lparse_date_return:
    ldp x29, x30, [sp], #16
    ret
.Lparse_date_assert_null:
    mov w0, #6
    bl assert_fail
.Lparse_date_assert_index:
    mov w0, #7
    bl assert_fail
.size parse_date, . - parse_date

// Turns a year, month and day into days since 1970-01-01, after checking the date exists.
// In: x0 = the year, x1 = the month, x2 = the day, each 0 to 9999.
// Out: x0 = 0 and x1 = the day's number, or x0 = ERROR_INVALID when the date isn't from
// 1970-01-01 to 9999-12-31 on the calendar.
// returns: result
.type civil_days, @function
civil_days:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    mov x19, x0
    mov x20, x1
    mov x21, x2
    cmp x19, #MIN_YEAR
    b.lo .Lcivil_days_invalid
    mov x9, #MAX_YEAR
    cmp x19, x9
    b.hi .Lcivil_days_invalid
    sub x9, x20, #1
    cmp x9, #MONTHS - 1
    b.hi .Lcivil_days_invalid
    cbz x21, .Lcivil_days_invalid
    bl days_in_month
    cmp x0, #MIN_MONTH_DAYS
    b.lo .Lcivil_days_assert_month_days
    cmp x21, x0
    b.hi .Lcivil_days_invalid
    mov x0, x19
    cmp x20, #FEBRUARY
    b.hi .Lcivil_days_march
    sub x0, x0, #1
.Lcivil_days_march:
    bl year_days
    mov x22, x0
    add x9, x20, #MARCH_SHIFT
    mov x10, #MONTHS
    udiv x11, x9, x10
    msub x12, x11, x10, x9
    mov x10, #MONTH_DAYS_FACTOR
    mul x9, x12, x10
    add x9, x9, #MONTH_DAYS_ROUND
    mov x10, #MONTH_DAYS_DIVISOR
    udiv x9, x9, x10
    add x9, x9, x21
    sub x9, x9, #1
    add x22, x22, x9
    mov x10, #EPOCH_DAYS & 0xffff
    movk x10, #EPOCH_DAYS >> 16, lsl #16
    cmp x22, x10
    b.lo .Lcivil_days_assert_epoch
    sub x1, x22, x10
    mov x0, #0
    b .Lcivil_days_return
.Lcivil_days_invalid:
    mov x0, #ERROR_INVALID
.Lcivil_days_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lcivil_days_assert_month_days:
    mov w0, #8
    bl assert_fail
.Lcivil_days_assert_epoch:
    mov w0, #9
    bl assert_fail
.size civil_days, . - civil_days

// Counts the days from 0000-03-01 to March 1 of a year, in the calendar that starts years in March.
// In: x0 = the year, 0 or more.
// Out: x0 = the days.
.type year_days, @function
year_days:
    tbnz x0, #63, .Lyear_days_assert_negative
    mov x9, #YEARS_PER_ERA
    udiv x10, x0, x9
    msub x11, x10, x9, x0
    cmp x11, x9
    b.hs .Lyear_days_assert_era
    mov x9, #DAYS_PER_ERA & 0xffff
    movk x9, #DAYS_PER_ERA >> 16, lsl #16
    mul x12, x10, x9
    mov x9, #DAYS_PER_YEAR
    mul x13, x11, x9
    add x13, x13, x11, lsr #2
    mov x9, #YEARS_PER_CENTURY
    udiv x14, x11, x9
    sub x13, x13, x14
    add x0, x12, x13
    ret
.Lyear_days_assert_negative:
    mov w0, #19
    bl assert_fail
.Lyear_days_assert_era:
    mov w0, #20
    bl assert_fail
.size year_days, . - year_days

// Gives the number of days in a month.
// In: x0 = the year, 1 or more; x1 = the month, 1 to 12.
// Out: x0 = 28 to 31.
.type days_in_month, @function
days_in_month:
    cbz x0, .Ldays_in_month_assert_year
    sub x9, x1, #1
    cmp x9, #MONTHS - 1
    b.hi .Ldays_in_month_assert_month
    cmp x1, #FEBRUARY
    b.eq .Ldays_in_month_february
    mov x9, #SHORT_MONTHS
    lsr x9, x9, x1
    mov x10, #MAX_MONTH_DAYS
    and x9, x9, #1
    sub x0, x10, x9
    b .Ldays_in_month_return
.Ldays_in_month_february:
    tst x0, #3
    b.ne .Ldays_in_month_28
    mov x9, #YEARS_PER_CENTURY
    udiv x10, x0, x9
    msub x11, x10, x9, x0
    cbnz x11, .Ldays_in_month_29
    mov x9, #YEARS_PER_ERA
    udiv x10, x0, x9
    msub x11, x10, x9, x0
    cbnz x11, .Ldays_in_month_28
.Ldays_in_month_29:
    mov x0, #MIN_MONTH_DAYS + 1
    b .Ldays_in_month_return
.Ldays_in_month_28:
    mov x0, #MIN_MONTH_DAYS
.Ldays_in_month_return:
    ret
.Ldays_in_month_assert_year:
    mov w0, #10
    bl assert_fail
.Ldays_in_month_assert_month:
    mov w0, #11
    bl assert_fail
.size days_in_month, . - days_in_month

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
    mov w0, #12
    bl assert_fail
.Lwrite_i64_assert_digit_index:
    mov w0, #13
    bl assert_fail
.Lwrite_i64_assert_digits:
    mov w0, #14
    bl assert_fail
.Lwrite_i64_assert_sign_index:
    mov w0, #15
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
    mov w0, #16
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #17
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #18
    bl assert_fail
.size write_all, . - write_all

