.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_WRITE, 1
.equ SYS_EXIT, 60
.equ SYS_CLOCK_GETTIME, 228
# File descriptors and clocks
.equ STDOUT, 1
.equ STDERR, 2
.equ CLOCK_REALTIME, 0
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_INVALID_DATE, 1
.equ EXIT_WRITE_FAILED, 2
.equ EXIT_USAGE, 3
.equ EXIT_CLOCK_FAILED, 4
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
# Argument counts, including the program's path
.equ ARGC_TIME, 1
.equ ARGC_DATE, 2
# Calendar
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
# Bit n set for each month n with 30 days: April, June, September, November.
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
# Sizes and limits
.equ DATE_LEN, 10
.equ MAX_WRITES, 64
.equ MAX_DIGITS, 19
.equ NEWLINE, 10
.equ USAGE_MSG_LEN, 26
.equ INVALID_MSG_LEN, 34
.equ OUT_BUF_LEN, 21
.equ TIME_BUF_LEN, 9
.equ CLOCK_FRAME, 24

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: clock [YYYY-MM-DD]\n"
.size usage_msg, . - usage_msg

.type invalid_msg, @object
invalid_msg:
    .ascii "invalid date: expected YYYY-MM-DD\n"
.size invalid_msg, . - invalid_msg

# What each of a date's ten characters must be: a digit (d) or a dash.
.type date_pattern, @object
date_pattern:
    .ascii "dddd-dd-dd"
.size date_pattern, . - date_pattern

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# Sign, 19 digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

# "hh:mm:ss" and a newline.
.type time_buf, @object
time_buf:
    .zero TIME_BUF_LEN
.size time_buf, . - time_buf

.text
.globl _start

# With no arguments prints the time of day; with a date prints the days until it.
.type _start, @function
_start:
    mov rax, qword ptr [rsp]
    cmp rax, ARGC_DATE
    je .Lstart_date
    cmp rax, ARGC_TIME
    jne .Lstart_usage
    call read_clock
    test rax, rax
    js .Lstart_clock_failed
    mov rdi, rdx
    call print_time
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_date:
    mov rdi, qword ptr [rsp + 16]
    test rdi, rdi
    jz .Lstart_assert_argv
    call parse_date
    test rax, rax
    js .Lstart_invalid
    mov rbx, rdx
    call read_day
    test rax, rax
    js .Lstart_clock_failed
    sub rbx, rdx
    jo .Lstart_assert_days
    mov rdi, rbx
    call write_i64
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_invalid:
    mov edi, STDERR
    lea rsi, [rip + invalid_msg]
    mov edx, INVALID_MSG_LEN
    call write_all
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_INVALID_DATE
    jmp .Lstart_exit
.Lstart_usage:
    mov edi, STDERR
    lea rsi, [rip + usage_msg]
    mov edx, USAGE_MSG_LEN
    call write_all
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_USAGE
    jmp .Lstart_exit
.Lstart_clock_failed:
    mov edi, EXIT_CLOCK_FAILED
    jmp .Lstart_exit
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_argv:
    mov edi, 1
    call assert_fail
.Lstart_assert_days:
    mov edi, 2
    call assert_fail
.size _start, . - _start

# Reads today's date.
# Out: rax = 0 and rdx = whole days since 1970-01-01, or a negative error code when the clock
# can't be read.
# returns: result
.type read_day, @function
read_day:
    sub rsp, 8
    call read_clock
    test rax, rax
    js .Lread_day_return
    test rdx, rdx
    js .Lread_day_assert_negative
    mov rax, rdx
    xor edx, edx
    mov ecx, SECONDS_PER_DAY
    div rcx
    cmp rdx, SECONDS_PER_DAY
    jae .Lread_day_assert_remainder
    mov rdx, rax
    xor eax, eax
.Lread_day_return:
    add rsp, 8
    ret
.Lread_day_assert_negative:
    mov edi, 21
    call assert_fail
.Lread_day_assert_remainder:
    mov edi, 22
    call assert_fail
.size read_day, . - read_day

