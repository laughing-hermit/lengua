.intel_syntax noprefix

# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
# os_terminal's modes: keys one at a time with no echo and no line editing, and the mode the
# terminal was found in.
.equ KEYS, 1
.equ BACK, 0
# os_poll on standard input: one entry, waiting to read.
.equ POLLIN, 1
.equ WAITING, 1
.equ TICK_MS, 100
# What next_key returns when no key came in time, and at the end of the input.
.equ NO_KEY_YET, -2
.equ END, -1
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 2
# Keys
.equ QUIT, 113
.equ LOWER_A, 97
.equ LETTERS, 26
.equ TO_CAPITAL, 32
.equ BYTE_MAX, 255
.equ KEY_LEN, 1
.equ OUT_LEN, 1
.equ HIDE_LEN, 6
.equ SHOW_LEN, 6
# The keys and waits the main loop takes at most.
.equ MAX_TURNS, 1099511627776

.section .rodata

# Escape sequences that hide the cursor and show it again.
.type hide_cursor, @object
hide_cursor:
    .ascii "\033[?25l"
.size hide_cursor, . - hide_cursor

.type show_cursor, @object
show_cursor:
    .ascii "\033[?25h"
.size show_cursor, . - show_cursor

.section .data

# What poll waits on: standard input, to read.
.type waiting, @object
waiting:
    .long STDIN
    .short POLLIN, 0
.size waiting, . - waiting

.section .bss

# The key read.
.type key, @object
key:
    .zero KEY_LEN
.size key, . - key

# The key shown.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text

# Shows each key as it's pressed, in capitals, with the cursor hidden, until q or the end of the
# input; in a terminal, keys come one at a time with no echo, and its mode is set back at the end.
# lengua's `_start` calls main, which never returns.
.type main, @function
main:
    sub rsp, 8
    mov edi, KEYS
    call os_terminal
    test rax, rax
    js .Lmain_hide
.Lmain_hide:
    mov edi, STDOUT
    lea rsi, [rip + hide_cursor]
    mov edx, HIDE_LEN
    call os_write
    test rax, rax
    js .Lmain_failed
    cmp rax, HIDE_LEN
    ja .Lmain_assert_hide
    mov rbx, MAX_TURNS
.Lmain_turn:  # bound: 1099511627776
    call next_key
    test rax, rax
    js .Lmain_no_key
    cmp eax, QUIT
    je .Lmain_done
    mov edi, eax
    call show_key
    test rax, rax
    js .Lmain_failed
    jmp .Lmain_next
.Lmain_no_key:
    cmp rax, NO_KEY_YET
    jne .Lmain_done
.Lmain_next:
    dec rbx
    jnz .Lmain_turn
.Lmain_done:
    mov edi, STDOUT
    lea rsi, [rip + show_cursor]
    mov edx, SHOW_LEN
    call os_write
    test rax, rax
    js .Lmain_failed
    cmp rax, SHOW_LEN
    ja .Lmain_assert_show
    mov edi, BACK
    call os_terminal
    test rax, rax
    js .Lmain_ok
.Lmain_ok:
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_failed:
    mov edi, BACK
    call os_terminal
    test rax, rax
    js .Lmain_io_failed
.Lmain_io_failed:
    mov edi, EXIT_IO_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_hide:
    mov edi, 1
    call assert_fail
.Lmain_assert_show:
    mov edi, 2
    call assert_fail
.size main, . - main

# Shows a key on standard output, a letter in capitals.
# In: edi = the key, 0 to 255.
# Out: rax = 0, or a negative error code.
# returns: result
.type show_key, @function
show_key:
    sub rsp, 8
    cmp edi, BYTE_MAX
    ja .Lshow_key_assert_key
    # A letter becomes its capital without a branch, so one path covers every key.
    lea ecx, [rdi - LOWER_A]
    lea eax, [rdi - TO_CAPITAL]
    cmp ecx, LETTERS
    cmovb edi, eax
    mov byte ptr [rip + out], dil
    mov edi, STDOUT
    lea rsi, [rip + out]
    mov edx, OUT_LEN
    call os_write
    test rax, rax
    js .Lshow_key_return
    cmp rax, OUT_LEN
    ja .Lshow_key_assert_count
    xor eax, eax
.Lshow_key_return:
    add rsp, 8
    ret
.Lshow_key_assert_key:
    mov edi, 3
    call assert_fail
.Lshow_key_assert_count:
    mov edi, 4
    call assert_fail
.size show_key, . - show_key

# Waits up to TICK_MS milliseconds for the next key and reads it.
# Out: rax = the key, 0 to 255; NO_KEY_YET when none came in time; END at the end of the input
# or when it can't be read.
# returns: result
.type next_key, @function
next_key:
    sub rsp, 8
    lea rdi, [rip + waiting]
    mov esi, WAITING
    mov edx, TICK_MS
    call os_poll
    test rax, rax
    js .Lnext_key_end
    jz .Lnext_key_none
    cmp rax, WAITING
    ja .Lnext_key_assert_ready
    mov edi, STDIN
    lea rsi, [rip + key]
    mov edx, KEY_LEN
    call os_read
    test rax, rax
    jle .Lnext_key_end
    cmp rax, KEY_LEN
    ja .Lnext_key_assert_count
    movzx eax, byte ptr [rip + key]
    jmp .Lnext_key_return
.Lnext_key_none:
    mov rax, NO_KEY_YET
    jmp .Lnext_key_return
.Lnext_key_end:
    mov rax, END
.Lnext_key_return:
    add rsp, 8
    ret
.Lnext_key_assert_ready:
    mov edi, 5
    call assert_fail
.Lnext_key_assert_count:
    mov edi, 6
    call assert_fail
.size next_key, . - next_key
