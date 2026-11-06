.intel_syntax noprefix

# Clocks and file descriptors
.equ CLOCK_REALTIME, 0
.equ STDOUT, 1
.equ STDERR, 2
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_NO_DATE, 2
.equ EXIT_WRITE_FAILED, 4
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
.equ ERROR_NO_DATE, -34
# Argument counts, including the program's path
.equ ARGC_TIME, 1
.equ ARGC_DATE, 2
# Time
.equ MAX_CLOCK, 253402300799
.equ TIMESPEC_SIZE, 16
.equ SECONDS_PER_DAY, 86400
.equ SECONDS_PER_HOUR, 3600
.equ SECONDS_PER_MINUTE, 60
.equ HOURS_PER_DAY, 24
# Dates
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
# Output
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

# A d stands for a digit, a dash for itself.
.type date_pattern, @object
date_pattern:
    .ascii "dddd-dd-dd"
.size date_pattern, . - date_pattern

# Days in each month of a year that is not a leap year.
.type month_days, @object
month_days:
    .byte 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31
.size month_days, . - month_days

.section .data

# hh:mm:ss and a newline; print_time fills in the digits.
.type time_buf, @object
time_buf:
    .ascii "00:00:00\n"
.size time_buf, . - time_buf

.section .bss

# Sign, 19 digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
# Prints the time with no argument, the days until a date with one, and usage otherwise.
.type main, @function
main:
    sub rsp, 8
    cmp rdi, ARGC_TIME
    je .Lmain_time
    cmp rdi, ARGC_DATE
    jne .Lmain_usage
    mov rbx, qword ptr [rsi + 8]
    mov rdi, rbx
    call days_until
    test rax, rax
    jns .Lmain_days
    cmp rax, ERROR_INVALID
    je .Lmain_usage
    cmp rax, ERROR_NO_DATE
    je .Lmain_no_date
    jmp .Lmain_assert_error
.Lmain_days:
    mov rdi, rdx
    call write_i64
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_time:
    call print_time
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_no_date:
    mov edi, STDERR
    lea rsi, [rip + no_date_msg]
    mov edx, NO_DATE_MSG_LEN
    call write_all
    test rax, rax
    js .Lmain_write_failed
    mov edi, STDERR
    mov rsi, rbx
    mov edx, DATE_LEN
    call write_all
    test rax, rax
    js .Lmain_write_failed
    mov edi, STDERR
    lea rsi, [rip + newline_msg]
    mov edx, NEWLINE_MSG_LEN
    call write_all
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_NO_DATE
    jmp .Lmain_exit
.Lmain_usage:
    mov edi, STDERR
    lea rsi, [rip + usage_msg]
    mov edx, USAGE_MSG_LEN
    call write_all
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_USAGE
    jmp .Lmain_exit
.Lmain_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_error:
    mov edi, 1
    call assert_fail
.size main, . - main

# Reads the time of day.
# Out: rax = seconds since 1970, from 0 to MAX_CLOCK.
.type read_clock, @function
read_clock:
    sub rsp, 8
    sub rsp, TIMESPEC_SIZE
    mov edi, CLOCK_REALTIME
    mov rsi, rsp
    call os_clock
    test rax, rax
    js .Lread_clock_assert_failed
    mov rax, qword ptr [rsp]
    mov rcx, MAX_CLOCK
    cmp rax, rcx
    ja .Lread_clock_assert_range
    add rsp, TIMESPEC_SIZE
    add rsp, 8
    ret
.Lread_clock_assert_failed:
    mov edi, 2
    call assert_fail
.Lread_clock_assert_range:
    mov edi, 3
    call assert_fail
.size read_clock, . - read_clock

