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
.equ LENGUA_SYS_FSTAT, 5
.equ LENGUA_SYS_POLL, 7
.equ LENGUA_SYS_LSEEK, 8
.equ LENGUA_SYS_IOCTL, 16
.equ LENGUA_SYS_SHUTDOWN, 48
.equ LENGUA_SYS_CLONE, 56
.equ LENGUA_SYS_EXIT, 60
.equ LENGUA_SYS_FUTEX, 202
.equ LENGUA_SYS_CLOCK_GETTIME, 228
.equ LENGUA_SYS_CLOCK_NANOSLEEP, 230
.equ LENGUA_SYS_EXIT_GROUP, 231
.equ LENGUA_SYS_MKDIRAT, 258
.equ LENGUA_SYS_UNLINKAT, 263
.equ LENGUA_SYS_RENAMEAT, 264
.equ LENGUA_SYS_GETDENTS64, 217
.equ LENGUA_SYS_OPENAT, 257
.equ LENGUA_SYS_ACCEPT4, 288
.equ LENGUA_SYS_GETRANDOM, 318
.equ LENGUA_AT_FDCWD, -100
.equ LENGUA_AT_REMOVEDIR, 512
.equ LENGUA_FOLDER_MODE, 493
.equ LENGUA_FILE_MODE, 420
.equ LENGUA_KIND_FILE, 0
.equ LENGUA_KIND_FOLDER, 1
.equ LENGUA_KIND_OTHER, 2
.equ LENGUA_CHANNEL, 4352
.equ LENGUA_MAIN_RETURNED, 125
.equ LENGUA_LISTENER, 3
.equ LENGUA_RANDOM_MOST, 256
.equ LENGUA_CLONE_FLAGS, 0x350f00
.equ LENGUA_MAX_WAITS, 64
.equ LENGUA_EBADF, -9
.equ LENGUA_EIO, -5
.equ LENGUA_EINVAL, -22
.equ LENGUA_ENOTTY, -25
.equ LENGUA_TCGETS, 0x5401
.equ LENGUA_TCSETS, 0x5402
.equ LENGUA_TIOCGWINSZ, 0x5413
.equ LENGUA_NOT_LINE_MODE, -11

# == _start: os_exit
.text
.globl _start
.type _start, @function
# The Linux start stack holds argc, then argv and a null, then envp: main gets all three.
_start:
    mov rdi, qword ptr [rsp]
    lea rsi, [rsp + 8]
    lea rdx, [rsi + 8*rdi + 8]
    call main
    mov edi, LENGUA_MAIN_RETURNED
    call os_exit
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

# == os_open: lengua_error
.section .rodata
.balign 4
.type lengua_open_flags, @object
# open's flags for each mode: READ, WRITE, APPEND, NEW and FOLDER.
lengua_open_flags:
    .long 0, 0x241, 0x441, 0xc1, 0x10000
.size lengua_open_flags, . - lengua_open_flags

.text
.type os_open, @function
# In: rdi = the name, NUL-terminated, relative to the working folder; esi = the mode: 0 to read, 1
# to write, which makes the file and empties it, 2 to write at its end, 3 to write a file that
# is not there yet, 4 to list a folder.
# Out: rax = a descriptor, or an error.
os_open:
    cmp esi, 4
    ja .Llengua_open_invalid
    push rdi
    push rsi
    push rdx
    push r10
    mov eax, esi
    lea r11, [rip + lengua_open_flags]
    mov edx, dword ptr [r11 + 4*rax]
    mov rsi, rdi
    mov edi, LENGUA_AT_FDCWD
    mov r10d, LENGUA_FILE_MODE
    mov eax, LENGUA_SYS_OPENAT
    syscall
    pop r10
    pop rdx
    pop rsi
    pop rdi
    test rax, rax
    js .Llengua_open_failed
    ret
.Llengua_open_invalid:
    mov rax, LENGUA_EINVAL
    ret
.Llengua_open_failed:
    lea r11, [rip + os_open_errors]
    call lengua_error
    ret
.size os_open, . - os_open

# == os_info: lengua_error
.text
.type os_info, @function
# In: edi = the descriptor, rsi = 16 bytes for the size and then the kind.
# Out: rax = 0, or an error. The kind is 0 for a file, 1 for a folder and 2 for anything else,
# and the size is 0 unless it is a file.
os_info:
    cmp edi, LENGUA_CHANNEL
    je .Llengua_info_refused
    push rsi
    sub rsp, 144
    mov rsi, rsp
    mov eax, LENGUA_SYS_FSTAT
    syscall
    test rax, rax
    js .Llengua_info_failed
    mov eax, dword ptr [rsp + 24]
    and eax, 0xf000
    cmp eax, 0x8000
    je .Llengua_info_file
    mov ecx, LENGUA_KIND_OTHER
    cmp eax, 0x4000
    jne .Llengua_info_sizeless
    mov ecx, LENGUA_KIND_FOLDER
