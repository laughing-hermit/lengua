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
.equ EXIT_IO_FAILED, 2
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# CRC-32C starts from all ones and ends inverted.
.equ CRC_START, 0xFFFFFFFF
# Three chains of BLOCK bytes each, joined after every BLOCK3 bytes.
.equ BLOCK, 4096
.equ BLOCK3, 12288
.equ STEP, 8
.equ STEPS, 512
.equ STEPS_REST, 1536
.equ BLOCKS, 86
# Sizes and limits
.equ INPUT_LEN, 1048576
.equ MAX_READS, 1099511627776
.equ MAX_DIGITS, 20
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ OUT_LEN, 11

.section .rodata

# x^(8 * 2 * BLOCK - 33) and x^(8 * BLOCK - 33) mod the CRC-32C polynomial, bit-reflected: a
# carry-less multiply by one and a crc32 of the product move a chain's state past 2 * BLOCK or
# BLOCK more bytes.
.type shift_two, @object
shift_two:
    .quad 0x54a86326
.size shift_two, . - shift_two

.type shift_one, @object
shift_one:
    .quad 0x82f89c77
.size shift_one, . - shift_one

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# Standard input as it comes, after the bytes kept from the last read.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

# The checksum in decimal and a newline.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
.globl _start

# Reads standard input as it comes, folding whole blocks into the checksum and keeping the rest
# for the next read, then prints the checksum.
.type _start, @function
_start:
    mov r12d, CRC_START
    xor r13d, r13d
    mov rbx, MAX_READS
.Lstart_read:  # bound: 1099511627776
    mov eax, SYS_READ
    mov edi, STDIN
    lea rsi, [rip + input]
    add rsi, r13
    mov edx, INPUT_LEN
    sub edx, r13d
    syscall
    test rax, rax
    js .Lstart_failed
    jz .Lstart_finish
    add r13, rax
    cmp r13, INPUT_LEN
    ja .Lstart_assert_count
    mov edi, r12d
    lea rsi, [rip + input]
    mov rdx, r13
    call crc_blocks
    mov r12d, eax
    sub r13, rdx
    jz .Lstart_next
    test rdx, rdx
    jz .Lstart_next
    lea rdi, [rip + input]
    lea rsi, [rdi + rdx]
    mov rdx, r13
    call move_front
.Lstart_next:
    dec rbx
    jnz .Lstart_read
    jmp .Lstart_assert_reads
.Lstart_finish:
    mov edi, r12d
    lea rsi, [rip + input]
    mov rdx, r13
    call crc_rest
    not eax
    mov edi, eax
    call print_number
    test rax, rax
    js .Lstart_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_failed:
    mov edi, EXIT_IO_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_count:
    mov edi, 1
    call assert_fail
.Lstart_assert_reads:
    mov edi, 2
    call assert_fail
.size _start, . - _start

# Prints a whole number from 0 to 4294967295 and a newline to standard output.
# In: edi = the number.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_number, @function
print_number:
    sub rsp, 8
    lea rsi, [rip + out]
    add rsi, OUT_LEN - 1
    mov byte ptr [rsi], NEWLINE
    mov edi, edi
    call digits
    lea rdx, [rip + out]
    cmp rax, rdx
    jb .Lprint_number_assert_digits
    add rdx, OUT_LEN
    sub rdx, rax
    mov rsi, rax
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lprint_number_return
    xor eax, eax
.Lprint_number_return:
    add rsp, 8
    ret
.Lprint_number_assert_digits:
    mov edi, 3
    call assert_fail
.size print_number, . - print_number

# Copies the bytes still to fold to the front of the buffer, 8 at a time and then one at a time,
# so it never reads past them.
# In: rdi = where they go; rsi = where they are, at least BLOCK3 bytes after rdi; rdx = how many,
# fewer than BLOCK3.
.type move_front, @function
move_front:
    test rdi, rdi
    jz .Lmove_front_assert_to
    test rsi, rsi
    jz .Lmove_front_assert_from
    cmp rdx, BLOCK3
    jae .Lmove_front_assert_length
    xor ecx, ecx
    mov r8, STEPS_REST
.Lmove_front_words:  # bound: 1536
    lea r9, [rcx + STEP]
    cmp r9, rdx
    ja .Lmove_front_bytes
    cmp rcx, BLOCK3 - STEP + 1
    jae .Lmove_front_assert_word
    mov rax, qword ptr [rsi + rcx]
    mov qword ptr [rdi + rcx], rax
    add rcx, STEP
    dec r8
    jnz .Lmove_front_words
    jmp .Lmove_front_assert_words
.Lmove_front_bytes:
    mov r8, STEP
.Lmove_front_byte:  # bound: 8
    cmp rcx, rdx
    jae .Lmove_front_done
    cmp rcx, BLOCK3
    jae .Lmove_front_assert_byte
    movzx eax, byte ptr [rsi + rcx]
    mov byte ptr [rdi + rcx], al
    inc rcx
    dec r8
    jnz .Lmove_front_byte
.Lmove_front_done:
    ret
.Lmove_front_assert_to:
    mov edi, 4
    call assert_fail
.Lmove_front_assert_from:
    mov edi, 5
    call assert_fail
.Lmove_front_assert_length:
    mov edi, 16
    call assert_fail
.Lmove_front_assert_word:
    mov edi, 17
    call assert_fail
.Lmove_front_assert_words:
    mov edi, 18
    call assert_fail
.Lmove_front_assert_byte:
    mov edi, 19
    call assert_fail
.size move_front, . - move_front

