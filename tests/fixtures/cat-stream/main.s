.intel_syntax noprefix

# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_READ_FAILED, 3
.equ EXIT_WRITE_FAILED, 4
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# fill's end byte for none
.equ READ_TO_END, -1
# Sizes and limits
.equ BUF_LEN, 4096
.equ MAX_FULLS, 1099511627776
.equ MAX_WRITES, 64

.section .bss

# One buffer-full of standard input.
.type buffer, @object
buffer:
    .zero BUF_LEN
.size buffer, . - buffer

.text

# Copies standard input of any length to standard output, a buffer-full at a time: fill reads
# until the buffer is full or the input ends, so a buffer-full that isn't full is the last.
# lengua's `_start` calls main, which never returns.
.type main, @function
main:
    sub rsp, 8
    mov rbx, MAX_FULLS
.Lmain_full:  # bound: 1099511627776
    mov edi, STDIN
    lea rsi, [rip + buffer]
    mov edx, BUF_LEN
    mov ecx, READ_TO_END
    call fill
    test rax, rax
    js .Lmain_read_failed
    jz .Lmain_done
    cmp rax, BUF_LEN
    ja .Lmain_assert_length
    mov r12, rax
    mov edi, STDOUT
    lea rsi, [rip + buffer]
    mov rdx, rax
    call write_all
    test rax, rax
    js .Lmain_write_failed
    cmp r12, BUF_LEN
    jb .Lmain_done
    dec rbx
    jnz .Lmain_full
    jmp .Lmain_read_failed
.Lmain_done:
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_read_failed:
    mov edi, EXIT_READ_FAILED
    jmp .Lmain_exit
.Lmain_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_length:
    mov edi, 1
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
    mov edi, 2
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 3
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 4
    call assert_fail
.size write_all, . - write_all
