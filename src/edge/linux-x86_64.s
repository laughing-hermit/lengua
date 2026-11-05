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
.equ LENGUA_SYS_CLOSE, 3
.equ LENGUA_SYS_POLL, 7
.equ LENGUA_SYS_LSEEK, 8
.equ LENGUA_SYS_SHUTDOWN, 48
.equ LENGUA_SYS_CLOCK_GETTIME, 228
.equ LENGUA_SYS_CLOCK_NANOSLEEP, 230
.equ LENGUA_SYS_EXIT_GROUP, 231
.equ LENGUA_SYS_MKDIRAT, 258
.equ LENGUA_SYS_UNLINKAT, 263
.equ LENGUA_SYS_RENAMEAT, 264
.equ LENGUA_SYS_ACCEPT4, 288
.equ LENGUA_SYS_GETRANDOM, 318
.equ LENGUA_AT_FDCWD, -100
.equ LENGUA_AT_REMOVEDIR, 512
.equ LENGUA_FOLDER_MODE, 493
.equ LENGUA_CHANNEL, 4352
.equ LENGUA_LISTENER, 3
.equ LENGUA_RANDOM_MOST, 256
.equ LENGUA_EBADF, -9
.equ LENGUA_EIO, -5
.equ LENGUA_EINVAL, -22

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

# == os_read: lengua_error
.text
.type os_read, @function
# In: edi = the descriptor, rsi = the buffer, rdx = its size.
# Out: rax = how many bytes it read, 0 at the end of the input, or an error.
os_read:
    cmp edi, LENGUA_CHANNEL
    je .Llengua_read_refused
    mov eax, LENGUA_SYS_READ
    syscall
    test rax, rax
    js .Llengua_read_failed
    ret
.Llengua_read_refused:
    mov rax, LENGUA_EBADF
    ret
.Llengua_read_failed:
    lea r11, [rip + os_read_errors]
    call lengua_error
    ret
.size os_read, . - os_read

# == os_close: lengua_error
.text
.type os_close, @function
# In: edi = the descriptor.
# Out: rax = 0, or an error.
os_close:
    cmp edi, LENGUA_CHANNEL
    je .Llengua_close_refused
    mov eax, LENGUA_SYS_CLOSE
    syscall
    test rax, rax
    js .Llengua_close_failed
    ret
.Llengua_close_refused:
    mov rax, LENGUA_EBADF
    ret
.Llengua_close_failed:
    lea r11, [rip + os_close_errors]
    call lengua_error
    ret
.size os_close, . - os_close

# == os_seek: lengua_error
.text
.type os_seek, @function
# In: edi = the descriptor, rsi = the offset, edx = where it counts from (0 the start, 1 here,
# 2 the end).
# Out: rax = the new offset, or an error.
os_seek:
    cmp edi, LENGUA_CHANNEL
    je .Llengua_seek_refused
    mov eax, LENGUA_SYS_LSEEK
    syscall
    test rax, rax
    js .Llengua_seek_failed
    ret
.Llengua_seek_refused:
    mov rax, LENGUA_EBADF
    ret
.Llengua_seek_failed:
    lea r11, [rip + os_seek_errors]
    call lengua_error
    ret
.size os_seek, . - os_seek

# == os_mkdir: lengua_error
.text
.type os_mkdir, @function
# In: rdi = the name, NUL-terminated, relative to the working folder.
# Out: rax = 0, or an error.
os_mkdir:
    push rdi
    push rsi
    push rdx
    mov rsi, rdi
    mov edi, LENGUA_AT_FDCWD
    mov edx, LENGUA_FOLDER_MODE
    mov eax, LENGUA_SYS_MKDIRAT
    syscall
    pop rdx
    pop rsi
    pop rdi
    test rax, rax
    js .Llengua_mkdir_failed
    ret
.Llengua_mkdir_failed:
    lea r11, [rip + os_mkdir_errors]
    call lengua_error
    ret
.size os_mkdir, . - os_mkdir

# == os_remove: lengua_error
.text
.type os_remove, @function
# In: rdi = the name, esi = 0 for a file or 1 for an empty folder.
# Out: rax = 0, or an error.
os_remove:
    cmp esi, 1
    ja .Llengua_remove_invalid
    push rdi
    push rsi
    push rdx
    xor edx, edx
    cmp esi, 1
    jne .Llengua_remove_file
    mov edx, LENGUA_AT_REMOVEDIR
.Llengua_remove_file:
    mov rsi, rdi
    mov edi, LENGUA_AT_FDCWD
    mov eax, LENGUA_SYS_UNLINKAT
    syscall
    pop rdx
    pop rsi
    pop rdi
    test rax, rax
    js .Llengua_remove_failed
    ret
.Llengua_remove_invalid:
    mov rax, LENGUA_EINVAL
    ret
.Llengua_remove_failed:
    lea r11, [rip + os_remove_errors]
    call lengua_error
    ret
.size os_remove, . - os_remove

# == os_rename: lengua_error
.text
.type os_rename, @function
# In: rdi = the name it has, rsi = the name it is to have, which is replaced.
# Out: rax = 0, or an error.
os_rename:
    push rdi
    push rsi
    push rdx
    push r10
    mov r10, rsi
    mov rsi, rdi
    mov edi, LENGUA_AT_FDCWD
    mov edx, LENGUA_AT_FDCWD
    mov eax, LENGUA_SYS_RENAMEAT
    syscall
    pop r10
    pop rdx
    pop rsi
    pop rdi
    test rax, rax
    js .Llengua_rename_failed
    ret