# Folds every whole block of BLOCK3 bytes into a CRC-32C state, three chains at a time.
# In: edi = the state; rsi = the bytes; rdx = how many, up to INPUT_LEN.
# Out: eax = the new state; rdx = how many bytes it folded, a multiple of BLOCK3.
.type crc_blocks, @function
crc_blocks:
    push rbx
    push r12
    push r13
    push r14
    sub rsp, 8
    test rsi, rsi
    jz .Lcrc_blocks_assert_bytes
    cmp rdx, INPUT_LEN
    ja .Lcrc_blocks_assert_length
    mov eax, edi
    mov rbx, rsi
    mov r12, rdx
    xor r14d, r14d
    mov r13, BLOCKS
.Lcrc_blocks_loop:  # bound: 86
    cmp r12, BLOCK3
    jb .Lcrc_blocks_done
    mov edi, eax
    mov rsi, rbx
    call crc_block
    add rbx, BLOCK3
    sub r12, BLOCK3
    add r14, BLOCK3
    dec r13
    jnz .Lcrc_blocks_loop
    jmp .Lcrc_blocks_assert_blocks
.Lcrc_blocks_done:
    mov rdx, r14
    add rsp, 8
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Lcrc_blocks_assert_bytes:
    mov edi, 6
    call assert_fail
.Lcrc_blocks_assert_length:
    mov edi, 7
    call assert_fail
.Lcrc_blocks_assert_blocks:
    mov edi, 8
    call assert_fail
.size crc_blocks, . - crc_blocks

# Folds BLOCK3 bytes into a CRC-32C state as three chains of BLOCK bytes, which the CPU runs side
# by side, then joins them by moving the first two past the bytes after them.
# In: edi = the state; rsi = the bytes, at least BLOCK3 of them.
# Out: eax = the new state.
.type crc_block, @function
crc_block:
    test rsi, rsi
    jz .Lcrc_block_assert_bytes
    mov eax, edi
    xor r8d, r8d
    xor r9d, r9d
    xor ecx, ecx
    mov r10, STEPS
.Lcrc_block_loop:  # bound: 512
    cmp rcx, BLOCK - STEP + 1
    jae .Lcrc_block_assert_index
    crc32 rax, qword ptr [rsi + rcx]
    crc32 r8, qword ptr [rsi + rcx + BLOCK]
    crc32 r9, qword ptr [rsi + rcx + 2 * BLOCK]
    add rcx, STEP
    dec r10
    jnz .Lcrc_block_loop
    movq xmm0, rax
    movq xmm1, qword ptr [rip + shift_two]
    pclmulqdq xmm0, xmm1, 0
    movq rax, xmm0
    xor edx, edx
    crc32 rdx, rax
    movq xmm0, r8
    movq xmm1, qword ptr [rip + shift_one]
    pclmulqdq xmm0, xmm1, 0
    movq r8, xmm0
    xor r10d, r10d
    crc32 r10, r8
    mov eax, r9d
    xor eax, edx
    xor eax, r10d
    ret
.Lcrc_block_assert_bytes:
    mov edi, 9
    call assert_fail
.Lcrc_block_assert_index:
    mov edi, 10
    call assert_fail
.size crc_block, . - crc_block

# Folds fewer than BLOCK3 bytes into a CRC-32C state, 8 at a time and then one at a time.
# In: edi = the state; rsi = the bytes; rdx = how many, fewer than BLOCK3.
# Out: eax = the new state.
.type crc_rest, @function
crc_rest:
    test rsi, rsi
    jz .Lcrc_rest_assert_bytes
    cmp rdx, BLOCK3
    jae .Lcrc_rest_assert_length
    mov eax, edi
    xor ecx, ecx
    mov r9, STEPS_REST
.Lcrc_rest_words:  # bound: 1536
    lea r10, [rcx + STEP]
    cmp r10, rdx
    ja .Lcrc_rest_bytes
    cmp rcx, BLOCK3 - STEP + 1
    jae .Lcrc_rest_assert_word
    crc32 rax, qword ptr [rsi + rcx]
    add rcx, STEP
    dec r9
    jnz .Lcrc_rest_words
    jmp .Lcrc_rest_assert_words
.Lcrc_rest_bytes:
    mov r9, STEP
.Lcrc_rest_byte:  # bound: 8
    cmp rcx, rdx
    jae .Lcrc_rest_done
    cmp rcx, BLOCK3
    jae .Lcrc_rest_assert_byte
    crc32 eax, byte ptr [rsi + rcx]
    inc rcx
    dec r9
    jnz .Lcrc_rest_byte
.Lcrc_rest_done:
    ret
.Lcrc_rest_assert_bytes:
    mov edi, 11
    call assert_fail
.Lcrc_rest_assert_length:
    mov edi, 12
    call assert_fail
.Lcrc_rest_assert_word:
    mov edi, 13
    call assert_fail
.Lcrc_rest_assert_words:
    mov edi, 14
    call assert_fail
.Lcrc_rest_assert_byte:
    mov edi, 15
    call assert_fail
.size crc_rest, . - crc_rest

# Writes a whole number's decimal digits just before a place in memory.
# In: rdi = the number; rsi = where the digits end.
# Out: rax = where they start.
.type digits, @function
digits:
    test rsi, rsi
    jz .Ldigits_assert_end
    mov rax, rdi
    mov r8d, 10
    mov rcx, MAX_DIGITS
.Ldigits_loop:  # bound: 20
    xor edx, edx
    div r8
    add edx, '0'
    dec rsi
    mov byte ptr [rsi], dl
    test rax, rax
    jz .Ldigits_done
    dec rcx
    jnz .Ldigits_loop
    jmp .Ldigits_assert_digits
.Ldigits_done:
    mov rax, rsi
    ret
.Ldigits_assert_end:
    mov edi, 20
    call assert_fail
.Ldigits_assert_digits:
    mov edi, 21
    call assert_fail
.size digits, . - digits

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
    mov edi, 30
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 31
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 32
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