# Reads the time of day.
# Out: rax = 0 and rdx = whole seconds since 1970, or a negative error code when the clock
# can't be read or reads before 1970.
# returns: result
.type read_clock, @function
read_clock:
    sub rsp, CLOCK_FRAME
    mov eax, SYS_CLOCK_GETTIME
    mov edi, CLOCK_REALTIME
    mov rsi, rsp
    syscall
    test rax, rax
    js .Lread_clock_return
    cmp qword ptr [rsp + 8], NS_PER_SECOND
    jae .Lread_clock_assert_ns
    mov rdx, qword ptr [rsp]
    test rdx, rdx
    js .Lread_clock_invalid
    xor eax, eax
    jmp .Lread_clock_return
.Lread_clock_invalid:
    mov rax, ERROR_INVALID
.Lread_clock_return:
    add rsp, CLOCK_FRAME
    ret
.Lread_clock_assert_ns:
    mov edi, 3
    call assert_fail
.size read_clock, . - read_clock

# Prints the time of day of a moment as hh:mm:ss and a newline to standard output.
# In: rdi = whole seconds since 1970, 0 or more.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_time, @function
print_time:
    sub rsp, 8
    test rdi, rdi
    js .Lprint_time_assert_negative
    mov rax, rdi
    xor edx, edx
    mov ecx, SECONDS_PER_DAY
    div rcx
    mov rax, rdx
    xor edx, edx
    mov ecx, SECONDS_PER_HOUR
    div rcx
    cmp rax, HOURS_PER_DAY
    jae .Lprint_time_assert_hours
    mov r8, rdx
    lea rsi, [rip + time_buf]
    mov ecx, 10
    xor edx, edx
    div ecx
    add al, '0'
    mov byte ptr [rsi], al
    add dl, '0'
    mov byte ptr [rsi + 1], dl
    mov byte ptr [rsi + 2], ':'
    mov rax, r8
    xor edx, edx
    mov ecx, SECONDS_PER_MINUTE
    div rcx
    mov r8, rdx
    mov ecx, 10
    xor edx, edx
    div ecx
    add al, '0'
    mov byte ptr [rsi + 3], al
    add dl, '0'
    mov byte ptr [rsi + 4], dl
    mov byte ptr [rsi + 5], ':'
    mov rax, r8
    xor edx, edx
    div ecx
    add al, '0'
    mov byte ptr [rsi + 6], al
    add dl, '0'
    mov byte ptr [rsi + 7], dl
    mov byte ptr [rsi + 8], NEWLINE
    mov edi, STDOUT
    mov edx, TIME_BUF_LEN
    call write_all
    test rax, rax
    js .Lprint_time_return
    xor eax, eax
.Lprint_time_return:
    add rsp, 8
    ret
.Lprint_time_assert_negative:
    mov edi, 4
    call assert_fail
.Lprint_time_assert_hours:
    mov edi, 5
    call assert_fail
.size print_time, . - print_time

# Parses a date written YYYY-MM-DD and turns it into days since 1970-01-01.
# In: rdi = a NUL-terminated string.
# Out: rax = 0 and rdx = the day's number, or rax = ERROR_INVALID when the text isn't a date
# from 1970-01-01 to 9999-12-31 on the calendar.
# returns: result
.type parse_date, @function
parse_date:
    sub rsp, 8
    test rdi, rdi
    jz .Lparse_date_assert_null
    xor eax, eax
    xor ecx, ecx
    xor r9d, r9d
    xor r10d, r10d
    lea r11, [rip + date_pattern]
    mov rsi, DATE_LEN
.Lparse_date_loop:  # bound: 10
    cmp rcx, DATE_LEN
    jae .Lparse_date_assert_index
    movzx edx, byte ptr [rdi + rcx]
    movzx r8d, byte ptr [r11 + rcx]
    cmp r8d, '-'
    jne .Lparse_date_digit
    cmp edx, '-'
    jne .Lparse_date_invalid
    mov r10, r9
    mov r9, rax
    xor eax, eax
    jmp .Lparse_date_next
