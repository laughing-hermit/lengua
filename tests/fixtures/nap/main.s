.intel_syntax noprefix

# File descriptors
.equ STDOUT, 1
# Clocks
.equ CLOCK_MONOTONIC, 1
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_SLEEP_FAILED, 2
.equ EXIT_WRITE_FAILED, 3
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# Sizes and limits
.equ AWAKE_LEN, 6
.equ MAX_WRITES, 64

.section .rodata

# How long it naps: 30 seconds and no nanoseconds.
.type nap, @object
nap:
    .quad 30
    .quad 0
.size nap, . - nap

.type awake_msg, @object
awake_msg:
    .ascii "awake\n"
.size awake_msg, . - awake_msg

.text
# Naps for 30 seconds, then says it's awake.
.type main, @function
main:
    sub rsp, 8
    mov edi, CLOCK_MONOTONIC
    xor esi, esi
    lea rdx, [rip + nap]
    test rdx, rdx
    jz .Lmain_assert_nap
    call os_sleep
    test rax, rax
    jnz .Lmain_sleep_failed
    mov edi, STDOUT
    lea rsi, [rip + awake_msg]
    mov edx, AWAKE_LEN
    call write_all
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_sleep_failed:
    mov edi, EXIT_SLEEP_FAILED
    jmp .Lmain_exit
.Lmain_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_nap:
    mov edi, 4
    call assert_fail
.size main, . - main

# Writes rdx bytes from rsi to file descriptor edi, retrying partial writes.
# In: edi = the file descriptor, rsi = the bytes, rdx = how many, at least 1.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_all, @function
write_all:
    sub rsp, 8
    test rsi, rsi
    jz .Lwrite_all_assert_null
    test rdx, rdx
    jz .Lwrite_all_assert_empty
    mov r8, MAX_WRITES
.Lwrite_all_loop:  # bound: 64
    call os_write
    test rax, rax
    js .Lwrite_all_return
    cmp rax, rdx
    ja .Lwrite_all_assert_count
    add rsi, rax
    sub rdx, rax
    jz .Lwrite_all_done
    dec r8
    jnz .Lwrite_all_loop
    mov rax, ERROR_IO
    jmp .Lwrite_all_return
.Lwrite_all_done:
    xor eax, eax
.Lwrite_all_return:
    add rsp, 8
    ret
.Lwrite_all_assert_null:
    mov edi, 1
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 2
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 3
    call assert_fail
.size write_all, . - write_all

