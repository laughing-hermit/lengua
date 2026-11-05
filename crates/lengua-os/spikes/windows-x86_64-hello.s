# Spike: the smallest Windows x86-64 program, calling kernel32 through the import address table.
# The loader enters _start as a thread start (rsp is 8 mod 16), and Windows's convention wants 32
# bytes of shadow space and the fifth argument above it.
.intel_syntax noprefix
.globl _start
.text
_start:
    sub rsp, 56
    mov ecx, -11
    call qword ptr [rip + __imp_GetStdHandle]
    mov rcx, rax
    lea rdx, [rip + message]
    mov r8d, 6
    lea r9, [rsp + 40]
    mov qword ptr [rsp + 32], 0
    call qword ptr [rip + __imp_WriteFile]
    xor ecx, ecx
    call qword ptr [rip + __imp_ExitProcess]
.section .rdata,"dr"
message:
    .asciz "hello\n"
