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

.equ LENGUA_SYS_MKDIRAT, 34
.equ LENGUA_SYS_UNLINKAT, 35
.equ LENGUA_SYS_RENAMEAT, 38
.equ LENGUA_SYS_OPENAT, 56
.equ LENGUA_SYS_CLOSE, 57
.equ LENGUA_SYS_GETDENTS64, 61
.equ LENGUA_SYS_LSEEK, 62
.equ LENGUA_SYS_READ, 63
.equ LENGUA_SYS_WRITE, 64
.equ LENGUA_SYS_PPOLL, 73
.equ LENGUA_SYS_FSTAT, 80
.equ LENGUA_SYS_EXIT_GROUP, 94
.equ LENGUA_SYS_CLOCK_GETTIME, 113
.equ LENGUA_SYS_CLOCK_NANOSLEEP, 115
.equ LENGUA_SYS_SHUTDOWN, 210
.equ LENGUA_SYS_ACCEPT4, 242
.equ LENGUA_SYS_GETRANDOM, 278
.equ LENGUA_AT_FDCWD, -100
.equ LENGUA_AT_REMOVEDIR, 512
.equ LENGUA_FOLDER_MODE, 493
.equ LENGUA_FILE_MODE, 420
.equ LENGUA_KIND_FILE, 0
.equ LENGUA_KIND_FOLDER, 1
.equ LENGUA_KIND_OTHER, 2
.equ LENGUA_CHANNEL, 4352
.equ LENGUA_LISTENER, 3
.equ LENGUA_RANDOM_MOST, 256
.equ LENGUA_EBADF, -9
.equ LENGUA_EIO, -5
.equ LENGUA_EINVAL, -22

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

// == os_read: lengua_error
.text
.type os_read, @function
// In: w0 = the descriptor, x1 = the buffer, x2 = its size.
// Out: x0 = how many bytes it read, 0 at the end of the input, or an error.
os_read:
    mov w16, #LENGUA_CHANNEL
    cmp w0, w16
    b.eq .Llengua_read_refused
    mov x8, #LENGUA_SYS_READ
    svc #0
    tbnz x0, #63, .Llengua_read_failed
    ret
.Llengua_read_refused:
    mov x0, #LENGUA_EBADF
    ret
.Llengua_read_failed:
    adrp x17, os_read_errors
    add x17, x17, :lo12:os_read_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_read, . - os_read

// == os_close: lengua_error
.text
.type os_close, @function
// In: w0 = the descriptor.
// Out: x0 = 0, or an error.
os_close:
    mov w16, #LENGUA_CHANNEL
    cmp w0, w16
    b.eq .Llengua_close_refused
    mov x8, #LENGUA_SYS_CLOSE
    svc #0
    tbnz x0, #63, .Llengua_close_failed
    ret
.Llengua_close_refused:
    mov x0, #LENGUA_EBADF
    ret
.Llengua_close_failed:
    adrp x17, os_close_errors
    add x17, x17, :lo12:os_close_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_close, . - os_close

// == os_seek: lengua_error
.text
.type os_seek, @function
// In: w0 = the descriptor, x1 = the offset, w2 = where it counts from (0 the start, 1 here, 2
// the end).
// Out: x0 = the new offset, or an error.
os_seek:
    mov w16, #LENGUA_CHANNEL
    cmp w0, w16
    b.eq .Llengua_seek_refused
    mov x8, #LENGUA_SYS_LSEEK
    svc #0
    tbnz x0, #63, .Llengua_seek_failed
    ret
.Llengua_seek_refused:
    mov x0, #LENGUA_EBADF
    ret
.Llengua_seek_failed:
    adrp x17, os_seek_errors
    add x17, x17, :lo12:os_seek_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_seek, . - os_seek

