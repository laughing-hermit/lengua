.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_READ, 63
.equ SYS_WRITE, 64
.equ SYS_EXIT, 93
// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_READ_FAILED, 3
.equ EXIT_WRITE_FAILED, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// fill's limit on reads, as assets/linux-aarch64/fill.s has it, and its end byte for none
.equ MAX_FILL_READS, 1099511627776
.equ READ_TO_END, -1
// Sizes and limits
.equ BUF_LEN, 4096
.equ MAX_FULLS, 1099511627776
.equ MAX_WRITES, 64

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// One buffer-full of standard input.
.type buffer, @object
buffer:
    .zero BUF_LEN
.size buffer, . - buffer

.text
.globl _start

// Copies standard input of any length to standard output, a buffer-full at a time: fill reads
// until the buffer is full or the input ends, so a buffer-full that isn't full is the last.
.type _start, @function
_start:
    mov x19, #MAX_FULLS
.Lstart_full:  // bound: 1099511627776
    mov x0, #STDIN
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, #BUF_LEN
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lstart_read_failed
    cbz x0, .Lstart_done
    cmp x0, #BUF_LEN
    b.hi .Lstart_assert_length
    mov x20, x0
    mov x2, x0
    mov x0, #STDOUT
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    bl write_all
    tbnz x0, #63, .Lstart_write_failed
    cmp x20, #BUF_LEN
    b.lo .Lstart_done
    subs x19, x19, #1
    b.ne .Lstart_full
    b .Lstart_read_failed
.Lstart_done:
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_read_failed:
    mov w0, #EXIT_READ_FAILED
    b .Lstart_exit
.Lstart_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_length:
    mov w0, #1
    bl assert_fail
.size _start, . - _start

// Reads from file descriptor x0 into the x2 bytes at x1 until they're full, the input ends,
// or, when w3 is a byte (0 to 255), the last byte read is w3; with w3 = -1 it reads to the
// end. However the input arrives, it reads the same.
// In: x0 = the file descriptor, x1 = the buffer, x2 = its size, at least 1, w3 = the end
// byte or -1.
// Out: x0 = how many bytes it read, or a negative error code.
// returns: result
.type fill, @function
fill:
    cbz x1, .Lfill_assert_null
    cbz x2, .Lfill_assert_empty
    mov x9, x0
    mov x10, x2
    mov w11, w3
    mov x12, #MAX_FILL_READS
.Lfill_loop:  // bound: 1099511627776
    mov x0, x9
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lfill_return
    cbz x0, .Lfill_done
    cmp x0, x2
    b.hi .Lfill_assert_count
    add x1, x1, x0
    subs x2, x2, x0
    b.eq .Lfill_done
    ldurb w13, [x1, #-1]
    cmp w13, w11
    b.eq .Lfill_done
    subs x12, x12, #1
    b.ne .Lfill_loop
    mov x0, #ERROR_IO
    b .Lfill_return
.Lfill_done:
    sub x0, x10, x2
.Lfill_return:
    ret
.Lfill_assert_null:
    mov w0, #990
    bl assert_fail
.Lfill_assert_empty:
    mov w0, #991
    bl assert_fail
.Lfill_assert_count:
    mov w0, #992
    bl assert_fail
.size fill, . - fill

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
    mov w0, #2
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #3
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #4
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
