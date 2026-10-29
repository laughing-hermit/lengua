.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_WRITE, 1
.equ SYS_EXIT, 60
# File descriptors
.equ STDOUT, 1
.equ STDERR, 2
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_NOT_A_NUMBER, 2
.equ EXIT_OUT_OF_RANGE, 3
.equ EXIT_WRITE_FAILED, 4
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
.equ ERROR_RANGE, -34
# Sizes and limits
.equ ARGC_EXPECTED, 3
.equ NUM_LIMIT, 1000000
.equ MAX_CHARS, 131072
.equ MAX_DIGITS, 19
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ USAGE_MSG_LEN, 23
.equ NAN_MSG_LEN, 28
.equ RANGE_MSG_LEN, 29
.equ OUT_BUF_LEN, 21
.equ FRAME_SIZE, 16

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: absdiff <a> <b>\n"
.size usage_msg, . - usage_msg

.type nan_msg, @object
nan_msg:
    .ascii "absdiff: not a whole number\n"
.size nan_msg, . - nan_msg

.type range_msg, @object
range_msg:
    .ascii "absdiff: number out of range\n"
.size range_msg, . - range_msg

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

.text
.globl _start

# Checks the argument count, parses both numbers, prints their absolute difference and exits.
# The frame holds a at [rsp] and the second argument's address at [rsp + 8].
.type _start, @function
_start:
    sub rsp, FRAME_SIZE
    mov rax, qword ptr [rsp + FRAME_SIZE]
    cmp rax, ARGC_EXPECTED
    jne .Lstart_usage
    mov rdi, qword ptr [rsp + FRAME_SIZE + 16]
    mov rax, qword ptr [rsp + FRAME_SIZE + 24]
    test rax, rax
    jz .Lstart_assert_argv
    mov qword ptr [rsp + 8], rax
    call parse_bounded
    test rax, rax
    js .Lstart_parse_error
    mov qword ptr [rsp], rdx
    mov rdi, qword ptr [rsp + 8]
    call parse_bounded
    test rax, rax
    js .Lstart_parse_error
    mov rax, qword ptr [rsp]
    sub rax, rdx
    mov rdi, rax
    neg rdi
    cmovs rdi, rax
    test rdi, rdi
    js .Lstart_assert_abs
    call write_i64
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_parse_error:
    mov qword ptr [rsp], rax
    mov rdi, rax
    call report_parse_error
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_NOT_A_NUMBER
    cmp qword ptr [rsp], ERROR_INVALID
    je .Lstart_exit
    mov edi, EXIT_OUT_OF_RANGE
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
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_argv:
    mov edi, 1
    call assert_fail
.Lstart_assert_abs:
    mov edi, 2
    call assert_fail
.size _start, . - _start

# Prints the message for an argument parse_bounded rejected.
# In: rdi = ERROR_INVALID or ERROR_RANGE.
# Out: rax = 0, or a negative error code.
# returns: result
.type report_parse_error, @function
report_parse_error:
    sub rsp, 8
    cmp rdi, ERROR_INVALID
    je .Lreport_not_a_number
    cmp rdi, ERROR_RANGE
    je .Lreport_out_of_range
    jmp .Lreport_assert_code
.Lreport_not_a_number:
    lea rsi, [rip + nan_msg]
    mov edx, NAN_MSG_LEN
    jmp .Lreport_write
.Lreport_out_of_range:
    lea rsi, [rip + range_msg]
    mov edx, RANGE_MSG_LEN
.Lreport_write:
    mov edi, STDERR
    call write_all
    test rax, rax
    js .Lreport_return
    xor eax, eax
.Lreport_return:
    add rsp, 8
    ret
.Lreport_assert_code:
    mov edi, 12
    call assert_fail
.size report_parse_error, . - report_parse_error

# Parses a decimal whole number with an optional leading minus sign.
# In: rdi = a NUL-terminated string.
# Out: rax = 0 and rdx = the number; rax = ERROR_INVALID when the text isn't a whole number;
# rax = ERROR_RANGE when it is one outside -NUM_LIMIT to NUM_LIMIT.
# returns: result
.type parse_bounded, @function
parse_bounded:
    test rdi, rdi
    jz .Lparse_assert_null
    xor r9d, r9d
    cmp byte ptr [rdi], '-'
    jne .Lparse_digits
    mov r9d, 1
    inc rdi
.Lparse_digits:
    cmp byte ptr [rdi], 0
    je .Lparse_invalid
    xor eax, eax
    xor ecx, ecx
    mov r8, MAX_CHARS
.Lparse_loop:  # bound: 131072
    cmp rcx, MAX_CHARS
    jae .Lparse_assert_index
    movzx edx, byte ptr [rdi + rcx]
    test edx, edx
    jz .Lparse_end
    sub edx, '0'
    cmp edx, 9
    ja .Lparse_invalid
    cmp rax, NUM_LIMIT
    ja .Lparse_next
    imul rax, rax, 10
    add rax, rdx
.Lparse_next:
    inc rcx
    dec r8
    jnz .Lparse_loop
    jmp .Lparse_assert_length
.Lparse_end:
    cmp rax, NUM_LIMIT
    ja .Lparse_range
    test r9d, r9d
    jz .Lparse_positive
    neg rax
.Lparse_positive:
    mov rdx, rax
    xor eax, eax
    jmp .Lparse_return
.Lparse_range:
    mov rax, ERROR_RANGE
    jmp .Lparse_return
.Lparse_invalid:
    mov rax, ERROR_INVALID
.Lparse_return:
    ret
.Lparse_assert_null:
    mov edi, 3
    call assert_fail
.Lparse_assert_index:
    mov edi, 4
    call assert_fail
.Lparse_assert_length:
    mov edi, 13
    call assert_fail
.size parse_bounded, . - parse_bounded

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
    mov edi, 5
    call assert_fail
.Lwrite_i64_assert_digit_index:
    mov edi, 6
    call assert_fail
.Lwrite_i64_assert_digits:
    mov edi, 7
    call assert_fail
.Lwrite_i64_assert_sign_index:
    mov edi, 8
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
    mov edi, 9
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 10
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 11
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
