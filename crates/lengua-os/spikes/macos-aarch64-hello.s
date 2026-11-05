// Spike: the smallest macOS ARM64 program, calling libSystem through the GOT. dyld enters _main as
// main(argc, argv, envp, apple).
.globl _main
.text
.p2align 2
_main:
    mov x0, #1
    adrp x1, message@PAGE
    add x1, x1, message@PAGEOFF
    mov x2, #6
    adrp x16, _write@GOTPAGE
    ldr x16, [x16, _write@GOTPAGEOFF]
    blr x16
    mov x0, #0
    adrp x16, __exit@GOTPAGE
    ldr x16, [x16, __exit@GOTPAGEOFF]
    blr x16
.section __TEXT,__cstring,cstring_literals
message:
    .asciz "hello\n"
