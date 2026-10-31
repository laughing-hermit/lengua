.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_READ, 0
.equ SYS_WRITE, 1
.equ SYS_POLL, 7
.equ SYS_IOCTL, 16
.equ SYS_EXIT, 60
# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
# The terminal's mode, as the kernel lays out struct termios in 36 bytes: its local flags at 12,
# and among its control characters, from 17, VTIME at 22 and VMIN at 23.
.equ TCGETS, 0x5401
.equ TCSETS, 0x5402
.equ TERMIOS_LEN, 36
.equ LOCAL_FLAGS, 12
.equ VTIME_AT, 22
.equ VMIN_AT, 23
# Every local flag but ICANON (2) and ECHO (8): no line editing, no echo.
.equ KEEP_FLAGS, -11
.equ ONE_KEY, 1
.equ NO_WAIT, 0
.equ CHANGED, 1
.equ UNCHANGED, 0
.equ CHANGED_LEN, 8
# poll on standard input: one entry, waiting to read.
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

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .data

# What poll waits on: standard input, to read.
.type waiting, @object
waiting:
    .long STDIN
    .short POLLIN, 0
.size waiting, . - waiting

.section .bss

# The terminal's mode as the program found it.
# shared: key_mode_on saves it, and key_mode_off sets it back
.type saved_mode, @object
saved_mode:
    .zero TERMIOS_LEN
.size saved_mode, . - saved_mode

# The mode the program reads keys in.
.type key_mode, @object
key_mode:
    .zero TERMIOS_LEN
.size key_mode, . - key_mode

# CHANGED once key_mode_on has changed the terminal's mode.
# shared: key_mode_on sets it, and key_mode_off sets the mode back only when it's set
.type changed, @object
changed:
    .zero CHANGED_LEN
.size changed, . - changed

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
.globl _start

# Shows each key as it's pressed, in capitals, with the cursor hidden, until q or the end of the
# input; in a terminal, keys come one at a time with no echo, and its mode is set back at the end.
.type _start, @function
_start:
    call key_mode_on
    mov eax, SYS_WRITE
    mov edi, STDOUT
    lea rsi, [rip + hide_cursor]
    mov edx, HIDE_LEN
    syscall
    test rax, rax
    js .Lstart_failed
    cmp rax, HIDE_LEN
    ja .Lstart_assert_hide
    mov rbx, MAX_TURNS
.Lstart_turn:  # bound: 1099511627776
    call next_key
    test rax, rax
    js .Lstart_no_key
    cmp eax, QUIT
    je .Lstart_done
    mov edi, eax
    call show_key
    test rax, rax
    js .Lstart_failed
    jmp .Lstart_next
.Lstart_no_key:
    cmp rax, NO_KEY_YET
    jne .Lstart_done
.Lstart_next:
    dec rbx
    jnz .Lstart_turn
.Lstart_done:
    mov eax, SYS_WRITE
    mov edi, STDOUT
    lea rsi, [rip + show_cursor]
    mov edx, SHOW_LEN
    syscall
    test rax, rax
    js .Lstart_failed
    cmp rax, SHOW_LEN
    ja .Lstart_assert_show
    call key_mode_off
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_failed:
    call key_mode_off
    mov edi, EXIT_IO_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_hide:
    mov edi, 1
    call assert_fail
.Lstart_assert_show:
    mov edi, 2
    call assert_fail
.size _start, . - _start

# Shows a key on standard output, a letter in capitals.
# In: edi = the key, 0 to 255.
# Out: rax = 0, or a negative error code.
# returns: result
.type show_key, @function
show_key:
    cmp edi, BYTE_MAX
    ja .Lshow_key_assert_key
    # A letter becomes its capital without a branch, so one path covers every key.
    lea ecx, [rdi - LOWER_A]
    lea eax, [rdi - TO_CAPITAL]
    cmp ecx, LETTERS
    cmovb edi, eax
    mov byte ptr [rip + out], dil
    mov eax, SYS_WRITE
    mov edi, STDOUT
    lea rsi, [rip + out]
    mov edx, OUT_LEN
    syscall
    test rax, rax
    js .Lshow_key_return
    cmp rax, OUT_LEN
    ja .Lshow_key_assert_count
    xor eax, eax
.Lshow_key_return:
    ret
.Lshow_key_assert_key:
    mov edi, 3
    call assert_fail
.Lshow_key_assert_count:
    mov edi, 4
    call assert_fail
.size show_key, . - show_key

