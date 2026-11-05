// The edge on Linux, AArch64: lengua.s's `_start` and the operations a program calls, each a
// system call with a few instructions around it. src/edge/mod.rs says how this file is read: a
// line `// == name: needs` starts a snippet, which is written when main.s calls `name` or another
// snippet needs it, and what comes before the first is written always.
//
// Arguments go in x0 to x5 and the result comes back in x0. A call changes x0, x8, x16, x17, x30,
// the flags, v0 to v7 and v16 to v31, the upper halves of v8 to v15, and all of z, p and ffr, and
// no other register; an error is the negative of one of src/edge/mod.rs's `error`s, and every
// operation refuses descriptor 4,352, which is lengua's own. What lengua writes here carries the
// prefix `lengua_`, `LENGUA_` or `.Llengua_`, which a program's own names may not.

.equ LENGUA_SYS_READ, 63
.equ LENGUA_SYS_WRITE, 64
.equ LENGUA_SYS_EXIT_GROUP, 94
.equ LENGUA_CHANNEL, 4352
.equ LENGUA_EBADF, -9
.equ LENGUA_EIO, -5

// == _start
.text
.globl _start
.type _start, @function
// The Linux start stack holds argc, then argv and a null, then envp: main gets all three.
_start:
    ldr x0, [sp]
    add x1, sp, #8
    add x2, x1, x0, lsl #3
    add x2, x2, #8
    bl main
    brk #0
.size _start, . - _start

// == lengua_error
.text
.type lengua_error, @function
// In: x0 = an error, negative; x17 = an operation's errors as bits (error 0 to 63, then 64 to 127).
// Out: x0 = the same error when the operation gives it, else -EIO.
lengua_error:
    neg x0, x0
    cmp x0, #127
    b.hi .Llengua_error_other
    lsr x16, x0, #6
    ldr x16, [x17, x16, lsl #3]
    lsr x16, x16, x0
    tbz x16, #0, .Llengua_error_other
    neg x0, x0
    ret
.Llengua_error_other:
    mov x0, #LENGUA_EIO
    ret
.size lengua_error, . - lengua_error

// == os_exit
.text
.type os_exit, @function
// In: w0 = the status. Never returns.
os_exit:
    mov x8, #LENGUA_SYS_EXIT_GROUP
    svc #0
    brk #0
.size os_exit, . - os_exit

// == os_write: lengua_error
.text
.type os_write, @function
// In: w0 = the descriptor, x1 = the bytes, x2 = how many.
// Out: x0 = how many it wrote, possibly fewer than asked, or an error.
os_write:
    mov w16, #LENGUA_CHANNEL
    cmp w0, w16
    b.eq .Llengua_write_refused
    mov x8, #LENGUA_SYS_WRITE
    svc #0
    tbnz x0, #63, .Llengua_write_failed
    ret
.Llengua_write_refused:
    mov x0, #LENGUA_EBADF
    ret
.Llengua_write_failed:
    adrp x17, os_write_errors
    add x17, x17, :lo12:os_write_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_write, . - os_write