.Llengua_info_sizeless:
    xor eax, eax
    jmp .Llengua_info_store
.Llengua_info_file:
    xor ecx, ecx
    mov rax, qword ptr [rsp + 48]
.Llengua_info_store:
    mov r11, qword ptr [rsp + 144]
    mov qword ptr [r11], rax
    mov qword ptr [r11 + 8], rcx
    xor eax, eax
    add rsp, 144
    pop rsi
    ret
.Llengua_info_refused:
    mov rax, LENGUA_EBADF
    ret
.Llengua_info_failed:
    add rsp, 144
    pop rsi
    lea r11, [rip + os_info_errors]
    call lengua_error
    ret
.size os_info, . - os_info

# == os_list: lengua_error
.section .bss
.balign 8
.type lengua_dirents, @object
# What the kernel lists, before it is put in the program's buffer in the form every system gives.
lengua_dirents:
    .zero 4096
.size lengua_dirents, . - lengua_dirents

.text
.type os_list, @function
# In: edi = a folder opened by os_open with mode 4, rsi = the buffer, rdx = its size.
# Out: rax = how many bytes of entries it put in the buffer, 0 at the end, or an error. An entry
# is a 2-byte length, a multiple of 8 that starts the next, a byte for the kind (0 a file, 1 a
# folder, 2 other) and a byte for the name's length, then the name and a NUL, padded with zeros;
# `.` and `..` are left out, and the order is the system's.
os_list:
    cmp edi, LENGUA_CHANNEL
    je .Llengua_list_refused
    push rbx
    push rsi
    push rdx
    push r8
    push r9
    push r10
    mov r10, rsi
    mov r9, rsi
    lea rsi, [rip + lengua_dirents]
    cmp rdx, 4096
    jbe .Llengua_list_read
    mov edx, 4096
.Llengua_list_read:
    mov eax, LENGUA_SYS_GETDENTS64
    syscall
    test rax, rax
    js .Llengua_list_failed
    jz .Llengua_list_done
    lea r8, [rsi + rax]
    mov rbx, rsi
.Llengua_list_entry:  # bound: 171
    cmp rbx, r8
    jae .Llengua_list_batch
    cmp byte ptr [rbx + 19], 46
    jne .Llengua_list_keep
    cmp byte ptr [rbx + 20], 0
    je .Llengua_list_next
    cmp byte ptr [rbx + 20], 46
    jne .Llengua_list_keep
    cmp byte ptr [rbx + 21], 0
    je .Llengua_list_next
.Llengua_list_keep:
    xor ecx, ecx
.Llengua_list_measure:  # bound: 255
    cmp byte ptr [rbx + rcx + 19], 0
    je .Llengua_list_measured
    inc ecx
    jmp .Llengua_list_measure
.Llengua_list_measured:
    movzx eax, byte ptr [rbx + 18]
    cmp eax, 8
    je .Llengua_list_file
    cmp eax, 4
    je .Llengua_list_folder
    mov eax, LENGUA_KIND_OTHER
    jmp .Llengua_list_kind
.Llengua_list_file:
    mov eax, LENGUA_KIND_FILE
    jmp .Llengua_list_kind
.Llengua_list_folder:
    mov eax, LENGUA_KIND_FOLDER
.Llengua_list_kind:
    mov byte ptr [r9 + 2], al
    mov byte ptr [r9 + 3], cl
    xor r11d, r11d
.Llengua_list_copy:  # bound: 256
    movzx eax, byte ptr [rbx + r11 + 19]
    mov byte ptr [r9 + r11 + 4], al
    inc r11d
    cmp r11d, ecx
    jbe .Llengua_list_copy
    lea eax, [rcx + 12]
    and eax, -8
    mov word ptr [r9], ax
    add r11d, 4
.Llengua_list_pad:  # bound: 8
    cmp r11d, eax
    jae .Llengua_list_padded
    mov byte ptr [r9 + r11], 0
    inc r11d
    jmp .Llengua_list_pad
.Llengua_list_padded:
    add r9, rax
.Llengua_list_next:
    movzx eax, word ptr [rbx + 16]
    add rbx, rax
    jmp .Llengua_list_entry
