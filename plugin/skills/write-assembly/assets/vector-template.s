# lengua: spec sha256:79a7a51052c6de2bcf381b92071059b7ee7e67e6c52cc4113eda18f40787e3d5
.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_READ, 0
.equ SYS_WRITE, 1
.equ SYS_EXIT, 60
# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
.equ STDERR, 2
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_NO_INPUT, 1
.equ EXIT_TOO_LONG, 2
.equ EXIT_READ_FAILED, 3
.equ EXIT_WRITE_FAILED, 4
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_TOO_LONG, -7
# What find_newline returns when there's no newline
.equ NO_NEWLINE, -1
# Sizes and limits
.equ CHUNK, 16
.equ MAX_LINE, 64
.equ READ_MAX, MAX_LINE + 1
.equ MAX_READS, READ_MAX
.equ LINE_CHUNKS, 5
.equ LINE_BUF_LEN, LINE_CHUNKS * CHUNK
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ NO_INPUT_MSG_LEN, 16
.equ TOO_LONG_MSG_LEN, 21
# Bytes repeated four times, spread over a vector register with pshufd
.equ NEWLINES_X4, 0x0a0a0a0a
.equ BELOW_A_X4, 0x60606060
.equ ABOVE_Z_X4, 0x7b7b7b7b
.equ CASE_BIT_X4, 0x20202020

.section .rodata

.type no_input_msg, @object
no_input_msg:
    .ascii "error: no input\n"
.size no_input_msg, . - no_input_msg

.type too_long_msg, @object
too_long_msg:
    .ascii "error: line too long\n"
.size too_long_msg, . - too_long_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# READ_MAX bytes of input, rounded up to whole 16-byte chunks so vector loads stay inside.
# shared: read_line fills it, find_newline and shout work on it, and _start prints it
.type line_buf, @object
line_buf:
    .zero LINE_BUF_LEN
.size line_buf, . - line_buf

.text
.globl _start

# Reads a line, prints it in capital letters and exits.
.type _start, @function
_start:
    call read_line
    test rax, rax
    js .Lstart_read_failed
    jz .Lstart_no_input
    cmp rax, MAX_LINE
    ja .Lstart_assert_length
    mov rbx, rax
    mov rdi, rbx
    call shout
    lea rsi, [rip + line_buf]
    cmp rbx, LINE_BUF_LEN
    jae .Lstart_assert_index
    mov byte ptr [rsi + rbx], NEWLINE
    mov edi, STDOUT
    lea rdx, [rbx + 1]
    call write_all
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_read_failed:
    cmp rax, ERROR_TOO_LONG
    je .Lstart_too_long
    mov edi, EXIT_READ_FAILED
    jmp .Lstart_exit
.Lstart_too_long:
    mov edi, STDERR
    lea rsi, [rip + too_long_msg]
    mov edx, TOO_LONG_MSG_LEN
    call write_all
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_TOO_LONG
    jmp .Lstart_exit
.Lstart_no_input:
    mov edi, STDERR
    lea rsi, [rip + no_input_msg]
    mov edx, NO_INPUT_MSG_LEN
    call write_all
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_NO_INPUT
    jmp .Lstart_exit
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_length:
    mov edi, 1
    call assert_fail
.Lstart_assert_index:
    mov edi, 2
    call assert_fail
.size _start, . - _start

# Reads standard input into line_buf until a newline arrives, the input ends or READ_MAX bytes
# are in.
# Out: rax = the line's length without its newline, ERROR_TOO_LONG when the line is longer than
# MAX_LINE, or ERROR_IO when reading fails.
# returns: result
.type read_line, @function
read_line:
    push rbx
    push r12
    sub rsp, 8
    xor ebx, ebx
    mov r12, MAX_READS
