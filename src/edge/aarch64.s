// The AArch64 library: functions a program may call without defining them, written once for every
// operating system because they reach the OS only through the operations (os_write, os_exit,
// ...). src/edge/mod.rs says how this file is read: a line `// == name: needs` starts a snippet,
// which is written when main.s calls `name` or another snippet needs it.

// == assert_fail: os_write os_exit
.section .rodata
.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.text
.type assert_fail, @function
// lengua's assertion handler.
// In: w0 = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
// standard error and ends the program, every thread of it, with code 134.
assert_fail:
    sub sp, sp, #32
    adrp x1, assert_message
    add x1, x1, :lo12:assert_message
    ldp x2, x3, [x1]
    stp x2, x3, [sp]
    ldr x2, [x1, #16]
    str x2, [sp, #16]
    ldr w2, [x1, #24]
    str w2, [sp, #24]
    ldrb w2, [x1, #28]
    strb w2, [sp, #28]
    mov w2, #100
    udiv w3, w0, w2
    msub w0, w3, w2, w0
    add w3, w3, #48
    strb w3, [sp, #18]
    mov w2, #10
    udiv w3, w0, w2
    msub w0, w3, w2, w0
    add w3, w3, #48
    strb w3, [sp, #19]
    add w0, w0, #48
    strb w0, [sp, #20]
    mov x0, #2
    mov x1, sp
    mov x2, #29
    bl os_write
    mov x0, #134
    bl os_exit
.size assert_fail, . - assert_fail
