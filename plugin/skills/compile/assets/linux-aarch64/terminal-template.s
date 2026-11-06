.arch armv8-a

// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// os_terminal's modes: keys one at a time with no echo and no line editing, and the mode the
// terminal was found in.
.equ KEYS, 1
.equ BACK, 0
// os_poll on standard input: one entry, waiting to read, a tenth of a second at most.
.equ POLLIN, 1
.equ WAITING, 1
.equ TICK_MS, 100
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

.section .data

// What poll waits on: standard input, to read.
.type waiting, @object
waiting:
    .long STDIN
    .short POLLIN, 0
.size waiting, . - waiting

.section .bss

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

// Shows each key as it's pressed, in capitals, with the cursor hidden, until q or the end of the
// input; in a terminal, keys come one at a time with no echo, and its mode is set back at the end.
// lengua's `_start` calls main, which never returns.
.type main, @function
main:
    mov x0, #KEYS
    bl os_terminal
    tbnz x0, #63, .Lmain_hide
.Lmain_hide:
    mov x0, #STDOUT
    adrp x1, hide_cursor
    add x1, x1, :lo12:hide_cursor
    mov x2, #HIDE_LEN
    bl os_write
    tbnz x0, #63, .Lmain_failed
    cmp x0, #HIDE_LEN
    b.hi .Lmain_assert_hide
    mov x19, #MAX_TURNS
.Lmain_turn:  // bound: 1099511627776
    bl next_key
    tbnz x0, #63, .Lmain_no_key
    cmp w0, #QUIT
    b.eq .Lmain_done
    bl show_key
    tbnz x0, #63, .Lmain_failed
    b .Lmain_next
.Lmain_no_key:
    cmn x0, #2
    b.ne .Lmain_done
.Lmain_next:
    subs x19, x19, #1
    b.ne .Lmain_turn
.Lmain_done:
    mov x0, #STDOUT
    adrp x1, show_cursor
    add x1, x1, :lo12:show_cursor
    mov x2, #SHOW_LEN
    bl os_write
    tbnz x0, #63, .Lmain_failed
    cmp x0, #SHOW_LEN
    b.hi .Lmain_assert_show
    mov x0, #BACK
    bl os_terminal
    tbnz x0, #63, .Lmain_ok
.Lmain_ok:
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_failed:
    mov x0, #BACK
    bl os_terminal
    tbnz x0, #63, .Lmain_io_failed
.Lmain_io_failed:
    mov w0, #EXIT_IO_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_hide:
    mov w0, #1
    bl assert_fail
.Lmain_assert_show:
    mov w0, #2
    bl assert_fail
.size main, . - main

// Shows a key on standard output, a letter in capitals.
// In: w0 = the key, 0 to 255.
// Out: x0 = 0, or a negative error code.
// returns: result
.type show_key, @function
show_key:
    stp x29, x30, [sp, #-16]!
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
    bl os_write
    tbnz x0, #63, .Lshow_key_return
    cmp x0, #OUT_LEN
    b.hi .Lshow_key_assert_count
    mov x0, #0
.Lshow_key_return:
    ldp x29, x30, [sp], #16
    ret
.Lshow_key_assert_key:
    mov w0, #3
    bl assert_fail
.Lshow_key_assert_count:
    mov w0, #4
    bl assert_fail
.size show_key, . - show_key

// Waits up to TICK_MS milliseconds for the next key and reads it.
// Out: x0 = the key, 0 to 255; NO_KEY_YET when none came in time; END at the end of the input
// or when it can't be read.
// returns: result
.type next_key, @function
next_key:
    stp x29, x30, [sp, #-16]!
    adrp x0, waiting
    add x0, x0, :lo12:waiting
    mov x1, #WAITING
    mov x2, #TICK_MS
    bl os_poll
    tbnz x0, #63, .Lnext_key_end
    cbz x0, .Lnext_key_none
    cmp x0, #WAITING
    b.hi .Lnext_key_assert_ready
    mov x0, #STDIN
    adrp x1, key
    add x1, x1, :lo12:key
    mov x2, #KEY_LEN
    bl os_read
    tbnz x0, #63, .Lnext_key_end
    cbz x0, .Lnext_key_end
    cmp x0, #KEY_LEN
    b.hi .Lnext_key_assert_count
    adrp x1, key
    add x1, x1, :lo12:key
    ldrb w0, [x1]
    b .Lnext_key_return
.Lnext_key_none:
    mov x0, #NO_KEY_YET
    b .Lnext_key_return
.Lnext_key_end:
    mov x0, #END
.Lnext_key_return:
    ldp x29, x30, [sp], #16
    ret
.Lnext_key_assert_ready:
    mov w0, #5
    bl assert_fail
.Lnext_key_assert_count:
    mov w0, #6
    bl assert_fail
.size next_key, . - next_key
