# The edge on Linux, x86-64: lengua.s's `_start` and the operations a program calls, each a system
# call with a few instructions around it. src/edge/mod.rs says how this file is read: a line
# `# == name: needs` starts a snippet, which is written when main.s calls `name` or another snippet
# needs it, and what comes before the first is written always.
#
# Arguments go in rdi, rsi, rdx, r10, r8 and r9 and the result comes back in rax. A call changes
# rax, rcx, r11, the flags and every vector register, and no other register; an error is the
# negative of one of src/edge/mod.rs's `error`s, and every operation refuses descriptor 4,352,
# which is lengua's own. What lengua writes here carries the prefix `lengua_`, `LENGUA_` or
# `.Llengua_`, which a program's own names may not.

.equ LENGUA_SYS_READ, 0
.equ LENGUA_SYS_WRITE, 1
.equ LENGUA_SYS_EXIT_GROUP, 231
.equ LENGUA_CHANNEL, 4352
.equ LENGUA_EBADF, -9
.equ LENGUA_EIO, -5

# == _start
.text
.globl _start
.type _start, @function
# The Linux start stack holds argc, then argv and a null, then envp: main gets all three.
_start:
    mov rdi, qword ptr [rsp]
    lea rsi, [rsp + 8]
    lea rdx, [rsi + 8*rdi + 8]
    call main
    ud2
.size _start, . - _start

# == lengua_error
.text
.type lengua_error, @function
# In: rax = an error, negative; r11 = an operation's errors as bits (error 0 to 63, then 64 to 127).
# Out: rax = the same error when the operation gives it, else -EIO.
lengua_error:
    neg rax
    cmp rax, 127
    ja .Llengua_error_other
    mov rcx, rax
    shr rcx, 6
    mov rcx, qword ptr [r11 + 8*rcx]
    bt rcx, rax
    jnc .Llengua_error_other
    neg rax
    ret
.Llengua_error_other:
    mov rax, LENGUA_EIO
    ret
.size lengua_error, . - lengua_error

# == os_exit
.text
.type os_exit, @function
# In: edi = the status. Never returns.
os_exit:
    mov eax, LENGUA_SYS_EXIT_GROUP
    syscall
    ud2
.size os_exit, . - os_exit

# == os_write: lengua_error
.text
.type os_write, @function
# In: edi = the descriptor, rsi = the bytes, rdx = how many.
# Out: rax = how many it wrote, possibly fewer than asked, or an error.
os_write:
    cmp edi, LENGUA_CHANNEL
    je .Llengua_write_refused
    mov eax, LENGUA_SYS_WRITE
    syscall
    test rax, rax
    js .Llengua_write_failed
    ret
.Llengua_write_refused:
    mov rax, LENGUA_EBADF
    ret
.Llengua_write_failed:
    lea r11, [rip + os_write_errors]
    call lengua_error
    ret
.size os_write, . - os_write
