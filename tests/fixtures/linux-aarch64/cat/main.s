.arch armv8-a

// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// Sizes and limits
.equ IN_BUF_LEN, 5000
.equ MAX_READS, 5001
.equ MAX_WRITES, 64

.section .bss

// Standard input, as read.
.type in_buf, @object
in_buf:
    .zero IN_BUF_LEN
.size in_buf, . - in_buf

.text

// Reads standard input and prints it back. lengua's `_start` calls main, which never returns.
.type main, @function
main:
    adrp x0, in_buf
    add x0, x0, :lo12:in_buf
    mov x1, #IN_BUF_LEN
    bl read_all
    tbnz x0, #63, .Lmain_io_failed
    mov x9, #IN_BUF_LEN
    cmp x0, x9
    b.hi .Lmain_assert_length
    cbz x0, .Lmain_done
    mov x2, x0
    mov x0, #STDOUT
    adrp x1, in_buf
    add x1, x1, :lo12:in_buf
    cbz x1, .Lmain_assert_buffer
    bl write_all
    tbnz x0, #63, .Lmain_io_failed
.Lmain_done:
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_io_failed:
    mov w0, #EXIT_IO_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_length:
    mov w0, #1
    bl assert_fail
.Lmain_assert_buffer:
    mov w0, #2
    bl assert_fail
.size main, . - main

// Reads standard input until it ends or the buffer is full.
// In: x0 = the buffer, x1 = its size, at least 1.
// Out: x0 = the bytes read, or a negative error code.
// returns: result
.type read_all, @function
read_all:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lread_all_assert_null
    cbz x1, .Lread_all_assert_empty
    mov x10, x1
    mov x2, x1
    mov x1, x0
    mov x11, #MAX_READS
.Lread_all_loop:  // bound: 5001
    mov x0, #STDIN
    bl os_read
    tbnz x0, #63, .Lread_all_return
    cbz x0, .Lread_all_done
    cmp x0, x2
    b.hi .Lread_all_assert_count
    add x1, x1, x0
    subs x2, x2, x0
    b.eq .Lread_all_done
    subs x11, x11, #1
    b.ne .Lread_all_loop
    mov x0, #ERROR_IO
    b .Lread_all_return
.Lread_all_done:
    sub x0, x10, x2
.Lread_all_return:
    ldp x29, x30, [sp], #16
    ret
.Lread_all_assert_null:
    mov w0, #3
    bl assert_fail
.Lread_all_assert_empty:
    mov w0, #4
    bl assert_fail
.Lread_all_assert_count:
    mov w0, #5
    bl assert_fail
.size read_all, . - read_all

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
    mov w0, #6
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #7
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #8
    bl assert_fail
.size write_all, . - write_all