// == os_mkdir: lengua_error
.text
.type os_mkdir, @function
// In: x0 = the name, NUL-terminated, relative to the working folder.
// Out: x0 = 0, or an error.
os_mkdir:
    mov x16, x0
    stp x1, x2, [sp, #-16]!
    mov x1, x16
    mov x0, #LENGUA_AT_FDCWD
    mov x2, #LENGUA_FOLDER_MODE
    mov x8, #LENGUA_SYS_MKDIRAT
    svc #0
    ldp x1, x2, [sp], #16
    tbnz x0, #63, .Llengua_mkdir_failed
    ret
.Llengua_mkdir_failed:
    adrp x17, os_mkdir_errors
    add x17, x17, :lo12:os_mkdir_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_mkdir, . - os_mkdir

// == os_remove: lengua_error
.text
.type os_remove, @function
// In: x0 = the name, w1 = 0 for a file or 1 for an empty folder.
// Out: x0 = 0, or an error.
os_remove:
    cmp w1, #1
    b.hi .Llengua_remove_invalid
    mov x16, x0
    mov x17, #LENGUA_AT_REMOVEDIR
    stp x1, x2, [sp, #-16]!
    cmp w1, #1
    csel x2, x17, xzr, eq
    mov x1, x16
    mov x0, #LENGUA_AT_FDCWD
    mov x8, #LENGUA_SYS_UNLINKAT
    svc #0
    ldp x1, x2, [sp], #16
    tbnz x0, #63, .Llengua_remove_failed
    ret
.Llengua_remove_invalid:
    mov x0, #LENGUA_EINVAL
    ret
.Llengua_remove_failed:
    adrp x17, os_remove_errors
    add x17, x17, :lo12:os_remove_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_remove, . - os_remove

// == os_rename: lengua_error
.text
.type os_rename, @function
// In: x0 = the name it has, x1 = the name it is to have, which is replaced.
// Out: x0 = 0, or an error.
os_rename:
    mov x16, x0
    mov x17, x1
    stp x1, x2, [sp, #-16]!
    str x3, [sp, #-16]!
    mov x3, x17
    mov x1, x16
    mov x0, #LENGUA_AT_FDCWD
    mov x2, #LENGUA_AT_FDCWD
    mov x8, #LENGUA_SYS_RENAMEAT
    svc #0
    ldr x3, [sp], #16
    ldp x1, x2, [sp], #16
    tbnz x0, #63, .Llengua_rename_failed
    ret
.Llengua_rename_failed:
    adrp x17, os_rename_errors
    add x17, x17, :lo12:os_rename_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_rename, . - os_rename

// == os_clock: lengua_error
.text
.type os_clock, @function
// In: w0 = 0 for the time of day or 1 for a clock that only moves forward, x1 = 16 bytes for the
// seconds and then the nanoseconds.
// Out: x0 = 0, or an error.
os_clock:
    cmp w0, #1
    b.hi .Llengua_clock_invalid
    mov x8, #LENGUA_SYS_CLOCK_GETTIME
    svc #0
    tbnz x0, #63, .Llengua_clock_failed
    ret
.Llengua_clock_invalid:
    mov x0, #LENGUA_EINVAL
    ret
.Llengua_clock_failed:
    adrp x17, os_clock_errors
    add x17, x17, :lo12:os_clock_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_clock, . - os_clock

// == os_sleep: lengua_error
.text
.type os_sleep, @function
// In: w0 = the clock as for os_clock, w1 = 0 to wait that long or 1 to wait until that time,
// x2 = 16 bytes for the seconds and then the nanoseconds.
// Out: x0 = 0, or an error.
os_sleep:
    cmp w0, #1
    b.hi .Llengua_sleep_invalid
    cmp w1, #1
    b.hi .Llengua_sleep_invalid
    str x3, [sp, #-16]!
    mov x3, #0
    mov x8, #LENGUA_SYS_CLOCK_NANOSLEEP
    svc #0
    ldr x3, [sp], #16
    tbnz x0, #63, .Llengua_sleep_failed
    ret
.Llengua_sleep_invalid:
    mov x0, #LENGUA_EINVAL
    ret
.Llengua_sleep_failed:
    adrp x17, os_sleep_errors
    add x17, x17, :lo12:os_sleep_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_sleep, . - os_sleep

// == os_random: lengua_error
.text
.type os_random, @function
// In: x0 = the buffer, x1 = how many bytes, at most 256.
// Out: x0 = how many it filled, all of them, or an error.
os_random:
    cmp x1, #LENGUA_RANDOM_MOST
    b.hi .Llengua_random_invalid
    str x2, [sp, #-16]!
    mov x2, #0
    mov x8, #LENGUA_SYS_GETRANDOM
    svc #0
    ldr x2, [sp], #16
    tbnz x0, #63, .Llengua_random_failed
    ret
.Llengua_random_invalid:
    mov x0, #LENGUA_EINVAL
    ret
.Llengua_random_failed:
    adrp x17, os_random_errors
    add x17, x17, :lo12:os_random_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_random, . - os_random

// == os_accept: lengua_error
.text
.type os_accept, @function
// In: w0 = 3, the listening socket lengua bound for the program.
// Out: x0 = the next connection's descriptor, or an error.
os_accept:
    cmp w0, #LENGUA_LISTENER
    b.ne .Llengua_accept_refused
    stp x1, x2, [sp, #-16]!
    str x3, [sp, #-16]!
    mov x1, #0
    mov x2, #0
    mov x3, #0
    mov x8, #LENGUA_SYS_ACCEPT4
    svc #0
    ldr x3, [sp], #16
    ldp x1, x2, [sp], #16
    tbnz x0, #63, .Llengua_accept_failed
    ret
.Llengua_accept_refused:
    mov x0, #LENGUA_EBADF
    ret
.Llengua_accept_failed:
    adrp x17, os_accept_errors
    add x17, x17, :lo12:os_accept_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_accept, . - os_accept

// == os_shutdown: lengua_error
.text
.type os_shutdown, @function
// In: w0 = the connection, w1 = 0 to stop receiving, 1 to stop sending or 2 for both.
// Out: x0 = 0, or an error.
os_shutdown:
    mov w16, #LENGUA_CHANNEL
    cmp w0, w16
    b.eq .Llengua_shutdown_refused
    mov x8, #LENGUA_SYS_SHUTDOWN
    svc #0
    tbnz x0, #63, .Llengua_shutdown_failed
    ret
.Llengua_shutdown_refused:
    mov x0, #LENGUA_EBADF
    ret
.Llengua_shutdown_failed:
    adrp x17, os_shutdown_errors
    add x17, x17, :lo12:os_shutdown_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_shutdown, . - os_shutdown

// == os_poll: lengua_error
.text
.type os_poll, @function
// In: x0 = a table of 8-byte entries (the descriptor, 4 bytes; the events waited for, 2; the
// events that came, 2), w1 = how many, w2 = the most milliseconds to wait, -1 for no limit.
// Out: x0 = how many entries have events, 0 when the time ran out, or an error. A negative
// descriptor is passed over, and descriptor 4,352 is refused.
os_poll:
    mov w8, #LENGUA_CHANNEL
    mov w16, #0
.Llengua_poll_scan:  // bound: 4352
    cmp w16, w1
    b.hs .Llengua_poll_scanned
    add x17, x0, x16, lsl #3
    ldr w17, [x17]
    cmp w17, w8
    b.eq .Llengua_poll_refused
    add w16, w16, #1
    b .Llengua_poll_scan
.Llengua_poll_scanned:
    sub sp, sp, #48
    stp x2, x3, [sp, #16]
    str x4, [sp, #32]
    sxtw x16, w2
    tbnz x16, #63, .Llengua_poll_forever
    mov x17, #1000
    udiv x4, x16, x17
    msub x16, x4, x17, x16
    mov x17, #0x4240
    movk x17, #0xf, lsl #16
    mul x16, x16, x17
    stp x4, x16, [sp]
    mov x2, sp
    b .Llengua_poll_wait
.Llengua_poll_forever:
    mov x2, #0
.Llengua_poll_wait:
    mov x3, #0
    mov x4, #0
    mov x8, #LENGUA_SYS_PPOLL
    svc #0
    ldp x2, x3, [sp, #16]
    ldr x4, [sp, #32]
    add sp, sp, #48
    tbnz x0, #63, .Llengua_poll_failed
    ret
.Llengua_poll_refused:
    mov x0, #LENGUA_EBADF
    ret
.Llengua_poll_failed:
    adrp x17, os_poll_errors
    add x17, x17, :lo12:os_poll_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_poll, . - os_poll

// == os_open: lengua_error
.section .rodata
.balign 4
.type lengua_open_flags, @object
// open's flags for each mode: READ, WRITE, APPEND, NEW and FOLDER (O_DIRECTORY is 0x4000 here).
lengua_open_flags:
    .long 0, 0x241, 0x441, 0xc1, 0x4000
.size lengua_open_flags, . - lengua_open_flags

.text
.type os_open, @function
// In: x0 = the name, NUL-terminated, relative to the working folder; w1 = the mode: 0 to read, 1
// to write, which makes the file and empties it, 2 to write at its end, 3 to write a file that
// is not there yet, 4 to list a folder.
// Out: x0 = a descriptor, or an error.
os_open:
    cmp w1, #4
    b.hi .Llengua_open_invalid
    mov w16, w1
    adrp x17, lengua_open_flags
    add x17, x17, :lo12:lengua_open_flags
    ldr w16, [x17, x16, lsl #2]
    mov x17, x0
    stp x1, x2, [sp, #-16]!
    str x3, [sp, #-16]!
    mov x2, x16
    mov x1, x17
    mov x0, #LENGUA_AT_FDCWD
    mov x3, #LENGUA_FILE_MODE
    mov x8, #LENGUA_SYS_OPENAT
    svc #0
    ldr x3, [sp], #16
    ldp x1, x2, [sp], #16
    tbnz x0, #63, .Llengua_open_failed
    ret
.Llengua_open_invalid:
    mov x0, #LENGUA_EINVAL
    ret
.Llengua_open_failed:
    adrp x17, os_open_errors
    add x17, x17, :lo12:os_open_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_open, . - os_open

// == os_info: lengua_error
.text
.type os_info, @function
// In: w0 = the descriptor, x1 = 16 bytes for the size and then the kind.
// Out: x0 = 0, or an error. The kind is 0 for a file, 1 for a folder and 2 for anything else,
// and the size is 0 unless it is a file.
os_info:
    mov w16, #LENGUA_CHANNEL
    cmp w0, w16
    b.eq .Llengua_info_refused
    sub sp, sp, #144
    str x1, [sp, #128]
    mov x1, sp
    mov x8, #LENGUA_SYS_FSTAT
    svc #0
    tbnz x0, #63, .Llengua_info_failed
    ldr w16, [sp, #16]
    and w16, w16, #0xf000
    mov x17, #0
    cmp w16, #0x8000
    b.ne .Llengua_info_sizeless
    ldr x17, [sp, #48]
    mov x16, #LENGUA_KIND_FILE
    b .Llengua_info_store
.Llengua_info_sizeless:
    cmp w16, #0x4000
    mov x16, #LENGUA_KIND_FOLDER
    mov x8, #LENGUA_KIND_OTHER
    csel x16, x16, x8, eq
.Llengua_info_store:
    ldr x1, [sp, #128]
    stp x17, x16, [x1]
    mov x0, #0
    add sp, sp, #144
    ret
.Llengua_info_refused:
    mov x0, #LENGUA_EBADF
    ret
.Llengua_info_failed:
    ldr x1, [sp, #128]
    add sp, sp, #144
    adrp x17, os_info_errors
    add x17, x17, :lo12:os_info_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_info, . - os_info

// == os_list: lengua_error
.section .bss
.balign 8
.type lengua_dirents, @object
// What the kernel lists, before it is put in the program's buffer in the form every system gives.
lengua_dirents:
    .zero 4096
.size lengua_dirents, . - lengua_dirents

.text
.type os_list, @function
// In: w0 = a folder opened by os_open with mode 4, x1 = the buffer, x2 = its size.
// Out: x0 = how many bytes of entries it put in the buffer, 0 at the end, or an error. An entry
// is a 2-byte length, a multiple of 8 that starts the next, a byte for the kind (0 a file, 1 a
// folder, 2 other) and a byte for the name's length, then the name and a NUL, padded with zeros;
// `.` and `..` are left out, and the order is the system's.
os_list:
    mov w16, #LENGUA_CHANNEL
    cmp w0, w16
    b.eq .Llengua_list_refused
    stp x1, x2, [sp, #-16]!
    stp x9, x10, [sp, #-16]!
    stp x11, x12, [sp, #-16]!
    stp x13, x14, [sp, #-16]!
    mov x14, x0
    mov x9, x1
    mov x10, x1
    mov x16, #4096
    cmp x2, x16
    csel x2, x2, x16, ls
.Llengua_list_read:
    mov x0, x14
    adrp x1, lengua_dirents
    add x1, x1, :lo12:lengua_dirents
    mov x8, #LENGUA_SYS_GETDENTS64
    svc #0
    tbnz x0, #63, .Llengua_list_failed
    cbz x0, .Llengua_list_done
    add x12, x1, x0
    mov x11, x1
.Llengua_list_entry:  // bound: 171
    cmp x11, x12
    b.hs .Llengua_list_batch
    ldrb w16, [x11, #19]
    cmp w16, #46
    b.ne .Llengua_list_keep
    ldrb w16, [x11, #20]
    cbz w16, .Llengua_list_next
    cmp w16, #46
    b.ne .Llengua_list_keep
    ldrb w16, [x11, #21]
    cbz w16, .Llengua_list_next
.Llengua_list_keep:
    mov x13, #0
.Llengua_list_measure:  // bound: 255
    add x16, x11, x13
    ldrb w16, [x16, #19]
    cbz w16, .Llengua_list_measured
    add x13, x13, #1
    b .Llengua_list_measure
.Llengua_list_measured:
    ldrb w16, [x11, #18]
    mov w17, #LENGUA_KIND_OTHER
    cmp w16, #8
    mov w8, #LENGUA_KIND_FILE
    csel w17, w8, w17, eq
    cmp w16, #4
    mov w8, #LENGUA_KIND_FOLDER
    csel w17, w8, w17, eq
    strb w17, [x10, #2]
    strb w13, [x10, #3]
    mov x16, #0
.Llengua_list_copy:  // bound: 256
    add x17, x11, x16
    ldrb w17, [x17, #19]
    add x8, x10, x16
    strb w17, [x8, #4]
    add x16, x16, #1
    cmp x16, x13
    b.ls .Llengua_list_copy
    add x17, x13, #12
    and x17, x17, #-8
    strh w17, [x10]
    add x16, x16, #4
.Llengua_list_pad:  // bound: 8
    cmp x16, x17
    b.hs .Llengua_list_padded
    add x8, x10, x16
    strb wzr, [x8]
    add x16, x16, #1
    b .Llengua_list_pad
.Llengua_list_padded:
    add x10, x10, x17
.Llengua_list_next:
    ldrh w16, [x11, #16]
    add x11, x11, x16
    b .Llengua_list_entry
.Llengua_list_batch:
    cmp x10, x9
    b.eq .Llengua_list_read
.Llengua_list_done:
    sub x0, x10, x9
    ldp x13, x14, [sp], #16
    ldp x11, x12, [sp], #16
    ldp x9, x10, [sp], #16
    ldp x1, x2, [sp], #16
    ret
.Llengua_list_refused:
    mov x0, #LENGUA_EBADF
    ret
.Llengua_list_failed:
    ldp x13, x14, [sp], #16
    ldp x11, x12, [sp], #16
    ldp x9, x10, [sp], #16
    ldp x1, x2, [sp], #16
    adrp x17, os_list_errors
    add x17, x17, :lo12:os_list_errors
    str x30, [sp, #-16]!
    bl lengua_error
    ldr x30, [sp], #16
    ret
.size os_list, . - os_list