.Lparse_date_digit:
    sub edx, '0'
    cmp edx, 9
    ja .Lparse_date_invalid
    imul rax, rax, 10
    add rax, rdx
.Lparse_date_next:
    inc rcx
    dec rsi
    jnz .Lparse_date_loop
    cmp byte ptr [rdi + DATE_LEN], 0
    jne .Lparse_date_invalid
    mov rdx, rax
    mov rdi, r10
    mov rsi, r9
    call civil_days
    test rax, rax
    js .Lparse_date_return
    xor eax, eax
    jmp .Lparse_date_return
.Lparse_date_invalid:
    mov rax, ERROR_INVALID
.Lparse_date_return:
    add rsp, 8
    ret
.Lparse_date_assert_null:
    mov edi, 6
    call assert_fail
.Lparse_date_assert_index:
    mov edi, 7
    call assert_fail
.size parse_date, . - parse_date

# Turns a year, month and day into days since 1970-01-01, after checking the date exists.
# In: rdi = the year, rsi = the month, rdx = the day, each 0 to 9999.
# Out: rax = 0 and rdx = the day's number, or rax = ERROR_INVALID when the date isn't from
# 1970-01-01 to 9999-12-31 on the calendar.
# returns: result
.type civil_days, @function
civil_days:
    push rbx
    push r12
    push r13
    mov rbx, rdi
    mov r12, rsi
    mov r13, rdx
    cmp rbx, MIN_YEAR
    jb .Lcivil_days_invalid
    cmp rbx, MAX_YEAR
    ja .Lcivil_days_invalid
    lea rax, [r12 - 1]
    cmp rax, MONTHS - 1
    ja .Lcivil_days_invalid
    test r13, r13
    jz .Lcivil_days_invalid
    call days_in_month
    cmp rax, MIN_MONTH_DAYS
    jb .Lcivil_days_assert_month_days
    cmp r13, rax
    ja .Lcivil_days_invalid
    mov rax, rbx
    cmp r12, FEBRUARY
    ja .Lcivil_days_march
    dec rax
.Lcivil_days_march:
    mov rdi, rax
    call year_days
    mov r10, rax
    lea rax, [r12 + MARCH_SHIFT]
    xor edx, edx
    mov ecx, MONTHS
    div rcx
    imul rax, rdx, MONTH_DAYS_FACTOR
    add rax, MONTH_DAYS_ROUND
    xor edx, edx
    mov ecx, MONTH_DAYS_DIVISOR
    div rcx
    add rax, r13
    dec rax
    add r10, rax
    mov r8, r10
    cmp r8, EPOCH_DAYS
    jb .Lcivil_days_assert_epoch
    sub r8, EPOCH_DAYS
    mov rdx, r8
    xor eax, eax
    jmp .Lcivil_days_return
.Lcivil_days_invalid:
    mov rax, ERROR_INVALID
.Lcivil_days_return:
    pop r13
    pop r12
    pop rbx
    ret
.Lcivil_days_assert_month_days:
    mov edi, 8
    call assert_fail
.Lcivil_days_assert_epoch:
    mov edi, 9
    call assert_fail
.size civil_days, . - civil_days

# Counts the days from 0000-03-01 to March 1 of a year, in the calendar that starts years in March.
# In: rdi = the year, 0 or more.
# Out: rax = the days.
.type year_days, @function
year_days:
    test rdi, rdi
    js .Lyear_days_assert_negative
    mov rax, rdi
    xor edx, edx
    mov ecx, YEARS_PER_ERA
    div rcx
    cmp rdx, YEARS_PER_ERA
    jae .Lyear_days_assert_era
    imul r8, rax, DAYS_PER_ERA
    mov r9, rdx
    imul r10, r9, DAYS_PER_YEAR
    mov rax, r9
    shr rax, 2
    add r10, rax
    mov rax, r9
    xor edx, edx
    mov ecx, YEARS_PER_CENTURY
    div rcx
    sub r10, rax
    lea rax, [r8 + r10]
    ret
