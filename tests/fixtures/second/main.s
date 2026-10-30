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
.equ EXIT_FAILED, 1
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# Sizes and limits
.equ INPUT_MAX, 64
.equ MAX_WRITES, 64
.equ NEWLINE, 10
# fill's limit on reads, as assets/fill.s has it
.equ MAX_FILL_READS, 1099511627776

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# Both lines.
.type input, @object
input:
    .zero INPUT_MAX
.size input, . - input

.text
.globl _start

# Reads both lines with one fill to the first newline a read ends with, and prints the second:
# right only when both lines arrive in one read.
.type _start, @function
_start:
    mov edi, STDIN
    lea rsi, [rip + input]
    mov edx, INPUT_MAX
    mov ecx, NEWLINE
    call fill
    test rax, rax
    js .Lstart_failed
    cmp rax, INPUT_MAX
    ja .Lstart_assert_count
    lea rdi, [rip + input]
    mov rsi, rax
    call print_second
    test rax, rax
    js .Lstart_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_failed:
    mov edi, EXIT_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_count:
    mov edi, 1
    call assert_fail
.size _start, . - _start

# Writes what follows the first newline in the rsi bytes at rdi to standard output: the second
# line, with its newline.
# In: rdi = the text, rsi = its length, at most INPUT_MAX.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_second, @function
print_second:
    sub rsp, 8
    test rdi, rdi
    jz .Lprint_second_assert_null
    cmp rsi, INPUT_MAX
    ja .Lprint_second_assert_length
    xor ecx, ecx
    mov r8, INPUT_MAX
.Lprint_second_scan:  # bound: 64
    cmp rcx, rsi
    jae .Lprint_second_write
    movzx eax, byte ptr [rdi + rcx]
    inc rcx
    cmp eax, NEWLINE
    je .Lprint_second_write
    dec r8
    jnz .Lprint_second_scan
.Lprint_second_write:
    mov rdx, rsi
    sub rdx, rcx
    jz .Lprint_second_none
    lea rsi, [rdi + rcx]
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lprint_second_return
.Lprint_second_none:
    xor eax, eax
.Lprint_second_return:
    add rsp, 8
    ret
.Lprint_second_assert_null:
    mov edi, 2
    call assert_fail
.Lprint_second_assert_length:
    mov edi, 3
    call assert_fail
.size print_second, . - print_second

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
    mov edi, 9
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 10
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 11
    call assert_fail
.size write_all, . - write_all

# Reads from file descriptor edi into the rdx bytes at rsi until they're full, the input ends,
# or, when ecx is a byte (0 to 255), the last byte read is ecx; with ecx = -1 it reads to the
# end. However the input arrives, it reads the same.
# In: edi = the file descriptor, rsi = the buffer, rdx = its size, at least 1, ecx = the end
# byte or -1.
# Out: rax = how many bytes it read, or a negative error code.
# returns: result
.type fill, @function
fill:
    test rsi, rsi
    jz .Lfill_assert_null
    test rdx, rdx
    jz .Lfill_assert_empty
    mov r8, rdx
    mov r10d, ecx
    mov r9, MAX_FILL_READS
.Lfill_loop:  # bound: 1099511627776
    mov eax, SYS_READ
    syscall
    test rax, rax
    js .Lfill_return
    jz .Lfill_done
    cmp rax, rdx
    ja .Lfill_assert_count
    add rsi, rax
    sub rdx, rax
    jz .Lfill_done
    movzx eax, byte ptr [rsi - 1]
    cmp eax, r10d
    je .Lfill_done
    dec r9
    jnz .Lfill_loop
    mov rax, ERROR_IO
    jmp .Lfill_return
.Lfill_done:
    mov rax, r8
    sub rax, rdx
.Lfill_return:
    ret
.Lfill_assert_null:
    mov edi, 990
    call assert_fail
.Lfill_assert_empty:
    mov edi, 991
    call assert_fail
.Lfill_assert_count:
    mov edi, 992
    call assert_fail
.size fill, . - fill

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
