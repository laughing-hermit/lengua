.arch armv8-a

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
.equ ERROR_IO, -5

.section .bss

// Both lines, as they're read.
.type input, @object
input:
    .zero INPUT_MAX
.size input, . - input

.text
// Reads both lines with one fill to the first newline a read ends with, and prints the second:
// right whenever no read ends right after the first line.
.type main, @function
main:
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, #INPUT_MAX
    mov w3, #NEWLINE
    bl fill
    tbnz x0, #63, .Lmain_failed
    cmp x0, #INPUT_MAX
    b.hi .Lmain_assert_count
    mov x1, x0
    adrp x0, input
    add x0, x0, :lo12:input
    bl print_second
    tbnz x0, #63, .Lmain_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_failed:
    mov w0, #EXIT_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_count:
    mov w0, #1
    bl assert_fail
.size main, . - main

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

