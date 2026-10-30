.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_WRITE, 1
.equ SYS_EXIT, 60
.equ SYS_CLOCK_NANOSLEEP, 230
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

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.text
.globl _start

# Naps for 30 seconds, then says it's awake.
.type _start, @function
_start:
    mov edi, CLOCK_MONOTONIC
    xor esi, esi
    lea rdx, [rip + nap]
    test rdx, rdx
    jz .Lstart_assert_nap
    xor r10d, r10d
    mov eax, SYS_CLOCK_NANOSLEEP
    syscall
    test rax, rax
    jnz .Lstart_sleep_failed
    mov edi, STDOUT
    lea rsi, [rip + awake_msg]
    mov edx, AWAKE_LEN
    call write_all
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_sleep_failed:
    mov edi, EXIT_SLEEP_FAILED
    jmp .Lstart_exit
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_nap:
    mov edi, 4
    call assert_fail
.size _start, . - _start

# Writes rdx bytes from rsi to file descriptor edi, retrying partial writes.
# In: edi = the file descriptor, rsi = the bytes, rdx = how many, at least 1.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_all, @function
write_all:
    test rsi, rsi
    jz .Lwrite_all_assert_null
    test rdx, rdx
    jz .Lwrite_all_assert_empty
    mov r8, MAX_WRITES
.Lwrite_all_loop:  # bound: 64
    mov eax, SYS_WRITE
    syscall
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