.Llengua_list_batch:
    cmp r9, r10
    je .Llengua_list_read
.Llengua_list_done:
    mov rax, r9
    sub rax, r10
    pop r10
    pop r9
    pop r8
    pop rdx
    pop rsi
    pop rbx
    ret
.Llengua_list_refused:
    mov rax, LENGUA_EBADF
    ret
.Llengua_list_failed:
    pop r10
    pop r9
    pop r8
    pop rdx
    pop rsi
    pop rbx
    lea r11, [rip + os_list_errors]
    call lengua_error
    ret
.size os_list, . - os_list

# == os_terminal: lengua_error
.section .bss
.balign 8
.type lengua_terminal, @object
# The terminal's mode as the program found it, 36 bytes, then a byte that is 1 once it is kept.
lengua_terminal:
    .zero 40
.size lengua_terminal, . - lengua_terminal

.text
.type os_terminal, @function
# In: edi = 1 to take keys as they are pressed, with no echo and no line editing (signals and
# output processing stay as they are), or 0 to put the mode back as it was found.
# Out: rax = 0, or -ENOTTY where standard input is not a terminal or there is no mode to put back.
os_terminal:
    push rdi
    push rsi
    push rdx
    sub rsp, 40
    test edi, edi
    jz .Llengua_terminal_restore
    mov esi, LENGUA_TCGETS
    xor edi, edi
    mov rdx, rsp
    mov eax, LENGUA_SYS_IOCTL
    syscall
    test rax, rax
    js .Llengua_terminal_done
    lea r11, [rip + lengua_terminal]
    cmp byte ptr [r11 + 36], 0
    jne .Llengua_terminal_set
    mov rax, qword ptr [rsp]
    mov qword ptr [r11], rax
    mov rax, qword ptr [rsp + 8]
    mov qword ptr [r11 + 8], rax
    mov rax, qword ptr [rsp + 16]
    mov qword ptr [r11 + 16], rax
    mov rax, qword ptr [rsp + 24]
    mov qword ptr [r11 + 24], rax
    mov eax, dword ptr [rsp + 32]
    mov dword ptr [r11 + 32], eax
    mov byte ptr [r11 + 36], 1
.Llengua_terminal_set:
    and dword ptr [rsp + 12], LENGUA_NOT_LINE_MODE
    mov byte ptr [rsp + 22], 0
    mov byte ptr [rsp + 23], 1
    mov esi, LENGUA_TCSETS
    xor edi, edi
    mov rdx, rsp
    mov eax, LENGUA_SYS_IOCTL
    syscall
    jmp .Llengua_terminal_done
.Llengua_terminal_restore:
    lea r11, [rip + lengua_terminal]
    mov rax, LENGUA_ENOTTY
    cmp byte ptr [r11 + 36], 0
    je .Llengua_terminal_done
    mov esi, LENGUA_TCSETS
    xor edi, edi
    mov rdx, r11
    mov eax, LENGUA_SYS_IOCTL
    syscall
.Llengua_terminal_done:
    add rsp, 40
    pop rdx
    pop rsi
    pop rdi
    test rax, rax
    js .Llengua_terminal_failed
    ret
.Llengua_terminal_failed:
    lea r11, [rip + os_terminal_errors]
    call lengua_error
    ret
.size os_terminal, . - os_terminal

# == os_window: lengua_error
.section .rodata
.type lengua_window_order, @object
# The descriptors a window's size is asked of, in turn: output, error, input.
lengua_window_order:
    .byte 1, 2, 0
.size lengua_window_order, . - lengua_window_order

.text
.type os_window, @function
# In: rdi = 8 bytes for the rows and then the columns, 4 bytes each.
# Out: rax = 0, or -ENOTTY where none of standard output, error and input is a terminal.
os_window:
    push rbx
    push rdi
    push rsi
    push rdx
    sub rsp, 8
    xor ebx, ebx
.Llengua_window_try:  # bound: 3
    lea r11, [rip + lengua_window_order]
    movzx edi, byte ptr [r11 + rbx]
    mov esi, LENGUA_TIOCGWINSZ
    mov rdx, rsp
    mov eax, LENGUA_SYS_IOCTL
    syscall
    test rax, rax
    jns .Llengua_window_found
    inc ebx
    cmp ebx, 3
    jb .Llengua_window_try
    jmp .Llengua_window_done
.Llengua_window_found:
    movzx eax, word ptr [rsp]
    movzx ecx, word ptr [rsp + 2]
    mov r11, qword ptr [rsp + 24]
    mov dword ptr [r11], eax
    mov dword ptr [r11 + 4], ecx
    xor eax, eax
