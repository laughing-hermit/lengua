.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_WRITE, 64
.equ SYS_EXIT, 93
.equ SYS_CLOCK_NANOSLEEP, 115
// File descriptors
.equ STDOUT, 1
// The clock that only moves forward
.equ CLOCK_MONOTONIC, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_SLEEP_FAILED, 2
.equ EXIT_WRITE_FAILED, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ AWAKE_LEN, 6
.equ MAX_WRITES, 64

.section .rodata

// How long the nap is: 30 seconds and no nanoseconds.
.type nap, @object
nap:
    .quad 30
    .quad 0
.size nap, . - nap

.type awake_msg, @object
awake_msg:
    .ascii "awake\n"
.size awake_msg, . - awake_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.text
.globl _start

// Naps 30 seconds on the monotonic clock, then says it's awake.
.type _start, @function
_start:
    mov x0, #CLOCK_MONOTONIC
    mov x1, #0
    adrp x2, nap
    add x2, x2, :lo12:nap
    cbz x2, .Lstart_assert_nap
    mov x3, #0
    mov x8, #SYS_CLOCK_NANOSLEEP
    svc #0
    cbnz x0, .Lstart_sleep_failed
    mov x0, #STDOUT
    adrp x1, awake_msg
    add x1, x1, :lo12:awake_msg
    mov x2, #AWAKE_LEN
    bl write_all
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_sleep_failed:
    mov w0, #EXIT_SLEEP_FAILED
    b .Lstart_exit
.Lstart_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_nap:
    mov w0, #4
    bl assert_fail
.size _start, . - _start

// Writes x2 bytes from x1 to file descriptor x0, retrying partial writes.
// In: x0 = the file descriptor, x1 = the bytes, x2 = how many, at least 1.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_all, @function
write_all:
    cbz x1, .Lwrite_all_assert_null
    cbz x2, .Lwrite_all_assert_empty
    mov x9, x0
    mov x10, #MAX_WRITES
.Lwrite_all_loop:  // bound: 64
    mov x0, x9
    mov x8, #SYS_WRITE
    svc #0
    tbnz x0, #63, .Lwrite_all_return
    cmp x0, x2
    b.hi .Lwrite_all_assert_count
    add x1, x1, x0
    subs x2, x2, x0
    b.eq .Lwrite_all_done
    subs x10, x10, #1
    b.ne .Lwrite_all_loop
    mov x0, #ERROR_IO
    b .Lwrite_all_return
.Lwrite_all_done:
    mov x0, #0
.Lwrite_all_return:
    ret
.Lwrite_all_assert_null:
    mov w0, #10
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #11
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #12
    bl assert_fail
.size write_all, . - write_all

// lengua's assertion handler. check compares it word for word: copy it exactly.
// In: w0 = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
// standard error and ends the program, every thread of it, with code 134.
.type assert_fail, @function
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
    mov x8, #64
    svc #0
    mov x0, #134
    mov x8, #94
    svc #0
.size assert_fail, . - assert_fail
