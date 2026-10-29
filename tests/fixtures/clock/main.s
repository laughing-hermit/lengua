.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_WRITE, 1
.equ SYS_EXIT, 60
.equ SYS_CLOCK_GETTIME, 228
# Clocks and file descriptors
.equ CLOCK_REALTIME, 0
.equ STDOUT, 1
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_WRITE_FAILED, 2
.equ EXIT_CLOCK_FAILED, 3
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# Sizes and limits
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ COLON, 58
.equ TIMESPEC_LEN, 16
.equ OUT_BUF_LEN, 9
.equ SECONDS_PER_DAY, 86400
.equ SECONDS_PER_HOUR, 3600
.equ SECONDS_PER_MINUTE, 60
.equ HOURS_PER_DAY, 24
.equ MINUTES_PER_HOUR, 60
.equ NANOS_PER_SECOND, 1000000000
.equ DECIMAL_BASE, 10

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# Seconds, then nanoseconds, as clock_gettime fills them.
.type timespec, @object
timespec:
    .zero TIMESPEC_LEN
.size timespec, . - timespec

# "HH:MM:SS" and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

# Reads the clock, prints the UTC time of day and exits.
.type _start, @function
_start:
    mov eax, SYS_CLOCK_GETTIME
    mov edi, CLOCK_REALTIME
    lea rsi, [rip + timespec]
    syscall
    test rax, rax
    js .Lstart_clock_failed
    mov rax, qword ptr [rip + timespec + 8]
    cmp rax, NANOS_PER_SECOND
    jae .Lstart_assert_nanos
    mov rdi, qword ptr [rip + timespec]
    test rdi, rdi
    js .Lstart_assert_seconds
    call print_time
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_clock_failed:
    mov edi, EXIT_CLOCK_FAILED
    jmp .Lstart_exit
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_nanos:
    mov edi, 1
    call assert_fail
.Lstart_assert_seconds:
    mov edi, 2
    call assert_fail
.size _start, . - _start

# Prints the time of day of a moment as HH:MM:SS and a newline to standard output.
# In: rdi = seconds since 1970, at least 0.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_time, @function
print_time:
    sub rsp, 8
    lea rsi, [rip + out_buf]
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
    mov ecx, DECIMAL_BASE
    xor edx, edx
    div rcx
    add al, '0'
    mov byte ptr [rsi], al
    add dl, '0'
    mov byte ptr [rsi + 1], dl
    mov byte ptr [rsi + 2], COLON
    mov rax, r8
    xor edx, edx
    mov ecx, SECONDS_PER_MINUTE
    div rcx
    cmp rax, MINUTES_PER_HOUR
    jae .Lprint_time_assert_minutes
    mov r8, rdx
    mov ecx, DECIMAL_BASE
    xor edx, edx
    div rcx
    add al, '0'
    mov byte ptr [rsi + 3], al
    add dl, '0'
    mov byte ptr [rsi + 4], dl
    mov byte ptr [rsi + 5], COLON
    mov rax, r8
    cmp rax, SECONDS_PER_MINUTE
    jae .Lprint_time_assert_seconds
    xor edx, edx
    div rcx
    add al, '0'
    mov byte ptr [rsi + 6], al
    add dl, '0'
    mov byte ptr [rsi + 7], dl
    mov byte ptr [rsi + 8], NEWLINE
    mov edi, STDOUT
    mov edx, OUT_BUF_LEN
    call write_all
    test rax, rax
    js .Lprint_time_return
    xor eax, eax
.Lprint_time_return:
    add rsp, 8
    ret
.Lprint_time_assert_hours:
    mov edi, 3
    call assert_fail
.Lprint_time_assert_minutes:
    mov edi, 4
    call assert_fail
.Lprint_time_assert_seconds:
    mov edi, 5
    call assert_fail
.size print_time, . - print_time

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
    mov edi, 6
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 7
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 8
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
