.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_READ, 63
.equ SYS_WRITE, 64
.equ SYS_PPOLL, 73
.equ SYS_IOCTL, 29
.equ SYS_EXIT, 93
// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// The terminal's mode, as the kernel lays out struct termios in 36 bytes: its local flags at 12,
// and among its control characters, from 17, VTIME at 22 and VMIN at 23.
.equ TCGETS, 0x5401
.equ TCSETS, 0x5402
.equ TERMIOS_LEN, 36
.equ LOCAL_FLAGS, 12
.equ VTIME_AT, 22
.equ VMIN_AT, 23
// Every local flag but ICANON (2) and ECHO (8): no line editing, no echo.
.equ KEEP_FLAGS, -11
.equ ONE_KEY, 1
.equ NO_WAIT, 0
.equ CHANGED, 1
.equ UNCHANGED, 0
.equ CHANGED_LEN, 8
// ppoll on standard input: one entry, waiting to read, a tenth of a second at most, given as
// seconds and nanoseconds, 8 bytes each.
.equ POLLIN, 1
.equ WAITING, 1
.equ TICK_NS, 100000000
.equ TIMESPEC_LEN, 16
// What next_key returns when no key came in time, and at the end of the input.
.equ NO_KEY_YET, -2
.equ END, -1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 2
// Keys
.equ QUIT, 113
.equ LOWER_A, 97
.equ LETTERS, 26
.equ TO_CAPITAL, 32
.equ BYTE_MAX, 255
.equ KEY_LEN, 1
.equ OUT_LEN, 1
.equ HIDE_LEN, 6
.equ SHOW_LEN, 6
// The keys and waits the main loop takes at most.
.equ MAX_TURNS, 1099511627776

.section .rodata

// Escape sequences that hide the cursor and show it again.
.type hide_cursor, @object
hide_cursor:
    .ascii "\033[?25l"
.size hide_cursor, . - hide_cursor

.type show_cursor, @object
show_cursor:
    .ascii "\033[?25h"
.size show_cursor, . - show_cursor

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .data

// What ppoll waits on: standard input, to read.
.type waiting, @object
waiting:
    .long STDIN
    .short POLLIN, 0
.size waiting, . - waiting

.section .bss

// How long ppoll waits for a key: no seconds and TICK_NS nanoseconds, x86-64's 100 ms. The kernel
// leaves the time left in it, so next_key sets it again before each wait.
// shared: next_key sets it, and ppoll reads it and writes what's left
.type tick, @object
tick:
    .zero TIMESPEC_LEN
.size tick, . - tick

// The terminal's mode as the program found it.
// shared: key_mode_on saves it, and key_mode_off sets it back
.type saved_mode, @object
saved_mode:
    .zero TERMIOS_LEN
.size saved_mode, . - saved_mode

// The mode the program reads keys in.
.type key_mode, @object
key_mode:
    .zero TERMIOS_LEN
.size key_mode, . - key_mode

// CHANGED once key_mode_on has changed the terminal's mode.
// shared: key_mode_on sets it, and key_mode_off sets the mode back only when it's set
.type changed, @object
changed:
    .zero CHANGED_LEN
.size changed, . - changed

// The key read.
.type key, @object
key:
    .zero KEY_LEN
.size key, . - key

// The key shown.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
.globl _start

// Shows each key as it's pressed, in capitals, with the cursor hidden, until q or the end of the
// input; in a terminal, keys come one at a time with no echo, and its mode is set back at the end.
.type _start, @function
_start:
    bl key_mode_on
    mov x0, #STDOUT
    adrp x1, hide_cursor
    add x1, x1, :lo12:hide_cursor
    mov x2, #HIDE_LEN
    mov x8, #SYS_WRITE
    svc #0
    tbnz x0, #63, .Lstart_failed
    cmp x0, #HIDE_LEN
    b.hi .Lstart_assert_hide
    mov x19, #MAX_TURNS
.Lstart_turn:  // bound: 1099511627776
    bl next_key
    tbnz x0, #63, .Lstart_no_key
    cmp w0, #QUIT
    b.eq .Lstart_done
    bl show_key
    tbnz x0, #63, .Lstart_failed
    b .Lstart_next
.Lstart_no_key:
    cmn x0, #2
    b.ne .Lstart_done
.Lstart_next:
    subs x19, x19, #1
    b.ne .Lstart_turn
.Lstart_done:
    mov x0, #STDOUT
    adrp x1, show_cursor
    add x1, x1, :lo12:show_cursor
    mov x2, #SHOW_LEN
    mov x8, #SYS_WRITE
    svc #0
    tbnz x0, #63, .Lstart_failed
    cmp x0, #SHOW_LEN
    b.hi .Lstart_assert_show
    bl key_mode_off
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_failed:
    bl key_mode_off
    mov w0, #EXIT_IO_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_hide:
    mov w0, #1
    bl assert_fail
.Lstart_assert_show:
    mov w0, #2
    bl assert_fail
.size _start, . - _start