# Switches the terminal, when standard input is one, to keys one at a time with no echo, saving
# the mode it had. Otherwise, as from a pipe, keys come as they are, and it changes nothing.
.type key_mode_on, @function
key_mode_on:
    mov eax, SYS_IOCTL
    mov edi, STDIN
    mov esi, TCGETS
    lea rdx, [rip + saved_mode]
    syscall
    test rax, rax
    js .Lkey_mode_on_return
    jnz .Lkey_mode_on_assert_saved
    mov eax, SYS_IOCTL
    mov edi, STDIN
    mov esi, TCGETS
    lea rdx, [rip + key_mode]
    syscall
    test rax, rax
    js .Lkey_mode_on_return
    jnz .Lkey_mode_on_assert_read
    and dword ptr [rip + key_mode + LOCAL_FLAGS], KEEP_FLAGS
    mov byte ptr [rip + key_mode + VMIN_AT], ONE_KEY
    mov byte ptr [rip + key_mode + VTIME_AT], NO_WAIT
    mov eax, SYS_IOCTL
    mov edi, STDIN
    mov esi, TCSETS
    lea rdx, [rip + key_mode]
    syscall
    test rax, rax
    js .Lkey_mode_on_return
    jnz .Lkey_mode_on_assert_set
    mov qword ptr [rip + changed], CHANGED
.Lkey_mode_on_return:
    ret
.Lkey_mode_on_assert_saved:
    mov edi, 5
    call assert_fail
.Lkey_mode_on_assert_read:
    mov edi, 6
    call assert_fail
.Lkey_mode_on_assert_set:
    mov edi, 7
    call assert_fail
.size key_mode_on, . - key_mode_on

# Sets the terminal back to the mode key_mode_on found, when it changed it.
.type key_mode_off, @function
key_mode_off:
    cmp qword ptr [rip + changed], CHANGED
    jne .Lkey_mode_off_return
    mov eax, SYS_IOCTL
    mov edi, STDIN
    mov esi, TCSETS
    lea rdx, [rip + saved_mode]
    syscall
    test rax, rax
    js .Lkey_mode_off_return
    jnz .Lkey_mode_off_assert_result
    mov qword ptr [rip + changed], UNCHANGED
.Lkey_mode_off_return:
    ret
.Lkey_mode_off_assert_result:
    mov edi, 8
    call assert_fail
.size key_mode_off, . - key_mode_off

# Waits up to TICK_MS milliseconds for the next key and reads it.
# Out: rax = the key, 0 to 255; NO_KEY_YET when none came in time; END at the end of the input
# or when it can't be read.
# returns: result
.type next_key, @function
next_key:
    mov eax, SYS_POLL
    lea rdi, [rip + waiting]
    mov esi, WAITING
    mov edx, TICK_MS
    syscall
    test rax, rax
    js .Lnext_key_end
    jz .Lnext_key_none
    cmp rax, WAITING
    ja .Lnext_key_assert_ready
    mov eax, SYS_READ
    mov edi, STDIN
    lea rsi, [rip + key]
    mov edx, KEY_LEN
    syscall
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
    ret
.Lnext_key_assert_ready:
    mov edi, 9
    call assert_fail
.Lnext_key_assert_count:
    mov edi, 10
    call assert_fail
.size next_key, . - next_key

# lengua's assertion handler. check_code compares it word for word: copy it exactly.
# In: edi = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
# standard error and ends the program, every thread of it, with code 134.
.type assert_fail, @function
assert_fail:
    sub rsp, 40
    mov rax, qword ptr [rip + assert_message]
    mov qword ptr [rsp], rax
    mov rax, qword ptr [rip + assert_message + 8]
    mov qword ptr [rsp + 8], rax
    mov rax, qword ptr [rip + assert_message + 16]
    mov qword ptr [rsp + 16], rax
    mov eax, dword ptr [rip + assert_message + 24]
    mov dword ptr [rsp + 24], eax
    mov al, byte ptr [rip + assert_message + 28]
    mov byte ptr [rsp + 28], al
    mov eax, edi
    xor edx, edx
    mov ecx, 100
    div ecx
    add al, 48
    mov byte ptr [rsp + 18], al
    mov eax, edx
    xor edx, edx
    mov ecx, 10
    div ecx
    add al, 48
    mov byte ptr [rsp + 19], al
    add dl, 48
    mov byte ptr [rsp + 20], dl
    mov eax, 1
    mov edi, 2
    mov rsi, rsp
    mov edx, 29
    syscall
    mov eax, 231
    mov edi, 134
    syscall
.size assert_fail, . - assert_fail