# Reads the clock and prints the time of day in UTC as hh:mm:ss and a newline to standard
# output.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_time, @function
print_time:
    sub rsp, 8
    call read_clock
    xor edx, edx
    mov ecx, SECONDS_PER_DAY
    div rcx
    mov rax, rdx
    xor edx, edx
    mov ecx, SECONDS_PER_HOUR
    div rcx
    cmp rax, HOURS_PER_DAY
    jae .Lprint_time_assert_hours
    mov r9, rdx
    mov ecx, 10
    xor edx, edx
    div rcx
    add al, '0'
    mov byte ptr [rip + time_buf], al
    add dl, '0'
    mov byte ptr [rip + time_buf + 1], dl
    mov rax, r9
    xor edx, edx
    mov ecx, SECONDS_PER_MINUTE
    div rcx
    cmp rax, SECONDS_PER_MINUTE
    jae .Lprint_time_assert_minutes
    mov r9, rdx
    mov ecx, 10
    xor edx, edx
    div rcx
    add al, '0'
    mov byte ptr [rip + time_buf + 3], al
    add dl, '0'
    mov byte ptr [rip + time_buf + 4], dl
    mov rax, r9
    xor edx, edx
    div rcx
    add al, '0'
    mov byte ptr [rip + time_buf + 6], al
    add dl, '0'
    mov byte ptr [rip + time_buf + 7], dl
    mov edi, STDOUT
    lea rsi, [rip + time_buf]
    mov edx, TIME_BUF_LEN
    call write_all
    test rax, rax
    js .Lprint_time_return
    xor eax, eax
.Lprint_time_return:
    add rsp, 8
    ret
.Lprint_time_assert_hours:
    mov edi, 4
    call assert_fail
.Lprint_time_assert_minutes:
    mov edi, 5
    call assert_fail
.size print_time, . - print_time

# Counts the days from today's UTC date to a date written as YYYY-MM-DD.
# In: rdi = a NUL-terminated string.
# Out: rax = 0 and rdx = the days (negative for a past date), or rax = ERROR_INVALID when the
# text isn't in the date's shape, or ERROR_NO_DATE when it names no real day.
# returns: result
.type days_until, @function
days_until:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Ldays_until_assert_null
    mov rbx, rdi
    call check_shape
    test rax, rax
    js .Ldays_until_return
    mov rdi, rbx
    mov esi, YEAR_DIGITS
    call digits_value
    mov r12, rax
    lea rdi, [rbx + MONTH_OFFSET]
    mov esi, FIELD_DIGITS
    call digits_value
    mov r13, rax
    lea rdi, [rbx + DAY_OFFSET]
    mov esi, FIELD_DIGITS
    call digits_value
    mov rdx, rax
    mov rdi, r12
    mov rsi, r13
    call days_from_date
    test rax, rax
    js .Ldays_until_return
    mov r12, rdx
    call read_clock
    xor edx, edx
    mov ecx, SECONDS_PER_DAY
    div rcx
    sub r12, rax
    jo .Ldays_until_assert_overflow
    mov rdx, r12
    xor eax, eax
.Ldays_until_return:
    pop r13
    pop r12
    pop rbx
    ret
.Ldays_until_assert_null:
    mov edi, 6
    call assert_fail
.Ldays_until_assert_overflow:
    mov edi, 7
    call assert_fail
.size days_until, . - days_until

# Checks that a string is four digits, a dash, two digits, a dash and two digits, and no more.
# In: rdi = a NUL-terminated string.
# Out: rax = 0, or ERROR_INVALID.
# returns: result
.type check_shape, @function
check_shape:
    test rdi, rdi
    jz .Lcheck_shape_assert_null
    lea rsi, [rip + date_pattern]
    xor ecx, ecx
    mov r8, DATE_LEN
.Lcheck_shape_loop:  # bound: 10
    cmp rcx, DATE_PATTERN_LEN
    jae .Lcheck_shape_assert_index
    movzx eax, byte ptr [rdi + rcx]
    movzx edx, byte ptr [rsi + rcx]
    cmp edx, '-'
    je .Lcheck_shape_dash
    cmp edx, 'd'
    jne .Lcheck_shape_assert_pattern
    sub eax, '0'
    cmp eax, 9
    ja .Lcheck_shape_invalid
    jmp .Lcheck_shape_next
.Lcheck_shape_dash:
    cmp eax, '-'
    jne .Lcheck_shape_invalid
.Lcheck_shape_next:
    inc rcx
    dec r8
    jnz .Lcheck_shape_loop
    cmp byte ptr [rdi + DATE_LEN], 0
    jne .Lcheck_shape_invalid
    xor eax, eax
    jmp .Lcheck_shape_return
.Lcheck_shape_invalid:
    mov rax, ERROR_INVALID
.Lcheck_shape_return:
    ret
.Lcheck_shape_assert_null:
    mov edi, 8
    call assert_fail