.Llengua_rename_failed:
    lea r11, [rip + os_rename_errors]
    call lengua_error
    ret
.size os_rename, . - os_rename

# == os_clock: lengua_error
.text
.type os_clock, @function
# In: edi = 0 for the time of day or 1 for a clock that only moves forward, rsi = 16 bytes for the
# seconds and then the nanoseconds.
# Out: rax = 0, or an error.
os_clock:
    cmp edi, 1
    ja .Llengua_clock_invalid
    mov eax, LENGUA_SYS_CLOCK_GETTIME
    syscall
    test rax, rax
    js .Llengua_clock_failed
    ret
.Llengua_clock_invalid:
    mov rax, LENGUA_EINVAL
    ret
.Llengua_clock_failed:
    lea r11, [rip + os_clock_errors]
    call lengua_error
    ret
.size os_clock, . - os_clock

# == os_sleep: lengua_error
.text
.type os_sleep, @function
# In: edi = the clock as for os_clock, esi = 0 to wait that long or 1 to wait until that time,
# rdx = 16 bytes for the seconds and then the nanoseconds.
# Out: rax = 0, or an error.
os_sleep:
    cmp edi, 1
    ja .Llengua_sleep_invalid
    cmp esi, 1
    ja .Llengua_sleep_invalid
    push r10
    xor r10d, r10d
    mov eax, LENGUA_SYS_CLOCK_NANOSLEEP
    syscall
    pop r10
    test rax, rax
    js .Llengua_sleep_failed
    ret
.Llengua_sleep_invalid:
    mov rax, LENGUA_EINVAL
    ret
.Llengua_sleep_failed:
    lea r11, [rip + os_sleep_errors]
    call lengua_error
    ret
.size os_sleep, . - os_sleep

# == os_random: lengua_error
.text
.type os_random, @function
# In: rdi = the buffer, rsi = how many bytes, at most 256.
# Out: rax = how many it filled, all of them, or an error.
os_random:
    cmp rsi, LENGUA_RANDOM_MOST
    ja .Llengua_random_invalid
    push rdx
    xor edx, edx
    mov eax, LENGUA_SYS_GETRANDOM
    syscall
    pop rdx
    test rax, rax
    js .Llengua_random_failed
    ret
.Llengua_random_invalid:
    mov rax, LENGUA_EINVAL
    ret
.Llengua_random_failed:
    lea r11, [rip + os_random_errors]
    call lengua_error
    ret
.size os_random, . - os_random

# == os_accept: lengua_error
.text
.type os_accept, @function
# In: edi = 3, the listening socket lengua bound for the program.
# Out: rax = the next connection's descriptor, or an error.
os_accept:
    cmp edi, LENGUA_LISTENER
    jne .Llengua_accept_refused
    push rsi
    push rdx
    push r10
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    mov eax, LENGUA_SYS_ACCEPT4
    syscall
    pop r10
    pop rdx
    pop rsi
    test rax, rax
    js .Llengua_accept_failed
    ret
.Llengua_accept_refused:
    mov rax, LENGUA_EBADF
    ret
.Llengua_accept_failed:
    lea r11, [rip + os_accept_errors]
    call lengua_error
    ret
.size os_accept, . - os_accept

# == os_shutdown: lengua_error
.text
.type os_shutdown, @function
# In: edi = the connection, esi = 0 to stop receiving, 1 to stop sending or 2 for both.
# Out: rax = 0, or an error.
os_shutdown:
    cmp edi, LENGUA_CHANNEL
    je .Llengua_shutdown_refused
    mov eax, LENGUA_SYS_SHUTDOWN
    syscall
    test rax, rax
    js .Llengua_shutdown_failed
    ret
.Llengua_shutdown_refused:
    mov rax, LENGUA_EBADF
    ret
.Llengua_shutdown_failed:
    lea r11, [rip + os_shutdown_errors]
    call lengua_error
    ret
.size os_shutdown, . - os_shutdown

# == os_poll: lengua_error
.text
.type os_poll, @function
# In: rdi = a table of 8-byte entries (the descriptor, 4 bytes; the events waited for, 2; the
# events that came, 2), esi = how many, edx = the most milliseconds to wait, -1 for no limit.
# Out: rax = how many entries have events, 0 when the time ran out, or an error. A negative
# descriptor is passed over, and descriptor 4,352 is refused.
os_poll:
    xor ecx, ecx
.Llengua_poll_scan:  # bound: 4352
    cmp ecx, esi
    jae .Llengua_poll_scanned
    cmp dword ptr [rdi + 8*rcx], LENGUA_CHANNEL
    je .Llengua_poll_refused
    inc ecx
    jmp .Llengua_poll_scan
.Llengua_poll_scanned:
    mov eax, LENGUA_SYS_POLL
    syscall
    test rax, rax
    js .Llengua_poll_failed
    ret
.Llengua_poll_refused:
    mov rax, LENGUA_EBADF
    ret
.Llengua_poll_failed:
    lea r11, [rip + os_poll_errors]
    call lengua_error
    ret
.size os_poll, . - os_poll