// Shows a key on standard output, a letter in capitals.
// In: w0 = the key, 0 to 255.
// Out: x0 = 0, or a negative error code.
// returns: result
.type show_key, @function
show_key:
    cmp w0, #BYTE_MAX
    b.hi .Lshow_key_assert_key
    // A letter becomes its capital without a branch, so one path covers every key.
    sub w9, w0, #LOWER_A
    sub w10, w0, #TO_CAPITAL
    cmp w9, #LETTERS
    csel w0, w10, w0, lo
    adrp x1, out
    add x1, x1, :lo12:out
    strb w0, [x1]
    mov x0, #STDOUT
    mov x2, #OUT_LEN
    mov x8, #SYS_WRITE
    svc #0
    tbnz x0, #63, .Lshow_key_return
    cmp x0, #OUT_LEN
    b.hi .Lshow_key_assert_count
    mov x0, #0
.Lshow_key_return:
    ret
.Lshow_key_assert_key:
    mov w0, #3
    bl assert_fail
.Lshow_key_assert_count:
    mov w0, #4
    bl assert_fail
.size show_key, . - show_key

// Switches the terminal, when standard input is one, to keys one at a time with no echo, saving
// the mode it had. Otherwise, as from a pipe, keys come as they are, and it changes nothing.
.type key_mode_on, @function
key_mode_on:
    mov x0, #STDIN
    mov x1, #TCGETS
    adrp x2, saved_mode
    add x2, x2, :lo12:saved_mode
    mov x8, #SYS_IOCTL
    svc #0
    tbnz x0, #63, .Lkey_mode_on_return
    cbnz x0, .Lkey_mode_on_assert_saved
    mov x0, #STDIN
    mov x1, #TCGETS
    adrp x2, key_mode
    add x2, x2, :lo12:key_mode
    mov x8, #SYS_IOCTL
    svc #0
    tbnz x0, #63, .Lkey_mode_on_return
    cbnz x0, .Lkey_mode_on_assert_read
    ldr w9, [x2, #LOCAL_FLAGS]
    mov w10, #KEEP_FLAGS
    and w9, w9, w10
    str w9, [x2, #LOCAL_FLAGS]
    mov w9, #ONE_KEY
    strb w9, [x2, #VMIN_AT]
    mov w9, #NO_WAIT
    strb w9, [x2, #VTIME_AT]
    mov x0, #STDIN
    mov x1, #TCSETS
    mov x8, #SYS_IOCTL
    svc #0
    tbnz x0, #63, .Lkey_mode_on_return
    cbnz x0, .Lkey_mode_on_assert_set
    adrp x9, changed
    add x9, x9, :lo12:changed
    mov x10, #CHANGED
    str x10, [x9]
.Lkey_mode_on_return:
    ret
.Lkey_mode_on_assert_saved:
    mov w0, #5
    bl assert_fail
.Lkey_mode_on_assert_read:
    mov w0, #6
    bl assert_fail
.Lkey_mode_on_assert_set:
    mov w0, #7
    bl assert_fail
.size key_mode_on, . - key_mode_on

// Sets the terminal back to the mode key_mode_on found, when it changed it.
.type key_mode_off, @function
key_mode_off:
    adrp x9, changed
    add x9, x9, :lo12:changed
    ldr x10, [x9]
    cmp x10, #CHANGED
    b.ne .Lkey_mode_off_return
    mov x0, #STDIN
    mov x1, #TCSETS
    adrp x2, saved_mode
    add x2, x2, :lo12:saved_mode
    mov x8, #SYS_IOCTL
    svc #0
    tbnz x0, #63, .Lkey_mode_off_return
    cbnz x0, .Lkey_mode_off_assert_result
    mov x10, #UNCHANGED
    str x10, [x9]
.Lkey_mode_off_return:
    ret
.Lkey_mode_off_assert_result:
    mov w0, #8
    bl assert_fail
.size key_mode_off, . - key_mode_off

// Waits up to TICK_NS nanoseconds for the next key and reads it.
// Out: x0 = the key, 0 to 255; NO_KEY_YET when none came in time; END at the end of the input
// or when it can't be read.
// returns: result
.type next_key, @function
next_key:
    adrp x0, waiting
    add x0, x0, :lo12:waiting
    mov x1, #WAITING
    adrp x2, tick
    add x2, x2, :lo12:tick
    mov x9, #TICK_NS & 0xffff
    movk x9, #TICK_NS >> 16, lsl #16
    str xzr, [x2]
    str x9, [x2, #8]
    mov x3, #0
    mov x8, #SYS_PPOLL
    svc #0
    tbnz x0, #63, .Lnext_key_end
    cbz x0, .Lnext_key_none
    cmp x0, #WAITING
    b.hi .Lnext_key_assert_ready
    mov x0, #STDIN
    adrp x1, key
    add x1, x1, :lo12:key
    mov x2, #KEY_LEN
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lnext_key_end
    cbz x0, .Lnext_key_end
    cmp x0, #KEY_LEN
    b.hi .Lnext_key_assert_count
    ldrb w0, [x1]
    b .Lnext_key_return
.Lnext_key_none:
    mov x0, #NO_KEY_YET
    b .Lnext_key_return
.Lnext_key_end:
    mov x0, #END
.Lnext_key_return:
    ret
.Lnext_key_assert_ready:
    mov w0, #9
    bl assert_fail
.Lnext_key_assert_count:
    mov w0, #10
    bl assert_fail
.size next_key, . - next_key

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
