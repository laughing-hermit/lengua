.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_READ, 63
.equ SYS_CLOSE, 57
.equ SYS_SENDTO, 206
.equ SYS_ACCEPT4, 242
// File descriptors: lengua hands a server its listening socket as 3.
.equ LISTENING, 3
// Socket flags: no SIGPIPE when a client hangs up early.
.equ MSG_NOSIGNAL, 0x4000
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// Sizes and limits
.equ LINE_MAX, 1024
.equ NEWLINE, 10
// fill's limit on reads, as fill.s has it
.equ MAX_FILL_READS, 1099511627776

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// One connection's line.
.type line, @object
line:
    .zero LINE_MAX
.size line, . - line

.text
.globl _start

// Takes connections on the listening socket for as long as it runs, echoing one line on each.
.type _start, @function
_start:
.Lstart_serve:  // bound: forever
    mov x0, #LISTENING
    mov x1, #0
    mov x2, #0
    mov x3, #0
    mov x8, #SYS_ACCEPT4
    svc #0
    tbnz x0, #63, .Lstart_next
    cmp x0, #LISTENING
    b.ls .Lstart_assert_connection
    mov x19, x0
    bl echo_line
    tbnz x0, #63, .Lstart_close
    cbnz x0, .Lstart_assert_result
.Lstart_close:
    mov x0, x19
    mov x8, #SYS_CLOSE
    svc #0
    tbnz x0, #63, .Lstart_next
.Lstart_next:
    b .Lstart_serve
.Lstart_assert_connection:
    mov w0, #1
    bl assert_fail
.Lstart_assert_result:
    mov w0, #2
    bl assert_fail
.size _start, . - _start

// Reads a line from a connection, until it ends with a newline, the client closes its side, or
// it fills the buffer, and sends it back.
// In: x0 = the connection.
// Out: x0 = 0, or a negative error code.
// returns: result
.type echo_line, @function
echo_line:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    mov x19, x0
    adrp x1, line
    add x1, x1, :lo12:line
    mov x2, #LINE_MAX
    mov w3, #NEWLINE
    bl fill
    tbnz x0, #63, .Lecho_line_return
    cbz x0, .Lecho_line_return
    cmp x0, #LINE_MAX
    b.hi .Lecho_line_assert_count
    mov x2, x0
    mov x0, x19
    adrp x1, line
    add x1, x1, :lo12:line
    mov x3, #MSG_NOSIGNAL
    mov x4, #0
    mov x5, #0
    mov x8, #SYS_SENDTO
    svc #0
    tbnz x0, #63, .Lecho_line_return
    cmp x0, x2
    b.ne .Lecho_line_short
    mov x0, #0
    b .Lecho_line_return
.Lecho_line_short:
    mov x0, #ERROR_IO
.Lecho_line_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lecho_line_assert_count:
    mov w0, #3
    bl assert_fail
.size echo_line, . - echo_line

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