.Lread_loop:  # bound: 65
    cmp rbx, READ_MAX
    jae .Lread_assert_full
    mov edi, STDIN
    lea rsi, [rip + line_buf]
    add rsi, rbx
    mov edx, READ_MAX
    sub rdx, rbx
    mov eax, SYS_READ
    syscall
    test rax, rax
    js .Lread_failed
    jz .Lread_ended
    add rbx, rax
    call find_newline
    test rax, rax
    jns .Lread_found
    cmp rbx, READ_MAX
    jae .Lread_too_long
    dec r12
    jnz .Lread_loop
    jmp .Lread_assert_reads
.Lread_ended:
    cmp rbx, MAX_LINE
    ja .Lread_too_long
    mov rax, rbx
    jmp .Lread_return
.Lread_found:
    cmp rax, MAX_LINE
    ja .Lread_too_long
    jmp .Lread_return
.Lread_too_long:
    mov rax, ERROR_TOO_LONG
    jmp .Lread_return
.Lread_failed:
    mov rax, ERROR_IO
.Lread_return:
    add rsp, 8
    pop r12
    pop rbx
    ret
.Lread_assert_full:
    mov edi, 3
    call assert_fail
.Lread_assert_reads:
    mov edi, 4
    call assert_fail
.size read_line, . - read_line

# Finds the first newline in line_buf, 16 bytes at a time. Bytes not read yet are zero.
# Out: rax = its index, or NO_NEWLINE when there's none.
# returns: result
.type find_newline, @function
find_newline:
    lea rsi, [rip + line_buf]
    mov eax, NEWLINES_X4
    movd xmm1, eax
    pshufd xmm1, xmm1, 0
    xor ecx, ecx
    mov r8, LINE_CHUNKS
.Lfind_loop:  # bound: 5
    cmp rcx, LINE_BUF_LEN - CHUNK + 1
    jae .Lfind_assert_index
    movdqu xmm0, xmmword ptr [rsi + rcx]
    pcmpeqb xmm0, xmm1
    pmovmskb eax, xmm0
    test eax, eax
    jnz .Lfind_found
    add rcx, CHUNK
    dec r8
    jnz .Lfind_loop
    mov rax, NO_NEWLINE
    jmp .Lfind_return
.Lfind_found:
    bsf eax, eax
    add rax, rcx
    cmp rax, LINE_BUF_LEN
    jae .Lfind_assert_found
.Lfind_return:
    ret
.Lfind_assert_index:
    mov edi, 5
    call assert_fail
.Lfind_assert_found:
    mov edi, 6
    call assert_fail
.size find_newline, . - find_newline

# Capitalizes the line in line_buf, 16 bytes at a time: a to z become A to Z, other bytes stay.
# The last chunk may change bytes past the line, which is harmless: only the line is printed.
# In: rdi = the line's length, at most MAX_LINE.
.type shout, @function
shout:
    cmp rdi, MAX_LINE
    ja .Lshout_assert_length
    lea rsi, [rip + line_buf]
    mov eax, BELOW_A_X4
    movd xmm1, eax
    pshufd xmm1, xmm1, 0
    mov eax, ABOVE_Z_X4
    movd xmm2, eax
    pshufd xmm2, xmm2, 0
    mov eax, CASE_BIT_X4
    movd xmm3, eax
    pshufd xmm3, xmm3, 0
    xor ecx, ecx
    mov r8, LINE_CHUNKS
.Lshout_loop:  # bound: 5
    cmp rcx, rdi
    jae .Lshout_return
    cmp rcx, LINE_BUF_LEN - CHUNK + 1
    jae .Lshout_assert_index
    movdqu xmm0, xmmword ptr [rsi + rcx]
    movdqu xmm4, xmm0
    pcmpgtb xmm4, xmm1
    movdqu xmm5, xmm2
    pcmpgtb xmm5, xmm0
    pand xmm4, xmm5
    pand xmm4, xmm3
    psubb xmm0, xmm4
    movdqu xmmword ptr [rsi + rcx], xmm0
    add rcx, CHUNK
    dec r8
    jnz .Lshout_loop
.Lshout_return:
    ret
.Lshout_assert_length:
    mov edi, 7
    call assert_fail
.Lshout_assert_index:
    mov edi, 8
    call assert_fail
.size shout, . - shout

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
