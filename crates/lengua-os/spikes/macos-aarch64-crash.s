// Spike: a macOS ARM64 program that writes to address 0, to see how long the system takes to
// report its end and which signal it ends by.
.globl _main
.text
.p2align 2
_main:
    mov x0, #0
    str x0, [x0]
    brk #0
