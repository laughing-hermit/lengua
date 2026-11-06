.intel_syntax noprefix

# File descriptors
.equ STDOUT, 1
.equ STDERR, 2
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_NOT_A_NUMBER, 2
.equ EXIT_WRITE_FAILED, 3
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
# Sizes and limits
.equ ARGC_EXPECTED, 3
.equ MIN_VALUE, -1000000000
.equ MAX_VALUE, 1000000000
.equ MAX_CHARS, 20
.equ MAX_DIGITS, 19
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ USAGE_MSG_LEN, 19
.equ NAN_MSG_LEN, 20
.equ OUT_BUF_LEN, 21

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: add <a> <b>\n"
.size usage_msg, . - usage_msg

.type nan_msg, @object
nan_msg:
    .ascii "error: not a number\n"
.size nan_msg, . - nan_msg

.section .bss

# Sign, 19 digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
# Checks the argument count, parses both numbers, prints their sum and exits. lengua's `_start`
# calls main with the argument count and the array of arguments in rdi and rsi; main never returns.
.type main, @function
main:
    sub rsp, 8
    mov r12, rsi
    cmp rdi, ARGC_EXPECTED
    jne .Lmain_usage
    mov rdi, qword ptr [r12 + 8]
    test rdi, rdi
    jz .Lmain_assert_first
    call parse_i64
    test rax, rax
    js .Lmain_not_a_number
    cmp rdx, MIN_VALUE
    jl .Lmain_not_a_number
    cmp rdx, MAX_VALUE
    jg .Lmain_not_a_number
    mov rbx, rdx
    mov rdi, qword ptr [r12 + 16]
    test rdi, rdi
    jz .Lmain_assert_second
    call parse_i64
    test rax, rax
    js .Lmain_not_a_number
    cmp rdx, MIN_VALUE
    jl .Lmain_not_a_number
    cmp rdx, MAX_VALUE
    jg .Lmain_not_a_number
    add rbx, rdx
    jo .Lmain_assert_sum
    mov rdi, rbx
    call write_i64
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_OK
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
.Lmain_not_a_number:
    mov edi, STDERR
    lea rsi, [rip + nan_msg]
    mov edx, NAN_MSG_LEN
    call write_all
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_NOT_A_NUMBER
    jmp .Lmain_exit
.Lmain_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_first:
    mov edi, 1
    call assert_fail
.Lmain_assert_second:
    mov edi, 2
    call assert_fail
.Lmain_assert_sum:
    mov edi, 3
    call assert_fail
.size main, . - main

# Parses a decimal whole number with an optional leading minus sign.
# In: rdi = a NUL-terminated string.
# Out: rax = 0 and rdx = the number, or rax = ERROR_INVALID when the text isn't a number
# between -9223372036854775807 and 9223372036854775807.
# returns: result
.type parse_i64, @function
parse_i64:
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
.Lparse_loop:  # bound: 20
    cmp rcx, MAX_CHARS
    jae .Lparse_assert_index
    movzx edx, byte ptr [rdi + rcx]
    test edx, edx
    jz .Lparse_end
    sub edx, '0'
    cmp edx, 9
    ja .Lparse_invalid
    imul rax, rax, 10
    jo .Lparse_invalid
    add rax, rdx
    jo .Lparse_invalid
    inc rcx
    dec r8
    jnz .Lparse_loop
    jmp .Lparse_invalid
.Lparse_end:
    test r9d, r9d
    jz .Lparse_positive
    neg rax
.Lparse_positive:
    mov rdx, rax
    xor eax, eax
    jmp .Lparse_return
.Lparse_invalid:
    mov rax, ERROR_INVALID
.Lparse_return:
    ret
.Lparse_assert_null:
    mov edi, 4
    call assert_fail
.Lparse_assert_index:
    mov edi, 5
    call assert_fail
.size parse_i64, . - parse_i64

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
    mov edi, 6
    call assert_fail
.Lwrite_i64_assert_digit_index:
    mov edi, 7
    call assert_fail
.Lwrite_i64_assert_digits:
    mov edi, 8
    call assert_fail
.Lwrite_i64_assert_sign_index:
    mov edi, 9
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
    mov edi, 10
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 11
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 12
    call assert_fail
.size write_all, . - write_all

