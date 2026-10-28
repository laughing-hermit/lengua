# lengua: spec sha256:fc7a82ce6d83bf7be9f82d9e32c4427a83d058aaa10a36e270417ebc4da5baa8
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
.equ ERROR_RANGE, -34
.equ ERROR_INVALID, -22
# Sizes and limits
.equ ARGC_EXPECTED, 3
.equ MAX_ARG_LEN, 131072
.equ MAX_WHOLE, 1000000
.equ MAX_CENTS, 100000000
.equ CENTS_PER_UNIT, 100
.equ MAX_DIGITS, 19
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ USAGE_MSG_LEN, 20
.equ NAN_MSG_LEN, 20
.equ RANGE_MSG_LEN, 20
.equ NEWLINE_MSG_LEN, 1
.equ OUT_BUF_LEN, 24

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: mean <a> <b>\n"
.size usage_msg, . - usage_msg

.type nan_msg, @object
nan_msg:
    .ascii "mean: not a number: "
.size nan_msg, . - nan_msg

.type range_msg, @object
range_msg:
    .ascii "mean: out of range: "
.size range_msg, . - range_msg

.type newline_msg, @object
newline_msg:
    .ascii "\n"
.size newline_msg, . - newline_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# Sign, 19 digits, a point, 2 decimals and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

# Checks the argument count, parses both numbers in cents, prints their mean and exits.
.type _start, @function
_start:
    mov rax, qword ptr [rsp]
    cmp rax, ARGC_EXPECTED
    jne .Lstart_usage
    mov r12, qword ptr [rsp + 16]
    mov r13, qword ptr [rsp + 24]
    test r12, r12
    jz .Lstart_assert_argv_a
    test r13, r13
    jz .Lstart_assert_argv_b
    mov rdi, r12
    call parse_cents
    test rax, rax
    js .Lstart_a_bad
    mov r14, rdx
    mov rdi, r13
    call parse_cents
    test rax, rax
    js .Lstart_bad
    add rdx, r14
    lea rax, [rdx + 1]
    lea rcx, [rdx - 1]
    test rdx, rdx
    cmovs rax, rcx
    cqo
    mov ecx, 2
    idiv rcx
    mov rdi, rax
    call print_cents
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_a_bad:
    mov r13, r12
.Lstart_bad:
    mov r14, rax
    mov rdi, rax
    mov rsi, r13
    call report_bad_number
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OUT_OF_RANGE
    cmp r14, ERROR_RANGE
    je .Lstart_exit
    mov edi, EXIT_NOT_A_NUMBER
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
.Lstart_assert_argv_a:
    mov edi, 1
    call assert_fail
.Lstart_assert_argv_b:
    mov edi, 2
    call assert_fail
.size _start, . - _start

# Reports an argument that isn't a number, or is out of range, on standard error.
# In: rdi = ERROR_INVALID or ERROR_RANGE, rsi = the NUL-terminated argument.
# Out: rax = 0, or a negative error code.
# returns: result
.type report_bad_number, @function
report_bad_number:
    sub rsp, 8
    test rsi, rsi
    jz .Lreport_bad_assert_null
    mov rdx, rsi
    cmp rdi, ERROR_INVALID
    je .Lreport_bad_not_a_number
    cmp rdi, ERROR_RANGE
    je .Lreport_bad_out_of_range
    jmp .Lreport_bad_assert_error
.Lreport_bad_not_a_number:
    lea rdi, [rip + nan_msg]
    mov esi, NAN_MSG_LEN
    jmp .Lreport_bad_write
.Lreport_bad_out_of_range:
    lea rdi, [rip + range_msg]
    mov esi, RANGE_MSG_LEN
.Lreport_bad_write:
    call report_error
    test rax, rax
    js .Lreport_bad_return
    xor eax, eax
.Lreport_bad_return:
    add rsp, 8
    ret
.Lreport_bad_assert_null:
    mov edi, 3
    call assert_fail
.Lreport_bad_assert_error:
    mov edi, 18
    call assert_fail
.size report_bad_number, . - report_bad_number