.Lyear_days_assert_negative:
    mov edi, 19
    call assert_fail
.Lyear_days_assert_era:
    mov edi, 20
    call assert_fail
.size year_days, . - year_days

# Gives the number of days in a month.
# In: rdi = the year, 1 or more; rsi = the month, 1 to 12.
# Out: rax = 28 to 31.
.type days_in_month, @function
days_in_month:
    test rdi, rdi
    jz .Ldays_in_month_assert_year
    lea rax, [rsi - 1]
    cmp rax, MONTHS - 1
    ja .Ldays_in_month_assert_month
    cmp rsi, FEBRUARY
    je .Ldays_in_month_february
    mov eax, MAX_MONTH_DAYS
    mov edx, SHORT_MONTHS
    bt edx, esi
    jnc .Ldays_in_month_return
    dec eax
    jmp .Ldays_in_month_return
.Ldays_in_month_february:
    test edi, 3
    jnz .Ldays_in_month_28
    mov eax, edi
    xor edx, edx
    mov ecx, YEARS_PER_CENTURY
    div ecx
    test edx, edx
    jnz .Ldays_in_month_29
    mov eax, edi
    xor edx, edx
    mov ecx, YEARS_PER_ERA
    div ecx
    test edx, edx
    jnz .Ldays_in_month_28
.Ldays_in_month_29:
    mov eax, MIN_MONTH_DAYS + 1
    jmp .Ldays_in_month_return
.Ldays_in_month_28:
    mov eax, MIN_MONTH_DAYS
.Ldays_in_month_return:
    ret
.Ldays_in_month_assert_year:
    mov edi, 10
    call assert_fail
.Ldays_in_month_assert_month:
    mov edi, 11
    call assert_fail
.size days_in_month, . - days_in_month

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
    mov edi, 12
    call assert_fail
.Lwrite_i64_assert_digit_index:
    mov edi, 13
    call assert_fail
.Lwrite_i64_assert_digits:
    mov edi, 14
    call assert_fail
.Lwrite_i64_assert_sign_index:
    mov edi, 15
    call assert_fail
.size write_i64, . - write_i64

# Writes rdx bytes from rsi to file descriptor edi, retrying partial writes.
# In: edi = the file descriptor, rsi = the bytes, rdx = how many, at least 1.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_all, @function
write_all:
    test rsi, rsi
    jz .Lwrite_all_assert_null
    test rdx, rdx
    jz .Lwrite_all_assert_empty
    mov r8, MAX_WRITES
.Lwrite_all_loop:  # bound: 64
    mov eax, SYS_WRITE
    syscall
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
    ret
.Lwrite_all_assert_null:
    mov edi, 16
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 17
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 18
    call assert_fail
.size write_all, . - write_all

# lengua's assertion handler. check_code compares it word for word: copy it exactly.
# In: edi = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
# standard error and ends the program, every thread of it, with code 134.
.type assert_fail, @function
assert_fail:
    sub rsp, 40
    mov rax, qword ptr [rip + assert_message]
    mov qword ptr [rsp], rax
    mov rax, qword ptr [rip + assert_message + 8]
    mov qword ptr [rsp + 8], rax
    mov rax, qword ptr [rip + assert_message + 16]
    mov qword ptr [rsp + 16], rax
    mov eax, dword ptr [rip + assert_message + 24]
    mov dword ptr [rsp + 24], eax
    mov al, byte ptr [rip + assert_message + 28]
    mov byte ptr [rsp + 28], al
    mov eax, edi
    xor edx, edx
    mov ecx, 100
    div ecx
    add al, 48
    mov byte ptr [rsp + 18], al
    mov eax, edx
    xor edx, edx
    mov ecx, 10
    div ecx
    add al, 48
    mov byte ptr [rsp + 19], al
    add dl, 48
    mov byte ptr [rsp + 20], dl
    mov eax, 1
    mov edi, 2
    mov rsi, rsp
    mov edx, 29
    syscall
    mov eax, 231
    mov edi, 134
    syscall
.size assert_fail, . - assert_fail
