.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_WRITE, 64
.equ SYS_EXIT, 93
.equ SYS_CLOCK_GETTIME, 113
// Clocks and file descriptors
.equ CLOCK_REALTIME, 0
.equ STDOUT, 1
.equ STDERR, 2
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_NO_DATE, 2
.equ EXIT_WRITE_FAILED, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
.equ ERROR_NO_DATE, -34
// Argument counts, including the program's path
.equ ARGC_TIME, 1
.equ ARGC_DATE, 2
// Time
.equ MAX_CLOCK, 253402300799
.equ TIMESPEC_SIZE, 16
.equ SECONDS_PER_DAY, 86400
.equ SECONDS_PER_HOUR, 3600
.equ SECONDS_PER_MINUTE, 60
.equ HOURS_PER_DAY, 24
// Dates
.equ DATE_LEN, 10
.equ YEAR_DIGITS, 4
.equ FIELD_DIGITS, 2
.equ MAX_FIELD_DIGITS, 4
.equ MONTH_OFFSET, 5
.equ DAY_OFFSET, 8
.equ MIN_YEAR, 1970
.equ MAX_YEAR, 9999
.equ MONTHS, 12
.equ FEBRUARY, 2
.equ LEAP_FEBRUARY_DAYS, 29
.equ MAX_MONTH_DAYS, 31
.equ DAYS_PER_ERA, 146097
.equ YEARS_PER_ERA, 400
.equ EPOCH_SHIFT, 719468
// Output
.equ MAX_DIGITS, 19
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ USAGE_MSG_LEN, 26
.equ NO_DATE_MSG_LEN, 21
.equ NEWLINE_MSG_LEN, 1
.equ DATE_PATTERN_LEN, 10
.equ MONTH_DAYS_LEN, 12
.equ TIME_BUF_LEN, 9
.equ OUT_BUF_LEN, 21

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: clock [YYYY-MM-DD]\n"
.size usage_msg, . - usage_msg

.type no_date_msg, @object
no_date_msg:
    .ascii "clock: no such date: "
.size no_date_msg, . - no_date_msg

.type newline_msg, @object
newline_msg:
    .ascii "\n"
.size newline_msg, . - newline_msg

// A d stands for a digit, a dash for itself.
.type date_pattern, @object
date_pattern:
    .ascii "dddd-dd-dd"
.size date_pattern, . - date_pattern

// Days in each month of a year that is not a leap year.
.type month_days, @object
month_days:
    .byte 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31
.size month_days, . - month_days

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .data

// hh:mm:ss and a newline; print_time fills in the digits.
.type time_buf, @object
time_buf:
    .ascii "00:00:00\n"
.size time_buf, . - time_buf

.section .bss

// The clock as clock_gettime gives it: seconds, then nanoseconds.
.type timespec, @object
timespec:
    .zero TIMESPEC_SIZE
.size timespec, . - timespec

// Sign, 19 digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

