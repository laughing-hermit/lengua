# lengua: spec {label}
.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_READ, 0
.equ SYS_WRITE, 1
.equ SYS_EXIT, 60
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

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# Standard input, as read.
.type in_buf, @object
in_buf:
    .zero IN_BUF_LEN
.size in_buf, . - in_buf

.text
.globl _start

# Reads standard input and prints it back.
.type _start, @function
_start:
    lea rdi, [rip + in_buf]
    mov esi, IN_BUF_LEN
    call read_all
    test rax, rax
    js .Lstart_io_failed
    cmp rax, IN_BUF_LEN
    ja .Lstart_assert_length
    test rax, rax
    jz .Lstart_done
    mov rdx, rax
    mov edi, STDOUT
    lea rsi, [rip + in_buf]
    test rsi, rsi
    jz .Lstart_assert_buffer
    call write_all
    test rax, rax
    js .Lstart_io_failed
.Lstart_done:
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_io_failed:
    mov edi, EXIT_IO_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_length:
    mov edi, 1
    call assert_fail
.Lstart_assert_buffer:
    mov edi, 2
    call assert_fail
.size _start, . - _start

# Reads standard input until it ends or the buffer is full.
# In: rdi = the buffer, rsi = its size, at least 1.
# Out: rax = the bytes read, or a negative error code.
# returns: result
.type read_all, @function
read_all:
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
    mov eax, SYS_READ
    syscall
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
    mov edi, 6
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 7
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 8
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
