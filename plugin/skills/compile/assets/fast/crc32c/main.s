.intel_syntax noprefix

# CRC-32C with the crc32 instruction of SSE4.2, one chain, 8 bytes a step: it runs on every x86-64
# computer that has SSE4.2, which lengua's x86-64 baseline holds. The form that runs three chains
# side by side and joins them with pclmulqdq, from the very same spec, is
# plugin/skills/compile/assets/fast/crc32c-pclmul.

# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 2
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# fill's constants
.equ READ_TO_END, -1
# CRC-32C starts from all ones and ends inverted.
.equ CRC_START, 0xFFFFFFFF
# Sizes and limits: standard input is folded in pieces of INPUT_LEN bytes, 8 bytes a step
.equ INPUT_LEN, 65536
.equ MAX_FULLS, 1099511627776
.equ STEP, 8
.equ WORD_STEPS, 8193
.equ MAX_DIGITS, 20
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ OUT_LEN, 11

.section .bss

# One piece of standard input, as it comes.
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
# Folds standard input into the checksum a piece at a time, as the pieces come, and prints it.
.type main, @function
main:
    sub rsp, 8
    mov r12d, CRC_START
    mov rbx, MAX_FULLS
.Lmain_full:  # bound: 1099511627776
    mov edi, STDIN
    lea rsi, [rip + input]
    mov edx, INPUT_LEN
    mov ecx, READ_TO_END
    call fill
    test rax, rax
    js .Lmain_failed
    jz .Lmain_print
    cmp rax, INPUT_LEN
    ja .Lmain_assert_count
    mov r13, rax
    mov edi, r12d
    lea rsi, [rip + input]
    mov rdx, r13
    call crc_bytes
    mov r12d, eax
    cmp r13, INPUT_LEN
    jb .Lmain_print
    dec rbx
    jnz .Lmain_full
    jmp .Lmain_assert_fulls
.Lmain_print:
    not r12d
    mov edi, r12d
    call print_number
    test rax, rax
    js .Lmain_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_failed:
    mov edi, EXIT_IO_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_count:
    mov edi, 1
    call assert_fail
.Lmain_assert_fulls:
    mov edi, 2
    call assert_fail
.size main, . - main

# Folds bytes into a CRC-32C state, 8 at a time and then one at a time.
# In: edi = the state; rsi = the bytes; rdx = how many, up to INPUT_LEN.
# Out: eax = the new state.
.type crc_bytes, @function
crc_bytes:
    test rsi, rsi
    jz .Lcrc_bytes_assert_bytes
    cmp rdx, INPUT_LEN
    ja .Lcrc_bytes_assert_length
    mov eax, edi
    xor ecx, ecx
    mov r9, WORD_STEPS
.Lcrc_bytes_words:  # bound: 8193
    lea r10, [rcx + STEP]
    cmp r10, rdx
    ja .Lcrc_bytes_tail
    cmp rcx, INPUT_LEN - STEP + 1
    jae .Lcrc_bytes_assert_word
    crc32 rax, qword ptr [rsi + rcx]
    add rcx, STEP
    dec r9
    jnz .Lcrc_bytes_words
    jmp .Lcrc_bytes_assert_words
.Lcrc_bytes_tail:
    mov r9, STEP
.Lcrc_bytes_byte:  # bound: 8
    cmp rcx, rdx
    jae .Lcrc_bytes_done
    cmp rcx, INPUT_LEN
    jae .Lcrc_bytes_assert_byte
    crc32 eax, byte ptr [rsi + rcx]
    inc rcx
    dec r9
    jnz .Lcrc_bytes_byte
.Lcrc_bytes_done:
    ret
.Lcrc_bytes_assert_bytes:
    mov edi, 3
    call assert_fail
.Lcrc_bytes_assert_length:
    mov edi, 4
    call assert_fail
.Lcrc_bytes_assert_word:
    mov edi, 5
    call assert_fail
.Lcrc_bytes_assert_words:
    mov edi, 6
    call assert_fail
.Lcrc_bytes_assert_byte:
    mov edi, 7
    call assert_fail
.size crc_bytes, . - crc_bytes

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
    mov edi, 8
    call assert_fail
.size print_number, . - print_number

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
    mov edi, 9
    call assert_fail
.Ldigits_assert_digits:
    mov edi, 10
    call assert_fail
.size digits, . - digits

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
    mov edi, 30
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 31
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 32
    call assert_fail
.size write_all, . - write_all

