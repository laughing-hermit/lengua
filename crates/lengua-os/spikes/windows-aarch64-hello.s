// Spike: the smallest Windows ARM64 program, calling kernel32 through the import address table.
.globl _start
.text
_start:
    sub sp, sp, #32
    mov x0, #-11
    adrp x8, __imp_GetStdHandle
    ldr x8, [x8, :lo12:__imp_GetStdHandle]
    blr x8
    adrp x1, message
    add x1, x1, :lo12:message
    mov w2, #6
    add x3, sp, #16
    mov x4, #0
    adrp x8, __imp_WriteFile
    ldr x8, [x8, :lo12:__imp_WriteFile]
    blr x8
    mov w0, #0
    adrp x8, __imp_ExitProcess
    ldr x8, [x8, :lo12:__imp_ExitProcess]
    blr x8
.section .rdata,"dr"
message:
    .asciz "hello\n"
