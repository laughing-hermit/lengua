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
# fill's reads at most, and what makes it read to the end
.equ READ_TO_END, -1
# A buffer-full of input, the most the main loop takes at a time
.equ BUF_LEN, 256
# The buffer-fulls the main loop takes at most
.equ MAX_FULLS, 1099511627776
.equ MAX_WRITES, 64
# The letter whose pairs are counted
.equ LETTER, 'a'
# Printing the count
.equ MAX_DIGITS, 19
.equ NEWLINE, 10
.equ OUT_BUF_LEN, 21

.section .bss

# One buffer-full of standard input.
# shared: main fills it, and count_pairs reads it
.type buffer, @object
buffer:
    .zero BUF_LEN
.size buffer, . - buffer

# The count's digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
# Counts the pairs of 'a' in standard input of any length, a buffer-full at a time, each 'a' in
# at most one pair, and prints the count and a newline. r14 carries across buffer-fulls whether
# the last byte read is an 'a' that starts no pair yet.
.type main, @function
main:
    sub rsp, 8
    xor r13d, r13d
    xor r14d, r14d
    mov rbx, MAX_FULLS
.Lmain_full:  # bound: 1099511627776
    mov edi, STDIN
    lea rsi, [rip + buffer]
    mov edx, BUF_LEN
    mov ecx, READ_TO_END
    call fill
    test rax, rax
    js .Lmain_read_failed
    jz .Lmain_print
    cmp rax, BUF_LEN
    ja .Lmain_assert_count
    mov r12, rax
    mov rdi, rax
    mov rsi, r14
    call count_pairs
    add r13, rax
    mov r14, rdx
    cmp r12, BUF_LEN
    jb .Lmain_print
    dec rbx
    jnz .Lmain_full
.Lmain_print:
    mov rdi, r13
    call write_i64
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_read_failed:
    mov edi, EXIT_READ_FAILED
    jmp .Lmain_exit
.Lmain_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_count:
    mov edi, 1
    call assert_fail
.size main, . - main

# Counts the pairs of 'a' among the first rdi bytes of buffer, each 'a' in at most one, taken
# from the left: an 'a' right after one that starts no pair yet ends a pair. Branchless, so a
# buffer-full is one path.
# In: rdi = how many, 1 to BUF_LEN; rsi = 1 when the byte before them is an 'a' that starts no
# pair yet, else 0.
# Out: rax = how many pairs end among them; rdx = 1 when the last of them is an 'a' that starts
# no pair yet, else 0.
.type count_pairs, @function
count_pairs:
    cmp rdi, BUF_LEN
    ja .Lcount_pairs_assert_length
    cmp rsi, 1
    ja .Lcount_pairs_assert_pending
    lea r8, [rip + buffer]
    xor eax, eax
    xor ecx, ecx
    mov rdx, rsi
    mov r9, BUF_LEN
.Lcount_pairs_loop:  # bound: 256
    cmp rcx, rdi
    jae .Lcount_pairs_return
    cmp rcx, BUF_LEN
    jae .Lcount_pairs_assert_index
    movzx r11d, byte ptr [r8 + rcx]
    xor r10d, r10d
    cmp r11d, LETTER
    sete r10b
    mov r11, r10
    and r11, rdx
    add rax, r11
    xor r11, 1
    and r10, r11
    mov rdx, r10
    inc rcx
    dec r9
    jnz .Lcount_pairs_loop
.Lcount_pairs_return:
    ret
.Lcount_pairs_assert_length:
    mov edi, 2
    call assert_fail
.Lcount_pairs_assert_pending:
    mov edi, 3
    call assert_fail
.Lcount_pairs_assert_index:
    mov edi, 12
    call assert_fail
.size count_pairs, . - count_pairs

# Writes a whole number in decimal, then a newline, to standard output.
# In: rdi = the number, never -9223372036854775808.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_i64, @function
write_i64:
    sub rsp, 8
    mov rax, rdi
    mov r9, rdi
    test rax, rax
    jns .Lwrite_i64_digits
    neg rax
    jo .Lwrite_i64_assert_min
.Lwrite_i64_digits:
    mov byte ptr [rip + out_buf + OUT_BUF_LEN - 1], NEWLINE
    lea rsi, [rip + out_buf]
    mov ecx, OUT_BUF_LEN - 1
    mov r10d, 10
    mov r8, MAX_DIGITS
.Lwrite_i64_digit:  # bound: 19
    xor edx, edx
    div r10
    add edx, '0'
    dec ecx
    cmp ecx, OUT_BUF_LEN
    jae .Lwrite_i64_assert_digit_index
    mov byte ptr [rsi + rcx], dl
    test rax, rax
    jz .Lwrite_i64_sign
    dec r8
    jnz .Lwrite_i64_digit
    jmp .Lwrite_i64_assert_digits
.Lwrite_i64_sign:
    test r9, r9
    jns .Lwrite_i64_output
    dec ecx
    cmp ecx, OUT_BUF_LEN
    jae .Lwrite_i64_assert_sign_index
    mov byte ptr [rsi + rcx], '-'
.Lwrite_i64_output:
    add rsi, rcx
    mov edx, OUT_BUF_LEN
    sub edx, ecx
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lwrite_i64_return
    xor eax, eax
.Lwrite_i64_return:
    add rsp, 8
    ret
.Lwrite_i64_assert_min:
    mov edi, 5
    call assert_fail
.Lwrite_i64_assert_digit_index:
    mov edi, 6
    call assert_fail
.Lwrite_i64_assert_digits:
    mov edi, 7
    call assert_fail
.Lwrite_i64_assert_sign_index:
    mov edi, 8
    call assert_fail
.size write_i64, . - write_i64

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

