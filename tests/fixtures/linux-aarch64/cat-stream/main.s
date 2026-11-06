.arch armv8-a

// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_READ_FAILED, 3
.equ EXIT_WRITE_FAILED, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// fill's end byte for none
.equ READ_TO_END, -1
// Sizes and limits
.equ BUF_LEN, 4096
.equ MAX_FULLS, 1099511627776
.equ MAX_WRITES, 64

.section .bss

// One buffer-full of standard input.
.type buffer, @object
buffer:
    .zero BUF_LEN
.size buffer, . - buffer

.text

// Copies standard input of any length to standard output, a buffer-full at a time: fill reads
// until the buffer is full or the input ends, so a buffer-full that isn't full is the last.
// lengua's `_start` calls main, which never returns.
.type main, @function
main:
    mov x19, #MAX_FULLS
.Lmain_full:  // bound: 1099511627776
    mov x0, #STDIN
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, #BUF_LEN
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lmain_read_failed
    cbz x0, .Lmain_done
    cmp x0, #BUF_LEN
    b.hi .Lmain_assert_length
    mov x20, x0
    mov x2, x0
    mov x0, #STDOUT
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    bl write_all
    tbnz x0, #63, .Lmain_write_failed
    cmp x20, #BUF_LEN
    b.lo .Lmain_done
    subs x19, x19, #1
    b.ne .Lmain_full
    b .Lmain_read_failed
.Lmain_done:
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_read_failed:
    mov w0, #EXIT_READ_FAILED
    b .Lmain_exit
.Lmain_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_length:
    mov w0, #1
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
    mov w0, #2
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #3
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #4
    bl assert_fail
.size write_all, . - write_all
