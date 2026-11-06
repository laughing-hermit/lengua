.intel_syntax noprefix

# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 3
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# Sizes and limits
.equ IN_BUF_LEN, 5000
.equ MAX_READS, 5001
.equ MAX_WRITES, 64

.section .bss

# Standard input, as read.
.type in_buf, @object
in_buf:
    .zero IN_BUF_LEN
.size in_buf, . - in_buf

.text

# Reads standard input and prints it back. lengua's `_start` calls main, which never returns.
.type main, @function
main:
    sub rsp, 8
    lea rdi, [rip + in_buf]
    mov esi, IN_BUF_LEN
    call read_all
    test rax, rax
    js .Lmain_io_failed
    cmp rax, IN_BUF_LEN
    ja .Lmain_assert_length
    test rax, rax
    jz .Lmain_done
    mov rdx, rax
    mov edi, STDOUT
    lea rsi, [rip + in_buf]
    test rsi, rsi
    jz .Lmain_assert_buffer
    call write_all
    test rax, rax
    js .Lmain_io_failed
.Lmain_done:
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_io_failed:
    mov edi, EXIT_IO_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_length:
    mov edi, 1
    call assert_fail
.Lmain_assert_buffer:
    mov edi, 2
    call assert_fail
.size main, . - main

# Reads standard input until it ends or the buffer is full.
# In: rdi = the buffer, rsi = its size, at least 1.
# Out: rax = the bytes read, or a negative error code.
# returns: result
.type read_all, @function
read_all:
    sub rsp, 8
    test rdi, rdi
    jz .Lread_all_assert_null
    test rsi, rsi
    jz .Lread_all_assert_empty
    mov r10, rsi
    mov rdx, rsi
    mov rsi, rdi
    mov r8, MAX_READS
.Lread_all_loop:  # bound: 5001
    mov edi, STDIN
    call os_read
    test rax, rax
    js .Lread_all_return
    jz .Lread_all_done
    cmp rax, rdx
    ja .Lread_all_assert_count
    add rsi, rax
    sub rdx, rax
    jz .Lread_all_done
    dec r8
    jnz .Lread_all_loop
    mov rax, ERROR_IO
    jmp .Lread_all_return
.Lread_all_done:
    mov rax, r10
    sub rax, rdx
.Lread_all_return:
    add rsp, 8
    ret
.Lread_all_assert_null:
    mov edi, 3
    call assert_fail
.Lread_all_assert_empty:
    mov edi, 4
    call assert_fail
.Lread_all_assert_count:
    mov edi, 5
    call assert_fail
.size read_all, . - read_all

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
    mov edi, 6
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 7
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 8
    call assert_fail
.size write_all, . - write_all
