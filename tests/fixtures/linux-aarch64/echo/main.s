.arch armv8-a

// File descriptors: lengua hands a server its listening socket as 3.
.equ LISTENING, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// Sizes and limits
.equ LINE_MAX, 1024
.equ NEWLINE, 10

.section .bss

// One connection's line.
.type line, @object
line:
    .zero LINE_MAX
.size line, . - line

.text
// Takes connections on the listening socket for as long as it runs, echoing one line on each.
.type main, @function
main:
.Lmain_serve:  // bound: forever
    mov x0, #LISTENING
    bl os_accept
    tbnz x0, #63, .Lmain_next
    cmp x0, #LISTENING
    b.ls .Lmain_assert_connection
    mov x19, x0
    bl echo_line
    tbnz x0, #63, .Lmain_close
    cbnz x0, .Lmain_assert_result
.Lmain_close:
    mov x0, x19
    bl os_close
    tbnz x0, #63, .Lmain_next
.Lmain_next:
    b .Lmain_serve
.Lmain_assert_connection:
    mov w0, #1
    bl assert_fail
.Lmain_assert_result:
    mov w0, #2
    bl assert_fail
.size main, . - main

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
    bl os_write
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

