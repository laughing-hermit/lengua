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
.equ EXIT_IO_FAILED, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// Sizes and limits
.equ IN_BUF_LEN, 5000
.equ MAX_READS, 5001
.equ MAX_WRITES, 64

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// Standard input, as read.
.type in_buf, @object
in_buf:
    .zero IN_BUF_LEN
.size in_buf, . - in_buf

.text
.globl _start

// Reads standard input and prints it back.
.type _start, @function
_start:
    adrp x0, in_buf
    add x0, x0, :lo12:in_buf
    mov x1, #IN_BUF_LEN
    bl read_all
    tbnz x0, #63, .Lstart_io_failed
    mov x9, #IN_BUF_LEN
    cmp x0, x9
    b.hi .Lstart_assert_length
    cbz x0, .Lstart_done
    mov x2, x0
    mov x0, #STDOUT
    adrp x1, in_buf
    add x1, x1, :lo12:in_buf
    cbz x1, .Lstart_assert_buffer
    bl write_all
    tbnz x0, #63, .Lstart_io_failed
.Lstart_done:
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_io_failed:
    mov w0, #EXIT_IO_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_length:
    mov w0, #1
    bl assert_fail
.Lstart_assert_buffer:
    mov w0, #2
    bl assert_fail
.size _start, . - _start

// Reads standard input until it ends or the buffer is full.
// In: x0 = the buffer, x1 = its size, at least 1.
// Out: x0 = the bytes read, or a negative error code.
// returns: result
.type read_all, @function
read_all:
    cbz x0, .Lread_all_assert_null
    cbz x1, .Lread_all_assert_empty
    mov x10, x1
    mov x2, x1
    mov x1, x0
    mov x11, #MAX_READS
.Lread_all_loop:  // bound: 5001
    mov x0, #STDIN
    mov x8, #SYS_READ
    svc #0
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
    mov w0, #6
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #7
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #8
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