.Lcheck_shape_assert_index:
    mov edi, 9
    call assert_fail
.Lcheck_shape_assert_pattern:
    mov edi, 10
    call assert_fail
.size check_shape, . - check_shape

# Reads the value of a run of decimal digits that check_shape has already checked.
# In: rdi = the first digit, rsi = how many digits, 1 to MAX_FIELD_DIGITS.
# Out: rax = their value.
.type digits_value, @function
digits_value:
    test rdi, rdi
    jz .Ldigits_value_assert_null
    cmp rsi, MAX_FIELD_DIGITS
    ja .Ldigits_value_assert_count
    xor eax, eax
    xor ecx, ecx
    mov r8, MAX_FIELD_DIGITS
.Ldigits_value_loop:  # bound: 4
    cmp rcx, rsi
    jae .Ldigits_value_return
    cmp rcx, MAX_FIELD_DIGITS
    jae .Ldigits_value_assert_index
    movzx edx, byte ptr [rdi + rcx]
    sub edx, '0'
    cmp edx, 9
    ja .Ldigits_value_assert_digit
    imul eax, eax, 10
    add eax, edx
    inc rcx
    dec r8
    jnz .Ldigits_value_loop
.Ldigits_value_return:
    ret
.Ldigits_value_assert_null:
    mov edi, 11
    call assert_fail
.Ldigits_value_assert_count:
    mov edi, 12
    call assert_fail
.Ldigits_value_assert_index:
    mov edi, 13
    call assert_fail
.Ldigits_value_assert_digit:
    mov edi, 14
    call assert_fail
.size digits_value, . - digits_value

# Checks that a year, month and day name a real day from 1970 to 9999, then counts its days.
# In: rdi = the year, rsi = the month, rdx = the day, each from 0 to 9999.
# Out: rax = 0 and rdx = days since 1970-01-01, or rax = ERROR_NO_DATE.
# returns: result
.type days_from_date, @function
days_from_date:
    sub rsp, 8
    cmp rdi, MAX_YEAR
    ja .Ldays_from_date_assert_input
    mov r10, rdx
    cmp rdi, MIN_YEAR
    jb .Ldays_from_date_invalid
    test rsi, rsi
    jz .Ldays_from_date_invalid
    cmp rsi, MONTHS
    ja .Ldays_from_date_invalid
    test r10, r10
    jz .Ldays_from_date_invalid
    lea r9, [rip + month_days]
    lea rcx, [rsi - 1]
    cmp rcx, MONTH_DAYS_LEN
    jae .Ldays_from_date_assert_index
    movzx r8d, byte ptr [r9 + rcx]
    cmp rsi, FEBRUARY
    jne .Ldays_from_date_check_day
    test edi, 3
    jnz .Ldays_from_date_check_day
    mov rax, rdi
    xor edx, edx
    mov ecx, 100
    div rcx
    test rdx, rdx
    jnz .Ldays_from_date_leap
    mov rax, rdi
    xor edx, edx
    mov ecx, YEARS_PER_ERA
    div rcx
    test rdx, rdx
    jnz .Ldays_from_date_check_day
.Ldays_from_date_leap:
    mov r8d, LEAP_FEBRUARY_DAYS
.Ldays_from_date_check_day:
    cmp r8, MAX_MONTH_DAYS
    ja .Ldays_from_date_assert_month_days
    cmp r10, r8
    ja .Ldays_from_date_invalid
    mov rdx, r10
    call civil_days
    mov rdx, rax
    xor eax, eax
    jmp .Ldays_from_date_return
.Ldays_from_date_invalid:
    mov rax, ERROR_NO_DATE
.Ldays_from_date_return:
    add rsp, 8
    ret
.Ldays_from_date_assert_input:
    mov edi, 15
    call assert_fail
.Ldays_from_date_assert_index:
    mov edi, 16
    call assert_fail
.Ldays_from_date_assert_month_days:
    mov edi, 17
    call assert_fail
.size days_from_date, . - days_from_date

# Counts the days from 1970-01-01 to a real date, by Howard Hinnant's days_from_civil.
# In: rdi = the year, 1970 to 9999, rsi = the month, 1 to 12, rdx = the day, 1 to 31.
# Out: rax = the days.
.type civil_days, @function
civil_days:
    cmp rdi, MIN_YEAR
    jb .Lcivil_days_assert_year
    lea rcx, [rsi - 1]
    cmp rcx, MONTHS
    jae .Lcivil_days_assert_month
    mov r10, rdx
    mov rax, rdi
    cmp rsi, FEBRUARY
    ja .Lcivil_days_year
    dec rax