# Parses a decimal number with an optional leading minus sign, one or more digits, and
# optionally a point followed by one or two digits.
# In: rdi = a NUL-terminated string.
# Out: rax = 0 and rdx = the number in cents, or rax = ERROR_INVALID when the text isn't such a
# number, or ERROR_RANGE when it is one beyond -1000000.00 to 1000000.00.
# returns: result
.type parse_cents, @function
parse_cents:
    sub rsp, 8
    test rdi, rdi
    jz .Lparse_assert_null
    xor r9d, r9d
    cmp byte ptr [rdi], '-'
    jne .Lparse_whole
    mov r9d, 1
    inc rdi
.Lparse_whole:
    xor eax, eax
    xor ecx, ecx
    mov r8, MAX_ARG_LEN
.Lparse_loop:  # bound: 131072
    cmp rcx, MAX_ARG_LEN
    jae .Lparse_assert_index
    movzx edx, byte ptr [rdi + rcx]
    sub edx, '0'
    cmp edx, 9
    ja .Lparse_whole_end
    cmp rax, MAX_WHOLE
    ja .Lparse_next
    imul rax, rax, 10
    add rax, rdx
.Lparse_next:
    inc rcx
    dec r8
    jnz .Lparse_loop
    jmp .Lparse_invalid
.Lparse_whole_end:
    test rcx, rcx
    jz .Lparse_invalid
    imul rax, rax, CENTS_PER_UNIT
    mov r10, rax
    add rdi, rcx
    call parse_fraction
    test rax, rax
    js .Lparse_return
    lea rax, [r10 + rdx]
.Lparse_check_range:
    cmp rax, MAX_CENTS
    ja .Lparse_out_of_range
    test r9d, r9d
    jz .Lparse_positive
    neg rax
.Lparse_positive:
    mov rdx, rax
    xor eax, eax
    jmp .Lparse_return
.Lparse_out_of_range:
    mov rax, ERROR_RANGE
    jmp .Lparse_return
.Lparse_invalid:
    mov rax, ERROR_INVALID
.Lparse_return:
    add rsp, 8
    ret
.Lparse_assert_null:
    mov edi, 4
    call assert_fail
.Lparse_assert_index:
    mov edi, 5
    call assert_fail
.size parse_cents, . - parse_cents

# Parses what follows the whole digits of a number: nothing, or a point and one or two digits.
# In: rdi = the rest of a NUL-terminated string, starting at a byte that isn't a digit.
# Out: rax = 0 and rdx = the fraction in cents, or rax = ERROR_INVALID.
# returns: result
.type parse_fraction, @function
parse_fraction:
    test rdi, rdi
    jz .Lparse_fraction_assert_null
    xor eax, eax
    xor ecx, ecx
    movzx edx, byte ptr [rdi]
    cmp edx, '0'
    jb .Lparse_fraction_not_digit
    cmp edx, '9'
    jbe .Lparse_fraction_assert_digit
.Lparse_fraction_not_digit:
    test edx, edx
    jz .Lparse_fraction_done
    cmp edx, '.'
    jne .Lparse_fraction_invalid
    movzx edx, byte ptr [rdi + 1]
    sub edx, '0'
    cmp edx, 9
    ja .Lparse_fraction_invalid
    imul ecx, edx, 10
    movzx edx, byte ptr [rdi + 2]
    test edx, edx
    jz .Lparse_fraction_done
    sub edx, '0'
    cmp edx, 9
    ja .Lparse_fraction_invalid
    add ecx, edx
    movzx edx, byte ptr [rdi + 3]
    test edx, edx
    jnz .Lparse_fraction_invalid
.Lparse_fraction_done:
    mov edx, ecx
    jmp .Lparse_fraction_return
.Lparse_fraction_invalid:
    mov rax, ERROR_INVALID
.Lparse_fraction_return:
    ret
.Lparse_fraction_assert_null:
    mov edi, 19
    call assert_fail
.Lparse_fraction_assert_digit:
    mov edi, 20
    call assert_fail
.size parse_fraction, . - parse_fraction

# Writes a number of cents as a decimal with two places, then a newline, to standard output.
# In: rdi = the cents, never -9223372036854775808.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_cents, @function
print_cents:
    sub rsp, 8
    mov rax, rdi
    mov r9, rdi
    test rax, rax
    jns .Lprint_cents_digits
    neg rax
    jo .Lprint_cents_assert_min
