.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_READ, 0
.equ SYS_CLOSE, 3
.equ SYS_SENDTO, 44
.equ SYS_ACCEPT4, 288
# File descriptors: lengua hands a server its listening socket as 3.
.equ LISTENING, 3
# Socket flags: no SIGPIPE when a client hangs up early.
.equ MSG_NOSIGNAL, 0x4000
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# Sizes and limits
.equ LINE_MAX, 1024
.equ MAX_READS, 1024
.equ NEWLINE, 10

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# One connection's line.
.type line, @object
line:
    .zero LINE_MAX
.size line, . - line

.text
.globl _start

# Takes connections on the listening socket for as long as it runs, echoing one line on each.
.type _start, @function
_start:
.Lstart_serve:  # bound: forever
    mov eax, SYS_ACCEPT4
    mov edi, LISTENING
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    syscall
    test rax, rax
    js .Lstart_next
    cmp rax, LISTENING
    jbe .Lstart_assert_connection
    mov rbx, rax
    mov edi, ebx
    call echo_line
    test rax, rax
    jg .Lstart_assert_result
    mov eax, SYS_CLOSE
    mov edi, ebx
    syscall
    test rax, rax
    js .Lstart_next
.Lstart_next:
    jmp .Lstart_serve
.Lstart_assert_connection:
    mov edi, 1
    call assert_fail
.Lstart_assert_result:
    mov edi, 2
    call assert_fail
.size _start, . - _start

# Reads a line from a connection, until it ends with a newline, the client closes its side, or
# it fills the buffer, and sends it back.
# In: edi = the connection.
# Out: rax = 0, or a negative error code.
# returns: result
.type echo_line, @function
echo_line:
    mov r9d, edi
    xor r8d, r8d
    mov r10, MAX_READS
.Lecho_line_read:  # bound: 1024
    cmp r8, LINE_MAX
    jae .Lecho_line_send
    mov eax, SYS_READ
    mov edi, r9d
    lea rsi, [rip + line]
    add rsi, r8
    mov edx, LINE_MAX
    sub edx, r8d
    syscall
    test rax, rax
    js .Lecho_line_return
    jz .Lecho_line_send
    add r8, rax
    cmp r8, LINE_MAX
    ja .Lecho_line_assert_count
    lea rdx, [r8 - 1]
    cmp rdx, LINE_MAX
    jae .Lecho_line_assert_index
    lea rcx, [rip + line]
    movzx eax, byte ptr [rcx + rdx]
    cmp eax, NEWLINE
    je .Lecho_line_send
    dec r10
    jnz .Lecho_line_read
.Lecho_line_send:
    xor eax, eax
    test r8, r8
    jz .Lecho_line_return
    mov eax, SYS_SENDTO
    mov edi, r9d
    lea rsi, [rip + line]
    mov rdx, r8
    mov r10d, MSG_NOSIGNAL
    xor r8d, r8d
    xor r9d, r9d
    syscall
    test rax, rax
    js .Lecho_line_return
    cmp rax, rdx
    jne .Lecho_line_short
    xor eax, eax
    jmp .Lecho_line_return
.Lecho_line_short:
    mov rax, ERROR_IO
.Lecho_line_return:
    ret
.Lecho_line_assert_count:
    mov edi, 3
    call assert_fail
.Lecho_line_assert_index:
    mov edi, 4
    call assert_fail
.size echo_line, . - echo_line

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
