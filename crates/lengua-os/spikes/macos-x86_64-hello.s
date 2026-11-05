# Spike: the smallest macOS x86-64 program, calling libSystem through the GOT. dyld enters _main as
# main(argc, argv, envp, apple).
.intel_syntax noprefix
.globl _main
.text
.p2align 4
_main:
    push rbp
    mov rbp, rsp
    mov edi, 1
    lea rsi, [rip + message]
    mov edx, 6
    call qword ptr [rip + _write@GOTPCREL]
    xor edi, edi
    call qword ptr [rip + __exit@GOTPCREL]
.section __TEXT,__cstring,cstring_literals
message:
    .asciz "hello\n"
