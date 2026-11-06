// The AArch64 library: functions a program may call without defining them, written once for every
// operating system because they reach the OS only through the operations (os_write, os_exit,
// ...). src/edge/mod.rs says how this file is read: a line `// == name: needs` starts a snippet,
// which is written when main.s calls `name` or another snippet needs it.

.equ LENGUA_FILL_READS, 1099511627776
.equ LENGUA_FILL_IO, -5

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

// == fill: assert_fail os_read
.text
// returns: result
.type fill, @function
// Reads from file descriptor x0 into the x2 bytes at x1 until they're full, the input ends,
// or, when w3 is a byte (0 to 255), the last byte read is w3; with w3 = -1 it reads to the
// end. However the input arrives, it reads the same.
// In: x0 = the file descriptor, x1 = the buffer, x2 = its size, at least 1, w3 = the end
// byte or -1.
// Out: x0 = how many bytes it read, or a negative error code.
fill:
    stp x29, x30, [sp, #-16]!
    cbz x1, .Llengua_fill_assert_null
    cbz x2, .Llengua_fill_assert_empty
    mov x9, x0
    mov x10, x2
    mov w11, w3
    mov x12, #LENGUA_FILL_READS
.Llengua_fill_loop:  // bound: 1099511627776
    mov x0, x9
    bl os_read
    tbnz x0, #63, .Llengua_fill_return
    cbz x0, .Llengua_fill_done
    cmp x0, x2
    b.hi .Llengua_fill_assert_count
    add x1, x1, x0
    subs x2, x2, x0
    b.eq .Llengua_fill_done
    ldurb w13, [x1, #-1]
    cmp w13, w11
    b.eq .Llengua_fill_done
    subs x12, x12, #1
    b.ne .Llengua_fill_loop
    mov x0, #LENGUA_FILL_IO
    b .Llengua_fill_return
.Llengua_fill_done:
    sub x0, x10, x2
.Llengua_fill_return:
    ldp x29, x30, [sp], #16
    ret
.Llengua_fill_assert_null:
    mov w0, #990
    bl assert_fail
.Llengua_fill_assert_empty:
    mov w0, #991
    bl assert_fail
.Llengua_fill_assert_count:
    mov w0, #992
    bl assert_fail
.size fill, . - fill

// == parallel_for: assert_fail os_thread_start os_thread_join
.text
.type parallel_for, @function
// Runs worker(index, slot) for every index from 0 to count - 1, each on its own thread (worker 0
// on this one), and returns when all have finished. worker is main.s's: it takes w0 = its index
// and x1 = its slot, writes only its own slot and its own stack, and returns a result in x0,
// which lands in results[index].
// In: x0 = count, from 1 to 64; x1 = the slots, slot_size bytes each; x2 = slot_size;
// x3 = the results, 8 bytes each.
parallel_for:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    mov x20, x0
    mov x21, x1
    mov x22, x2
    mov x23, x3
    cmp x20, #1
    b.lo .Llengua_parallel_assert_none
    cmp x20, #64
    b.hi .Llengua_parallel_assert_many
    mov x19, #1
.Llengua_parallel_start:  // bound: 64
    cmp x19, x20
    b.hs .Llengua_parallel_started
    mov x0, x19
    madd x1, x19, x22, x21
    bl os_thread_start
    tbnz x0, #63, .Llengua_parallel_assert_start
    add x19, x19, #1
    b .Llengua_parallel_start
.Llengua_parallel_started:
    mov x0, #0
    mov x1, x21
    bl worker
    str x0, [x23]
    mov x19, #1
.Llengua_parallel_join:  // bound: 64
    cmp x19, x20
    b.hs .Llengua_parallel_joined
    mov x0, x19
    bl os_thread_join
    str x0, [x23, x19, lsl #3]
    add x19, x19, #1
    b .Llengua_parallel_join
.Llengua_parallel_joined:
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Llengua_parallel_assert_none:
    mov w0, #901
    bl assert_fail
.Llengua_parallel_assert_many:
    mov w0, #902
    bl assert_fail
.Llengua_parallel_assert_start:
    mov w0, #903
    bl assert_fail
.size parallel_for, . - parallel_for
