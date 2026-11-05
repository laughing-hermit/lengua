# Spike: a macOS x86-64 program that writes to address 0, to see how long the system takes to
# report its end and which signal it ends by.
.intel_syntax noprefix
.globl _main
.text
.p2align 4
_main:
    xor eax, eax
    mov dword ptr [rax], 1
    ud2
