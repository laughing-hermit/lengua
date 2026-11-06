.intel_syntax noprefix

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

.section .bss

# Both lines.
.type input, @object
input:
    .zero INPUT_MAX
.size input, . - input

.text
# Reads both lines with one fill to the first newline a read ends with, and prints the second:
# right only when both lines arrive in one read.
.type main, @function
main:
    sub rsp, 8
    mov edi, STDIN
    lea rsi, [rip + input]
    mov edx, INPUT_MAX
    mov ecx, NEWLINE
    call fill
    test rax, rax
    js .Lmain_failed
    cmp rax, INPUT_MAX
    ja .Lmain_assert_count
    lea rdi, [rip + input]
    mov rsi, rax
    call print_second
    test rax, rax
    js .Lmain_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_failed:
    mov edi, EXIT_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_count:
    mov edi, 1
    call assert_fail
.size main, . - main

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
    mov edi, 9
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 10
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 11
    call assert_fail
.size write_all, . - write_all

