.intel_syntax noprefix

# File descriptors: lengua hands a server its listening socket as 3.
.equ LISTENING, 3
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# Sizes and limits
.equ LINE_MAX, 1024
.equ NEWLINE, 10

.section .bss

# One connection's line.
.type line, @object
line:
    .zero LINE_MAX
.size line, . - line

.text
# Takes connections on the listening socket for as long as it runs, echoing one line on each.
.type main, @function
main:
    sub rsp, 8
.Lmain_serve:  # bound: forever
    mov edi, LISTENING
    call os_accept
    test rax, rax
    js .Lmain_next
    cmp rax, LISTENING
    jbe .Lmain_assert_connection
    mov rbx, rax
    mov edi, ebx
    call echo_line
    test rax, rax
    jg .Lmain_assert_result
    mov edi, ebx
    call os_close
    test rax, rax
    js .Lmain_next
.Lmain_next:
    jmp .Lmain_serve
.Lmain_assert_connection:
    mov edi, 1
    call assert_fail
.Lmain_assert_result:
    mov edi, 2
    call assert_fail
.size main, . - main

# Reads a line from a connection, until it ends with a newline, the client closes its side, or
# it fills the buffer, and sends it back.
# In: edi = the connection.
# Out: rax = 0, or a negative error code.
# returns: result
.type echo_line, @function
echo_line:
    push rbx
    cmp edi, LISTENING
    jbe .Lecho_line_assert_connection
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
    mov edi, ebx
    lea rsi, [rip + line]
    call os_write
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
.Lecho_line_assert_connection:
    mov edi, 4
    call assert_fail
.size echo_line, . - echo_line