// Prints the time with no argument, the days until a date with one, and usage otherwise.
.type _start, @function
_start:
    ldr x9, [sp]
    cmp x9, #ARGC_TIME
    b.eq .Lstart_time
    cmp x9, #ARGC_DATE
    b.ne .Lstart_usage
    ldr x19, [sp, #16]
    mov x0, x19
    bl days_until
    tbnz x0, #63, .Lstart_error
    mov x0, x1
    bl write_i64
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_error:
    cmp x0, #ERROR_INVALID
    b.eq .Lstart_usage
    cmp x0, #ERROR_NO_DATE
    b.eq .Lstart_no_date
    b .Lstart_assert_error
.Lstart_time:
    bl print_time
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_no_date:
    mov x0, #STDERR
    adrp x1, no_date_msg
    add x1, x1, :lo12:no_date_msg
    mov x2, #NO_DATE_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lstart_write_failed
    mov x0, #STDERR
    mov x1, x19
    mov x2, #DATE_LEN
    bl write_all
    tbnz x0, #63, .Lstart_write_failed
    mov x0, #STDERR
    adrp x1, newline_msg
    add x1, x1, :lo12:newline_msg
    mov x2, #NEWLINE_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_NO_DATE
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
.Lstart_assert_error:
    mov w0, #1
    bl assert_fail
.size _start, . - _start

// Reads the time of day.
// Out: x0 = seconds since 1970, from 0 to MAX_CLOCK.
.type read_clock, @function
read_clock:
    mov x0, #CLOCK_REALTIME
    adrp x1, timespec
    add x1, x1, :lo12:timespec
    mov x8, #SYS_CLOCK_GETTIME
    svc #0
    tbnz x0, #63, .Lread_clock_assert_failed
    ldr x0, [x1]
    mov x9, #MAX_CLOCK & 0xffff
    movk x9, #(MAX_CLOCK >> 16) & 0xffff, lsl #16
    movk x9, #MAX_CLOCK >> 32, lsl #32
    cmp x0, x9
    b.hi .Lread_clock_assert_range
    ret
.Lread_clock_assert_failed:
    mov w0, #2
    bl assert_fail
.Lread_clock_assert_range:
    mov w0, #3
    bl assert_fail
.size read_clock, . - read_clock

// Reads the clock and prints the time of day in UTC as hh:mm:ss and a newline to standard
// output.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_time, @function
print_time:
    stp x29, x30, [sp, #-16]!
    bl read_clock
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
    mov x9, #10
    udiv x14, x12, x9
    msub x15, x14, x9, x12
    add w14, w14, #'0'
    strb w14, [x1]
    add w15, w15, #'0'
    strb w15, [x1, #1]
    mov x9, #SECONDS_PER_MINUTE
    udiv x12, x13, x9
    msub x13, x12, x9, x13
    cmp x12, #SECONDS_PER_MINUTE
    b.hs .Lprint_time_assert_minutes
    mov x9, #10
    udiv x14, x12, x9
    msub x15, x14, x9, x12
    add w14, w14, #'0'
    strb w14, [x1, #3]
    add w15, w15, #'0'
    strb w15, [x1, #4]
    udiv x14, x13, x9
    msub x15, x14, x9, x13
    add w14, w14, #'0'
    strb w14, [x1, #6]
    add w15, w15, #'0'
    strb w15, [x1, #7]
    mov x0, #STDOUT
    mov x2, #TIME_BUF_LEN
    bl write_all
    tbnz x0, #63, .Lprint_time_return
    mov x0, #0
.Lprint_time_return:
    ldp x29, x30, [sp], #16
    ret
.Lprint_time_assert_hours:
    mov w0, #4
    bl assert_fail
.Lprint_time_assert_minutes:
    mov w0, #5
    bl assert_fail
.size print_time, . - print_time

// Counts the days from today's UTC date to a date written as YYYY-MM-DD.
// In: x0 = a NUL-terminated string.
// Out: x0 = 0 and x1 = the days (negative for a past date), or x0 = ERROR_INVALID when the
// text isn't in the date's shape, or ERROR_NO_DATE when it names no real day.
// returns: result
.type days_until, @function
days_until:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Ldays_until_assert_null
    mov x19, x0
    bl check_shape
    tbnz x0, #63, .Ldays_until_return
    mov x0, x19
    mov x1, #YEAR_DIGITS
    bl digits_value
    mov x20, x0
    add x0, x19, #MONTH_OFFSET
    mov x1, #FIELD_DIGITS
    bl digits_value
    mov x21, x0
    add x0, x19, #DAY_OFFSET
    mov x1, #FIELD_DIGITS
    bl digits_value
    mov x2, x0
    mov x0, x20
    mov x1, x21
    bl days_from_date
    tbnz x0, #63, .Ldays_until_return
    mov x20, x1
    bl read_clock
    mov x9, #SECONDS_PER_DAY & 0xffff
    movk x9, #SECONDS_PER_DAY >> 16, lsl #16
    udiv x10, x0, x9
    subs x1, x20, x10
    b.vs .Ldays_until_assert_overflow
    mov x0, #0
.Ldays_until_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Ldays_until_assert_null:
    mov w0, #6
    bl assert_fail
.Ldays_until_assert_overflow:
    mov w0, #7
    bl assert_fail
.size days_until, . - days_until

// Checks that a string is four digits, a dash, two digits, a dash and two digits, and no more.
// In: x0 = a NUL-terminated string.
// Out: x0 = 0, or ERROR_INVALID.
// returns: result
.type check_shape, @function
check_shape:
    cbz x0, .Lcheck_shape_assert_null
    adrp x1, date_pattern
    add x1, x1, :lo12:date_pattern
    mov x2, #0
    mov x3, #DATE_LEN
.Lcheck_shape_loop:  // bound: 10
    cmp x2, #DATE_PATTERN_LEN
    b.hs .Lcheck_shape_assert_index
    ldrb w4, [x0, x2]
    ldrb w5, [x1, x2]
    cmp w5, #'-'
    b.eq .Lcheck_shape_dash
    cmp w5, #'d'
    b.ne .Lcheck_shape_assert_pattern
    sub w4, w4, #'0'
    cmp w4, #9
    b.hi .Lcheck_shape_invalid
    b .Lcheck_shape_next
.Lcheck_shape_dash:
    cmp w4, #'-'
    b.ne .Lcheck_shape_invalid
.Lcheck_shape_next:
    add x2, x2, #1
    subs x3, x3, #1
    b.ne .Lcheck_shape_loop
    ldrb w4, [x0, #DATE_LEN]
    cbnz w4, .Lcheck_shape_invalid
    mov x0, #0
    b .Lcheck_shape_return
.Lcheck_shape_invalid:
    mov x0, #ERROR_INVALID
.Lcheck_shape_return:
    ret
.Lcheck_shape_assert_null:
    mov w0, #8
    bl assert_fail
.Lcheck_shape_assert_index:
    mov w0, #9
    bl assert_fail
.Lcheck_shape_assert_pattern:
    mov w0, #10
    bl assert_fail
.size check_shape, . - check_shape

// Reads the value of a run of decimal digits that check_shape has already checked.
// In: x0 = the first digit, x1 = how many digits, 1 to MAX_FIELD_DIGITS.
// Out: x0 = their value.
.type digits_value, @function
digits_value:
    cbz x0, .Ldigits_value_assert_null
    cmp x1, #MAX_FIELD_DIGITS
    b.hi .Ldigits_value_assert_count
    mov x9, x0
    mov x0, #0
    mov x10, #0
    mov x11, #MAX_FIELD_DIGITS
.Ldigits_value_loop:  // bound: 4
    cmp x10, x1
    b.hs .Ldigits_value_return
    cmp x10, #MAX_FIELD_DIGITS
    b.hs .Ldigits_value_assert_index
    ldrb w12, [x9, x10]
    sub w12, w12, #'0'
    cmp w12, #9
    b.hi .Ldigits_value_assert_digit
    mov x13, #10
    madd x0, x0, x13, x12
    add x10, x10, #1
    subs x11, x11, #1
    b.ne .Ldigits_value_loop
.Ldigits_value_return:
    ret
.Ldigits_value_assert_null:
    mov w0, #11
    bl assert_fail
.Ldigits_value_assert_count:
    mov w0, #12
    bl assert_fail
.Ldigits_value_assert_index:
    mov w0, #13
    bl assert_fail
.Ldigits_value_assert_digit:
    mov w0, #14
    bl assert_fail
.size digits_value, . - digits_value

// Checks that a year, month and day name a real day from 1970 to 9999, then counts its days.
// In: x0 = the year, x1 = the month, x2 = the day, each from 0 to 9999.
// Out: x0 = 0 and x1 = days since 1970-01-01, or x0 = ERROR_NO_DATE.
// returns: result
.type days_from_date, @function
days_from_date:
    stp x29, x30, [sp, #-16]!
    mov x9, #MAX_YEAR
    cmp x0, x9
    b.hi .Ldays_from_date_assert_input
    mov x10, x2
    cmp x0, #MIN_YEAR
    b.lo .Ldays_from_date_invalid
    cbz x1, .Ldays_from_date_invalid
    cmp x1, #MONTHS
    b.hi .Ldays_from_date_invalid
    cbz x10, .Ldays_from_date_invalid
    adrp x11, month_days
    add x11, x11, :lo12:month_days
    sub x12, x1, #1
    cmp x12, #MONTH_DAYS_LEN
    b.hs .Ldays_from_date_assert_index
    ldrb w13, [x11, x12]
    cmp x1, #FEBRUARY
    b.ne .Ldays_from_date_check_day
    tst x0, #3
    b.ne .Ldays_from_date_check_day
    mov x9, #100
    udiv x14, x0, x9
    msub x15, x14, x9, x0
    cbnz x15, .Ldays_from_date_leap
    mov x9, #YEARS_PER_ERA
    udiv x14, x0, x9
    msub x15, x14, x9, x0
    cbnz x15, .Ldays_from_date_check_day
.Ldays_from_date_leap:
    mov x13, #LEAP_FEBRUARY_DAYS
.Ldays_from_date_check_day:
    cmp x13, #MAX_MONTH_DAYS
    b.hi .Ldays_from_date_assert_month_days
    cmp x10, x13
    b.hi .Ldays_from_date_invalid
    mov x2, x10
    bl civil_days
    mov x1, x0
    mov x0, #0
    b .Ldays_from_date_return
.Ldays_from_date_invalid:
    mov x0, #ERROR_NO_DATE
.Ldays_from_date_return:
    ldp x29, x30, [sp], #16
    ret
.Ldays_from_date_assert_input:
    mov w0, #15
    bl assert_fail
.Ldays_from_date_assert_index:
    mov w0, #16
    bl assert_fail
.Ldays_from_date_assert_month_days:
    mov w0, #17
    bl assert_fail
.size days_from_date, . - days_from_date

// Counts the days from 1970-01-01 to a real date, by Howard Hinnant's days_from_civil.
// In: x0 = the year, 1970 to 9999, x1 = the month, 1 to 12, x2 = the day, 1 to 31.
// Out: x0 = the days.
.type civil_days, @function
civil_days:
    cmp x0, #MIN_YEAR
    b.lo .Lcivil_days_assert_year
    sub x9, x1, #1
    cmp x9, #MONTHS
    b.hs .Lcivil_days_assert_month
    mov x10, x2
    mov x11, x0
    cmp x1, #FEBRUARY
    b.hi .Lcivil_days_year
    sub x11, x11, #1
.Lcivil_days_year:
    mov x9, #YEARS_PER_ERA
    udiv x12, x11, x9
    msub x13, x12, x9, x11
    mov x9, #DAYS_PER_ERA & 0xffff
    movk x9, #DAYS_PER_ERA >> 16, lsl #16
    mul x14, x12, x9
    add x9, x1, #9
    mov x15, #MONTHS
    udiv x16, x9, x15
    msub x17, x16, x15, x9
    mov x15, #153
    mul x9, x17, x15
    add x9, x9, #2
    mov x15, #5
    udiv x9, x9, x15
    add x9, x9, x10
    sub x9, x9, #1
    add x14, x14, x9
    mov x15, #365
    mul x9, x13, x15
    add x14, x14, x9
    add x14, x14, x13, lsr #2
    mov x15, #100
    udiv x9, x13, x15
    sub x14, x14, x9
    mov x9, #EPOCH_SHIFT & 0xffff
    movk x9, #EPOCH_SHIFT >> 16, lsl #16
    sub x0, x14, x9
    ret
.Lcivil_days_assert_year:
    mov w0, #18
    bl assert_fail
.Lcivil_days_assert_month:
    mov w0, #19
    bl assert_fail
.size civil_days, . - civil_days

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
    mov w0, #20
    bl assert_fail
.Lwrite_i64_assert_digit_index:
    mov w0, #21
    bl assert_fail
.Lwrite_i64_assert_digits:
    mov w0, #22
    bl assert_fail
.Lwrite_i64_assert_sign_index:
    mov w0, #23
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
    mov w0, #24
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #25
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #26
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
