# lengua: spec sha256:5b8172f199719bb1a1cd8f481522466b8a47cd0b54a0dcd2eba9033019a6d879
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
.equ MAX_ARG_LEN, 131072
.equ MAX_PLACES, 3
.equ MAX_WHOLE, 1000000
.equ UNITS, 1000
.equ MAX_UNITS, 1000000000
.equ MAX_WRITES, 64
.equ MAX_INT_DIGITS, 16
.equ NEWLINE, 10
.equ USAGE_MSG_LEN, 26
.equ NAN_MSG_LEN, 28
.equ RANGE_MSG_LEN, 25
.equ OUT_BUF_LEN, 21

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: hypotenuse <a> <b>\n"
.size usage_msg, . - usage_msg

.type nan_msg, @object
nan_msg:
    .ascii "error: side is not a number\n"
.size nan_msg, . - nan_msg

.type range_msg, @object
range_msg:
    .ascii "error: side out of range\n"
.size range_msg, . - range_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# Up to 16 whole digits, a point, 3 decimals and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

# Checks the argument count, parses both sides, prints the hypotenuse and exits.
.type _start, @function
_start:
    mov rax, qword ptr [rsp]
    cmp rax, ARGC_EXPECTED
    jne .Lstart_usage
    mov rdi, qword ptr [rsp + 16]
    test rdi, rdi
    jz .Lstart_assert_argv1
    call parse_side
    test rax, rax
    js .Lstart_bad_side
    mov rbx, rdx
    mov rdi, qword ptr [rsp + 24]
    test rdi, rdi
    jz .Lstart_assert_argv2
    call parse_side
    test rax, rax
    js .Lstart_bad_side
    mov rdi, rbx
    mov rsi, rdx
    call print_hypotenuse
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_bad_side:
    cmp rax, ERROR_INVALID
    je .Lstart_not_a_number
    cmp rax, ERROR_RANGE
    je .Lstart_out_of_range
    jmp .Lstart_assert_error
.Lstart_usage:
    lea rdi, [rip + usage_msg]
    mov esi, USAGE_MSG_LEN
    call write_error
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_USAGE
    jmp .Lstart_exit
.Lstart_not_a_number:
    lea rdi, [rip + nan_msg]
    mov esi, NAN_MSG_LEN
    call write_error
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_NOT_A_NUMBER
    jmp .Lstart_exit
.Lstart_out_of_range:
    lea rdi, [rip + range_msg]
    mov esi, RANGE_MSG_LEN
    call write_error
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OUT_OF_RANGE
    jmp .Lstart_exit
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_argv1:
    mov edi, 1
    call assert_fail
.Lstart_assert_argv2:
    mov edi, 2
    call assert_fail
.Lstart_assert_error:
    mov edi, 3
    call assert_fail
.size _start, . - _start

# Parses a side: digits, optionally a point and 1 to 3 more digits.
# In: rdi = a NUL-terminated string.
# Out: rax = 0 and rdx = the side in thousandths, from 1 to MAX_UNITS; or rax = ERROR_INVALID
# when the text isn't a number, or ERROR_RANGE when the number is out of range.
# returns: result
.type parse_side, @function
parse_side:
    sub rsp, 8
    test rdi, rdi
    jz .Lparse_assert_null
    xor eax, eax
    xor r9d, r9d
    xor r10d, r10d
    xor r11d, r11d
    xor esi, esi
    xor ecx, ecx
    mov r8, MAX_ARG_LEN
.Lparse_loop:  # bound: 131072
    cmp rcx, MAX_ARG_LEN
    jae .Lparse_assert_index
    movzx edx, byte ptr [rdi + rcx]
    test edx, edx
    jz .Lparse_end
    inc rcx
    cmp edx, '.'
    je .Lparse_point
    sub edx, '0'
    cmp edx, 9
    ja .Lparse_invalid
    test esi, esi
    jnz .Lparse_fraction
    inc r10
    cmp rax, MAX_WHOLE
    ja .Lparse_next
    imul rax, rax, 10
    add rax, rdx
    jmp .Lparse_next
.Lparse_fraction:
    inc r11
    cmp r11, MAX_PLACES
    ja .Lparse_invalid
    imul r9, r9, 10
    add r9, rdx
    jmp .Lparse_next
.Lparse_point:
    test esi, esi
    jnz .Lparse_invalid
    mov esi, 1
.Lparse_next:
    dec r8
    jnz .Lparse_loop
    jmp .Lparse_assert_length
.Lparse_end:
    mov r8, rsi
    mov rsi, r9
    mov rdx, r11
    mov rcx, r10
    mov rdi, rax
    call to_units
    test rax, rax
    js .Lparse_return
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
.Lparse_assert_length:
    mov edi, 6
    call assert_fail
.size parse_side, . - parse_side

# Prints the hypotenuse of two sides with 3 decimals, then a newline, to standard output.
# In: rdi, rsi = the sides in thousandths, from 1 to MAX_UNITS.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_hypotenuse, @function
print_hypotenuse:
    sub rsp, 8
    test rdi, rdi
    jz .Lprint_assert_side_a
    test rsi, rsi
    jz .Lprint_assert_side_b
    mov eax, UNITS
    cvtsi2sd xmm2, rax
    cvtsi2sd xmm0, rdi
    divsd xmm0, xmm2
    cvtsi2sd xmm1, rsi
    divsd xmm1, xmm2
    mulsd xmm0, xmm0
    mulsd xmm1, xmm1
    addsd xmm0, xmm1
    sqrtsd xmm0, xmm0
    mulsd xmm0, xmm2
    cvtsd2si rdi, xmm0
    call write_fixed3
    test rax, rax
    js .Lprint_return
    xor eax, eax