.Lcivil_days_year:
    xor edx, edx
    mov ecx, YEARS_PER_ERA
    div rcx
    imul r9, rax, DAYS_PER_ERA
    mov r11, rdx
    lea rax, [rsi + 9]
    xor edx, edx
    mov ecx, MONTHS
    div rcx
    imul rax, rdx, 153
    add rax, 2
    xor edx, edx
    mov ecx, 5
    div rcx
    add rax, r10
    dec rax
    add r9, rax
    imul rax, r11, 365
    add r9, rax
    mov rax, r11
    shr rax, 2
    add r9, rax
    mov rax, r11
    xor edx, edx
    mov ecx, 100
    div rcx
    sub r9, rax
    sub r9, EPOCH_SHIFT
    mov rax, r9
    ret
.Lcivil_days_assert_year:
    mov edi, 18
    call assert_fail
.Lcivil_days_assert_month:
    mov edi, 19
    call assert_fail
.size civil_days, . - civil_days

# Writes a whole number in decimal, then a newline, to standard output.
# In: rdi = the number, never -9223372036854775808.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_i64, @function
write_i64:
    sub rsp, 8
    mov rax, rdi
    mov r9, rdi
    test rax, rax
    jns .Lwrite_i64_digits
    neg rax
    jo .Lwrite_i64_assert_min
.Lwrite_i64_digits:
    mov byte ptr [rip + out_buf + OUT_BUF_LEN - 1], NEWLINE
    lea rsi, [rip + out_buf]
    mov ecx, OUT_BUF_LEN - 1
    mov r10d, 10
    mov r8, MAX_DIGITS
.Lwrite_i64_digit:  # bound: 19
    xor edx, edx
    div r10
    add edx, '0'
    dec ecx
    cmp ecx, OUT_BUF_LEN
    jae .Lwrite_i64_assert_digit_index
    mov byte ptr [rsi + rcx], dl
    test rax, rax
    jz .Lwrite_i64_sign
    dec r8
    jnz .Lwrite_i64_digit
    jmp .Lwrite_i64_assert_digits
.Lwrite_i64_sign:
    test r9, r9
    jns .Lwrite_i64_output
    dec ecx
    cmp ecx, OUT_BUF_LEN
    jae .Lwrite_i64_assert_sign_index
    mov byte ptr [rsi + rcx], '-'
.Lwrite_i64_output:
    add rsi, rcx
    mov edx, OUT_BUF_LEN
    sub edx, ecx
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lwrite_i64_return
    xor eax, eax
.Lwrite_i64_return:
    add rsp, 8
    ret
.Lwrite_i64_assert_min:
    mov edi, 20
    call assert_fail
.Lwrite_i64_assert_digit_index:
    mov edi, 21
    call assert_fail
.Lwrite_i64_assert_digits:
    mov edi, 22
    call assert_fail
.Lwrite_i64_assert_sign_index:
    mov edi, 23
    call assert_fail
.size write_i64, . - write_i64

# Writes rdx bytes from rsi to file descriptor edi, retrying partial writes.
# In: edi = the file descriptor, rsi = the bytes, rdx = how many, at least 1.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_all, @function
write_all:
    sub rsp, 8
    test rsi, rsi
    jz .Lwrite_all_assert_null
    test rdx, rdx
    jz .Lwrite_all_assert_empty
    mov r8, MAX_WRITES
.Lwrite_all_loop:  # bound: 64
    call os_write
    test rax, rax
    js .Lwrite_all_return
    cmp rax, rdx
    ja .Lwrite_all_assert_count
    add rsi, rax
    sub rdx, rax
    jz .Lwrite_all_done
    dec r8
    jnz .Lwrite_all_loop
    mov rax, ERROR_IO
    jmp .Lwrite_all_return
.Lwrite_all_done:
    xor eax, eax
.Lwrite_all_return:
    add rsp, 8
    ret
.Lwrite_all_assert_null:
    mov edi, 24
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 25
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 26
    call assert_fail
.size write_all, . - write_all

