.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_WRITE, 64
.equ SYS_EXIT, 93
// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_FAILED, 1
// Two lines of at most 20 letters and their newlines, with room
.equ INPUT_MAX, 64
.equ MAX_WRITES, 64
.equ NEWLINE, 10
// fill's limit on reads, as assets/linux-aarch64/fill.s has it
.equ SYS_READ, 63
.equ ERROR_IO, -5
.equ MAX_FILL_READS, 1099511627776

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// Both lines, as they're read.
.type input, @object
input:
    .zero INPUT_MAX
.size input, . - input

.text
.globl _start

// Reads both lines with one fill to the first newline a read ends with, and prints the second:
// right whenever no read ends right after the first line.
.type _start, @function
_start:
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, #INPUT_MAX
    mov w3, #NEWLINE
    bl fill
    tbnz x0, #63, .Lstart_failed
    cmp x0, #INPUT_MAX
    b.hi .Lstart_assert_count
    mov x1, x0
    adrp x0, input
    add x0, x0, :lo12:input
    bl print_second
    tbnz x0, #63, .Lstart_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_failed:
    mov w0, #EXIT_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_count:
    mov w0, #1
    bl assert_fail
.size _start, . - _start

// Prints what follows the first newline of the bytes read.
// In: x0 = the bytes; x1 = how many, up to INPUT_MAX.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_second, @function
print_second:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lprint_second_assert_null
    cmp x1, #INPUT_MAX
    b.hi .Lprint_second_assert_length
    mov x2, #0
    mov x4, #INPUT_MAX
.Lprint_second_scan:  // bound: 64
    cmp x2, x1
    b.hs .Lprint_second_write
    ldrb w5, [x0, x2]
    add x2, x2, #1
    cmp w5, #NEWLINE
    b.eq .Lprint_second_write
    subs x4, x4, #1
    b.ne .Lprint_second_scan
.Lprint_second_write:
    subs x6, x1, x2
    b.eq .Lprint_second_none
    add x1, x0, x2
    mov x2, x6
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lprint_second_return
.Lprint_second_none:
    mov x0, #0
.Lprint_second_return:
    ldp x29, x30, [sp], #16
    ret
.Lprint_second_assert_null:
    mov w0, #2
    bl assert_fail
.Lprint_second_assert_length:
    mov w0, #3
    bl assert_fail
.size print_second, . - print_second

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