.Llengua_window_done:
    add rsp, 8
    pop rdx
    pop rsi
    pop rdi
    pop rbx
    test rax, rax
    js .Llengua_window_failed
    ret
.Llengua_window_failed:
    lea r11, [rip + os_window_errors]
    call lengua_error
    ret
.size os_window, . - os_window

# == lengua_threads
.section .bss
.balign 64
.type lengua_stacks, @object
# The stacks of the threads os_thread_start starts: 64 of 1 MiB, one for each number, the top of
# each being where the next begins. Only the pages a thread touches are ever there.
lengua_stacks:
    .zero 67108864
.size lengua_stacks, . - lengua_stacks

.balign 4
.type lengua_tids, @object
# Each thread's id, which the kernel sets when it starts and clears, waking a waiter, when it ends.
lengua_tids:
    .zero 256
.size lengua_tids, . - lengua_tids

.balign 8
.type lengua_results, @object
# What each thread's worker returned.
lengua_results:
    .zero 512
.size lengua_results, . - lengua_results

# == os_thread_start: lengua_error lengua_threads
.text
.type os_thread_start, @function
# In: edi = the thread's number, from 1 to 63; rsi = its slot. The thread runs worker(number,
# slot), which is main.s's, on a stack of its own, and keeps what it returns for os_thread_join.
# Out: rax = 0 once it is running, or an error.
os_thread_start:
    lea eax, [rdi - 1]
    cmp eax, 62
    ja .Llengua_thread_start_invalid
    push rbx
    push r12
    push rdi
    push rsi
    push rdx
    push r10
    push r8
    mov ebx, edi
    mov r12, rsi
    lea rsi, [rip + lengua_stacks]
    lea rax, [rbx + 1]
    shl rax, 20
    add rsi, rax
    lea rdx, [rip + lengua_tids]
    lea rdx, [rdx + 4*rbx]
    mov r10, rdx
    xor r8d, r8d
    mov edi, LENGUA_CLONE_FLAGS
    mov eax, LENGUA_SYS_CLONE
    syscall
    test rax, rax
    js .Llengua_thread_start_failed
    jz .Llengua_thread_child
    pop r8
    pop r10
    pop rdx
    pop rsi
    pop rdi
    pop r12
    pop rbx
    xor eax, eax
    ret
.Llengua_thread_child:
    mov edi, ebx
    mov rsi, r12
    call worker
    lea rcx, [rip + lengua_results]
    mov qword ptr [rcx + 8*rbx], rax
    xor edi, edi
    mov eax, LENGUA_SYS_EXIT
    syscall
    ud2
.Llengua_thread_start_invalid:
    mov rax, LENGUA_EINVAL
    ret
.Llengua_thread_start_failed:
    pop r8
    pop r10
    pop rdx
    pop rsi
    pop rdi
    pop r12
    pop rbx
    lea r11, [rip + os_thread_start_errors]
    call lengua_error
    ret
.size os_thread_start, . - os_thread_start

# == os_thread_join: lengua_threads
.text
.type os_thread_join, @function
# In: edi = a thread's number, as os_thread_start was given it.
# Out: rax = what the thread's worker returned, once it has ended; -EINVAL for a number no thread
# has, or one that does not end.
os_thread_join:
    lea eax, [rdi - 1]
    cmp eax, 62
    ja .Llengua_thread_join_invalid
    push rbx
    push rdi
    push rsi
    push rdx
    push r10
    push r9
    mov ebx, edi
    lea rdi, [rip + lengua_tids]
    lea rdi, [rdi + 4*rbx]
    mov r9d, LENGUA_MAX_WAITS
.Llengua_thread_join_wait:  # bound: 64
    mov edx, dword ptr [rdi]
    test edx, edx
    jz .Llengua_thread_join_done
    xor esi, esi
    xor r10d, r10d
    mov eax, LENGUA_SYS_FUTEX
    syscall
    dec r9d
    jnz .Llengua_thread_join_wait
    mov rax, LENGUA_EINVAL
    jmp .Llengua_thread_join_leave
.Llengua_thread_join_done:
    lea rcx, [rip + lengua_results]
    mov rax, qword ptr [rcx + 8*rbx]
.Llengua_thread_join_leave:
    pop r9
    pop r10
    pop rdx
    pop rsi
    pop rdi
    pop rbx
    ret
.Llengua_thread_join_invalid:
    mov rax, LENGUA_EINVAL
    ret
.size os_thread_join, . - os_thread_join
