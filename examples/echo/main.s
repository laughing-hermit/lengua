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
.equ NEWLINE, 10
# fill's limit on reads, as assets/fill.s has it
.equ MAX_FILL_READS, 1099511627776

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
    push rbx
    mov ebx, edi
    lea rsi, [rip + line]
    mov edx, LINE_MAX
    mov ecx, NEWLINE
    call fill
    test rax, rax
    jle .Lecho_line_return
    cmp rax, LINE_MAX
    ja .Lecho_line_assert_count
    mov rdx, rax
    mov eax, SYS_SENDTO
    mov edi, ebx
    lea rsi, [rip + line]
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
    pop rbx
    ret
.Lecho_line_assert_count:
    mov edi, 3
    call assert_fail
.size echo_line, . - echo_line

# Reads from file descriptor edi into the rdx bytes at rsi until they're full, the input ends,
# or, when ecx is a byte (0 to 255), the last byte read is ecx; with ecx = -1 it reads to the
# end. However the input arrives, it reads the same.
# In: edi = the file descriptor, rsi = the buffer, rdx = its size, at least 1, ecx = the end
# byte or -1.
# Out: rax = how many bytes it read, or a negative error code.
# returns: result
.type fill, @function
fill:
    test rsi, rsi
    jz .Lfill_assert_null
    test rdx, rdx
    jz .Lfill_assert_empty
    mov r8, rdx
    mov r10d, ecx
    mov r9, MAX_FILL_READS
.Lfill_loop:  # bound: 1099511627776
    mov eax, SYS_READ
    syscall
    test rax, rax
    js .Lfill_return
    jz .Lfill_done
    cmp rax, rdx
    ja .Lfill_assert_count
    add rsi, rax
    sub rdx, rax
    jz .Lfill_done
    movzx eax, byte ptr [rsi - 1]
    cmp eax, r10d
    je .Lfill_done
    dec r9
    jnz .Lfill_loop
    mov rax, ERROR_IO
    jmp .Lfill_return
.Lfill_done:
    mov rax, r8
    sub rax, rdx
.Lfill_return:
    ret
.Lfill_assert_null:
    mov edi, 990
    call assert_fail
.Lfill_assert_empty:
    mov edi, 991
    call assert_fail
.Lfill_assert_count:
    mov edi, 992
    call assert_fail
.size fill, . - fill

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