.Lprint_cents_digits:
    lea rsi, [rip + out_buf]
    mov byte ptr [rip + out_buf + OUT_BUF_LEN - 1], NEWLINE
    mov r10d, 10
    xor edx, edx
    div r10
    add edx, '0'
    mov byte ptr [rip + out_buf + OUT_BUF_LEN - 2], dl
    xor edx, edx
    div r10
    add edx, '0'
    mov byte ptr [rip + out_buf + OUT_BUF_LEN - 3], dl
    mov byte ptr [rip + out_buf + OUT_BUF_LEN - 4], '.'
    mov ecx, OUT_BUF_LEN - 4
    mov r8, MAX_DIGITS
.Lprint_cents_digit:  # bound: 19
    xor edx, edx
    div r10
    add edx, '0'
    dec ecx
    cmp ecx, OUT_BUF_LEN
    jae .Lprint_cents_assert_digit_index
    mov byte ptr [rsi + rcx], dl
    test rax, rax
    jz .Lprint_cents_sign
    dec r8
    jnz .Lprint_cents_digit
    jmp .Lprint_cents_assert_digits
.Lprint_cents_sign:
    test r9, r9
    jns .Lprint_cents_output
    dec ecx
    cmp ecx, OUT_BUF_LEN
    jae .Lprint_cents_assert_sign_index
    mov byte ptr [rsi + rcx], '-'
.Lprint_cents_output:
    add rsi, rcx
    mov edx, OUT_BUF_LEN
    sub edx, ecx
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lprint_cents_return
    xor eax, eax
.Lprint_cents_return:
    add rsp, 8
    ret
.Lprint_cents_assert_min:
    mov edi, 6
    call assert_fail
.Lprint_cents_assert_digit_index:
    mov edi, 7
    call assert_fail
.Lprint_cents_assert_digits:
    mov edi, 8
    call assert_fail
.Lprint_cents_assert_sign_index:
    mov edi, 9
    call assert_fail
.size print_cents, . - print_cents

# Writes a message, then an argument, then a newline, to standard error.
# In: rdi = the message, esi = its length, at least 1, rdx = a NUL-terminated argument.
# Out: rax = 0, or a negative error code.
# returns: result
.type report_error, @function
report_error:
    push r12
    test rdi, rdi
    jz .Lreport_error_assert_message
    test rdx, rdx
    jz .Lreport_error_assert_argument
    mov r12, rdx
    mov edx, esi
    mov rsi, rdi
    mov edi, STDERR
    call write_all
    test rax, rax
    js .Lreport_error_return
    mov rdi, r12
    call str_len
    test rax, rax
    jz .Lreport_error_newline
    mov rdx, rax
    mov rsi, r12
    mov edi, STDERR
    call write_all
    test rax, rax
    js .Lreport_error_return
.Lreport_error_newline:
    mov edi, STDERR
    lea rsi, [rip + newline_msg]
    mov edx, NEWLINE_MSG_LEN
    call write_all
    test rax, rax
    js .Lreport_error_return
    xor eax, eax
.Lreport_error_return:
    pop r12
    ret
.Lreport_error_assert_message:
    mov edi, 10
    call assert_fail
.Lreport_error_assert_argument:
    mov edi, 11
    call assert_fail
.size report_error, . - report_error

# Counts the bytes of a NUL-terminated string.
# In: rdi = the string, shorter than MAX_ARG_LEN bytes.
# Out: rax = its length.
.type str_len, @function
str_len:
    test rdi, rdi
    jz .Lstr_len_assert_null
    xor eax, eax
    mov rcx, MAX_ARG_LEN
.Lstr_len_loop:  # bound: 131072
    cmp rax, MAX_ARG_LEN
    jae .Lstr_len_assert_index
    cmp byte ptr [rdi + rax], 0
    je .Lstr_len_return
    inc rax
    dec rcx
    jnz .Lstr_len_loop
    jmp .Lstr_len_assert_long
.Lstr_len_return:
    ret
.Lstr_len_assert_null:
    mov edi, 12
    call assert_fail
.Lstr_len_assert_index:
    mov edi, 13
    call assert_fail
.Lstr_len_assert_long:
    mov edi, 14
    call assert_fail
.size str_len, . - str_len

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
    mov edi, 15
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 16
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 17
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