.Lprint_return:
    add rsp, 8
    ret
.Lprint_assert_side_a:
    mov edi, 15
    call assert_fail
.Lprint_assert_side_b:
    mov edi, 18
    call assert_fail
.size print_hypotenuse, . - print_hypotenuse

# Checks a parsed side's shape and turns it into thousandths.
# In: rdi = the whole part (above MAX_WHOLE when larger), rsi = the decimals' value,
# rdx = how many decimals, rcx = how many whole digits, r8 = 1 when there was a point.
# Out: rax = 0 and rdx = the side in thousandths, from 1 to MAX_UNITS; or rax = ERROR_INVALID
# or ERROR_RANGE.
# returns: result
.type to_units, @function
to_units:
    cmp rdx, MAX_PLACES
    ja .Lunits_assert_places
    test rcx, rcx
    jz .Lunits_invalid
    test r8, r8
    jz .Lunits_scale
    test rdx, rdx
    jz .Lunits_invalid
.Lunits_scale:
    mov r9, MAX_PLACES
.Lunits_pad:  # bound: 3
    cmp rdx, MAX_PLACES
    jae .Lunits_padded
    imul rsi, rsi, 10
    inc rdx
    dec r9
    jnz .Lunits_pad
.Lunits_padded:
    cmp rsi, UNITS
    jae .Lunits_assert_fraction
    cmp rdi, MAX_WHOLE
    ja .Lunits_range
    imul rax, rdi, UNITS
    add rax, rsi
    jz .Lunits_range
    cmp rax, MAX_UNITS
    ja .Lunits_range
    mov rdx, rax
    xor eax, eax
    jmp .Lunits_return
.Lunits_range:
    mov rax, ERROR_RANGE
    jmp .Lunits_return
.Lunits_invalid:
    mov rax, ERROR_INVALID
.Lunits_return:
    ret
.Lunits_assert_places:
    mov edi, 16
    call assert_fail
.Lunits_assert_fraction:
    mov edi, 17
    call assert_fail
.size to_units, . - to_units

# Writes a number of thousandths with 3 decimals, then a newline, to standard output.
# In: rdi = the number, at least 0.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_fixed3, @function
write_fixed3:
    sub rsp, 8
    test rdi, rdi
    js .Lwrite_fixed3_assert_negative
    mov rax, rdi
    mov byte ptr [rip + out_buf + OUT_BUF_LEN - 1], NEWLINE
    lea rsi, [rip + out_buf]
    mov ecx, OUT_BUF_LEN - 1
    mov r10d, 10
    mov r8, MAX_PLACES
.Lwrite_fixed3_fraction:  # bound: 3
    xor edx, edx
    div r10
    add edx, '0'
    dec ecx
    cmp ecx, OUT_BUF_LEN
    jae .Lwrite_fixed3_assert_fraction_index
    mov byte ptr [rsi + rcx], dl
    dec r8
    jnz .Lwrite_fixed3_fraction
    dec ecx
    cmp ecx, OUT_BUF_LEN
    jae .Lwrite_fixed3_assert_point_index
    mov byte ptr [rsi + rcx], '.'
    mov r8, MAX_INT_DIGITS
.Lwrite_fixed3_digit:  # bound: 16
    xor edx, edx
    div r10
    add edx, '0'
    dec ecx
    cmp ecx, OUT_BUF_LEN
    jae .Lwrite_fixed3_assert_digit_index
    mov byte ptr [rsi + rcx], dl
    test rax, rax
    jz .Lwrite_fixed3_output
    dec r8
    jnz .Lwrite_fixed3_digit
    jmp .Lwrite_fixed3_assert_digits
.Lwrite_fixed3_output:
    add rsi, rcx
    mov edx, OUT_BUF_LEN
    sub edx, ecx
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lwrite_fixed3_return
    xor eax, eax
.Lwrite_fixed3_return:
    add rsp, 8
    ret
.Lwrite_fixed3_assert_negative:
    mov edi, 7
    call assert_fail
.Lwrite_fixed3_assert_fraction_index:
    mov edi, 8
    call assert_fail
.Lwrite_fixed3_assert_point_index:
    mov edi, 9
    call assert_fail
.Lwrite_fixed3_assert_digit_index:
    mov edi, 10
    call assert_fail
.Lwrite_fixed3_assert_digits:
    mov edi, 11
    call assert_fail
.size write_fixed3, . - write_fixed3

# Writes an error message to standard error.
# In: rdi = the message, rsi = its length, at least 1.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_error, @function
write_error:
    sub rsp, 8
    test rdi, rdi
    jz .Lwrite_error_assert_null
    test rsi, rsi
    jz .Lwrite_error_assert_empty
    mov rdx, rsi
    mov rsi, rdi
    mov edi, STDERR
    call write_all
    test rax, rax
    js .Lwrite_error_return
    xor eax, eax
.Lwrite_error_return:
    add rsp, 8
    ret
.Lwrite_error_assert_null:
    mov edi, 19
    call assert_fail
.Lwrite_error_assert_empty:
    mov edi, 20
    call assert_fail
.size write_error, . - write_error

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
    mov edi, 12
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 13
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 14
    call assert_fail
.size write_all, . - write_all

# lengua's assertion handler. check_code compares it word for word: copy it exactly.
# In: edi = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
# standard error and exits with code 134.
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
    mov eax, 60
    mov edi, 134
    syscall
.size assert_fail, . - assert_fail
