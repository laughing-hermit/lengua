.arch armv8-a

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

.text
// Naps 30 seconds on the monotonic clock, then says it's awake.
.type main, @function
main:
    mov x0, #CLOCK_MONOTONIC
    mov x1, #0
    adrp x2, nap
    add x2, x2, :lo12:nap
    cbz x2, .Lmain_assert_nap
    bl os_sleep
    cbnz x0, .Lmain_sleep_failed
    mov x0, #STDOUT
    adrp x1, awake_msg
    add x1, x1, :lo12:awake_msg
    mov x2, #AWAKE_LEN
    bl write_all
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_sleep_failed:
    mov w0, #EXIT_SLEEP_FAILED
    b .Lmain_exit
.Lmain_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_nap:
    mov w0, #4
    bl assert_fail
.size main, . - main

// Writes x2 bytes from x1 to file descriptor x0, retrying partial writes.
// In: x0 = the file descriptor, x1 = the bytes, x2 = how many, at least 1.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_all, @function
write_all:
    stp x29, x30, [sp, #-16]!
    cbz x1, .Lwrite_all_assert_null
    cbz x2, .Lwrite_all_assert_empty
    mov x9, x0
    mov x10, #MAX_WRITES
.Lwrite_all_loop:  // bound: 64
    mov x0, x9
    bl os_write
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
    ldp x29, x30, [sp], #16
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

