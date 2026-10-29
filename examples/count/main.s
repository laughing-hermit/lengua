# lengua: spec sha256:5b66c154c1e3459e9ef09009ecd735ff200090bd4621acab4f975588a819b3fc
.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_READ, 0
.equ SYS_WRITE, 1
.equ SYS_EXIT, 60
# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
.equ STDERR, 2
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_READ_FAILED, 2
.equ EXIT_WRITE_FAILED, 3
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# Bytes that matter
.equ NEWLINE, 10
.equ SPACE, 32
.equ TAB, 9
# Tab through carriage return are the 5 bytes from 9 to 13.
.equ TAB_TO_CR, 4
# Sizes and limits
.equ ARGC_EXPECTED, 1
.equ INPUT_LEN, 262144
.equ VECTOR, 64
.equ VECTOR_MASK, 63
.equ PAIR, 128
.equ PAIR_MASK, 127
.equ PAIRS, 2048
.equ CHUNKS, 4096
.equ MAX_READS, 1099511627776
.equ MAX_DIGITS, 20
.equ MAX_WRITES, 64
.equ OUT_LEN, 42
.equ USAGE_MSG_LEN, 21

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: count < input\n"
.size usage_msg, . - usage_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# One read's worth of standard input.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

# The line it prints: two numbers of up to 20 digits, a space and a newline.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
.globl _start

# Checks there are no arguments, then reads standard input 256 KiB at a time, a piece that stays in the core's cache, counting
# newlines and word starts, and prints both.
.type _start, @function
_start:
    mov rax, qword ptr [rsp]
    cmp rax, ARGC_EXPECTED
    jne .Lstart_usage
    xor r12d, r12d
    xor r13d, r13d
    mov r14d, 1
    mov rbx, MAX_READS
.Lstart_read:  # bound: 1099511627776
    mov eax, SYS_READ
    mov edi, STDIN
    lea rsi, [rip + input]
    mov edx, INPUT_LEN
    syscall
    test rax, rax
    js .Lstart_read_failed
    jz .Lstart_print
    cmp rax, INPUT_LEN
    ja .Lstart_assert_count
    lea rdi, [rip + input]
    mov rsi, rax
    mov edx, r14d
    call count_piece
    add r12, rax
    add r13, rdx
    mov r14, rcx
    dec rbx
    jnz .Lstart_read
    jmp .Lstart_assert_reads
.Lstart_print:
    mov rdi, r12
    mov rsi, r13
    call print_counts
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_usage:
    mov edi, STDERR
    lea rsi, [rip + usage_msg]
    mov edx, USAGE_MSG_LEN
    call write_all
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_USAGE
    jmp .Lstart_exit
.Lstart_read_failed:
    mov edi, EXIT_READ_FAILED
    jmp .Lstart_exit
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
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

# Counts the newlines and word starts in one read's worth of input.
# In: rdi = the bytes; rsi = how many, 1 to INPUT_LEN; edx = 1 when the byte before them is
# whitespace (or there is none), else 0.
# Out: rax = newlines; rdx = word starts; rcx = 1 when the last byte is whitespace, else 0.
.type count_piece, @function
count_piece:
    push r12
    push r13
    push r14
    push r15
    push rbx
    test rdi, rdi
    jz .Lcount_piece_assert_bytes
    test rsi, rsi
    jz .Lcount_piece_assert_empty
    cmp rsi, INPUT_LEN
    ja .Lcount_piece_assert_length
    mov rbx, rdi
    mov r15, rsi
    mov r14d, edx
    call load_classes
    mov rdi, rbx
    mov rsi, r15
    and rsi, -PAIR
    mov edx, r14d
    call count_pairs
    mov r12, rax
    mov r13, rdx
    mov r14, rcx
    test r15, VECTOR
    jz .Lcount_piece_tail
    mov rdi, r15
    and rdi, -PAIR
    add rdi, rbx
    mov esi, VECTOR
    mov edx, r14d
    call count_full
    add r12, rax
    add r13, rdx
    mov r14, rcx
.Lcount_piece_tail:
    mov rsi, r15
    and rsi, VECTOR_MASK
    jz .Lcount_piece_done
    lea rdi, [rbx + r15]
    sub rdi, rsi
    mov edx, r14d
    call count_tail
    add r12, rax
    add r13, rdx
    mov r14, rcx
.Lcount_piece_done:
    mov rax, r12
    mov rdx, r13
    mov rcx, r14
    pop rbx
    pop r15
    pop r14
    pop r13
    pop r12
    ret
.Lcount_piece_assert_bytes:
    mov edi, 15
    call assert_fail
.Lcount_piece_assert_empty:
    mov edi, 13
    call assert_fail
.Lcount_piece_assert_length:
    mov edi, 14
    call assert_fail
.size count_piece, . - count_piece

# Fills zmm1 to zmm4 with the bytes count_pairs compares against, in every lane: newline, space,
# tab, and the distance from tab to carriage return.
# Out: zmm1 = newlines; zmm2 = spaces; zmm3 = tabs; zmm4 = TAB_TO_CR.
.type load_classes, @function
load_classes:
    mov eax, NEWLINE
    vpbroadcastb zmm1, eax
    mov eax, SPACE
    vpbroadcastb zmm2, eax
    mov eax, TAB
    vpbroadcastb zmm3, eax
    mov eax, TAB_TO_CR
    vpbroadcastb zmm4, eax
    ret
.size load_classes, . - load_classes

# Counts the newlines and word starts in whole pairs of 64-byte vectors, two vectors a step.
# In: rdi = the bytes; rsi = how many, a multiple of 128 up to INPUT_LEN; edx = 1 when the byte
# before them is whitespace (or there is none), else 0; zmm1 to zmm4 = the byte classes from
# load_classes.
# Out: rax = newlines; rdx = word starts; rcx = 1 when the last byte is whitespace, else as given.
.type count_pairs, @function
count_pairs:
    push rbx
    push r12
    cmp rsi, INPUT_LEN
    ja .Lcount_pairs_assert_length
    mov r10d, edx
    xor r8d, r8d
    xor r9d, r9d
    xor ecx, ecx
    mov r11, PAIRS
.Lcount_pairs_loop:  # bound: 2048
    cmp rcx, rsi
    jae .Lcount_pairs_done
    cmp rcx, INPUT_LEN - PAIR + 1
    jae .Lcount_pairs_assert_index
    vmovdqu8 zmm0, zmmword ptr [rdi + rcx]
    vmovdqu8 zmm6, zmmword ptr [rdi + rcx + VECTOR]
    vpcmpeqb k2, zmm0, zmm1
    vpcmpeqb k5, zmm6, zmm1
    vpcmpeqb k3, zmm0, zmm2
    vpcmpeqb k6, zmm6, zmm2
    vpsubb zmm5, zmm0, zmm3
    vpsubb zmm7, zmm6, zmm3
    vpcmpub k4, zmm5, zmm4, 2
    vpcmpub k7, zmm7, zmm4, 2
    korq k3, k3, k4
    korq k6, k6, k7
    kmovq rax, k2
    kmovq rdx, k5
    popcnt rax, rax
    popcnt rdx, rdx
    add r8, rax
    add r8, rdx
    kmovq rax, k3
    kmovq rdx, k6
    lea rbx, [rax + rax]
    or rbx, r10
    andn rbx, rax, rbx
    shr rax, 63
    lea r12, [rdx + rdx]
    or r12, rax
    andn r12, rdx, r12
    popcnt rbx, rbx
    popcnt r12, r12
    add r9, rbx
    add r9, r12
    shr rdx, 63
    mov r10, rdx
    add rcx, PAIR
    dec r11
    jnz .Lcount_pairs_loop
.Lcount_pairs_done:
    mov rax, r8
    mov rdx, r9
    mov rcx, r10
    pop r12
    pop rbx
    ret
.Lcount_pairs_assert_length:
    mov edi, 17
    call assert_fail
.Lcount_pairs_assert_index:
    mov edi, 18
    call assert_fail
.size count_pairs, . - count_pairs

# Counts the newlines and word starts in whole 64-byte vectors.
# In: rdi = the bytes; rsi = how many, a multiple of 64 up to INPUT_LEN; edx = 1 when the byte
# before them is whitespace (or there is none), else 0.
# Out: rax = newlines; rdx = word starts; rcx = 1 when the last byte is whitespace, else as given.
.type count_full, @function
count_full:
    test rdi, rdi
    jz .Lcount_full_assert_bytes
    cmp rsi, INPUT_LEN
    ja .Lcount_full_assert_length
    mov eax, NEWLINE
    vpbroadcastb zmm1, eax
    mov eax, SPACE
    vpbroadcastb zmm2, eax
    mov eax, TAB
    vpbroadcastb zmm3, eax
    mov eax, TAB_TO_CR
    vpbroadcastb zmm4, eax
    mov r10d, edx
    xor r8d, r8d
    xor r9d, r9d
    xor ecx, ecx
    mov r11, CHUNKS
.Lcount_full_loop:  # bound: 4096
    cmp rcx, rsi
    jae .Lcount_full_done
    cmp rcx, INPUT_LEN - VECTOR + 1
    jae .Lcount_full_assert_index
    vmovdqu8 zmm0, zmmword ptr [rdi + rcx]
    vpcmpeqb k2, zmm0, zmm1
    kmovq rax, k2
    popcnt rax, rax
    add r8, rax
    vpcmpeqb k3, zmm0, zmm2
    vpsubb zmm5, zmm0, zmm3
    vpcmpub k4, zmm5, zmm4, 2
    korq k3, k3, k4
    kmovq rax, k3
    lea rdx, [rax + rax]
    or rdx, r10
    andn rdx, rax, rdx
    popcnt rdx, rdx
    add r9, rdx
    shr rax, 63
    mov r10, rax
    add rcx, VECTOR
    dec r11
    jnz .Lcount_full_loop
.Lcount_full_done:
    mov rax, r8
    mov rdx, r9
    mov rcx, r10
    ret
.Lcount_full_assert_bytes:
    mov edi, 3
    call assert_fail
.Lcount_full_assert_length:
    mov edi, 4
    call assert_fail
.Lcount_full_assert_index:
    mov edi, 5
    call assert_fail
.size count_full, . - count_full

# Counts the newlines and word starts in the last 1 to 63 bytes of a read, with a masked load.
# In: rdi = the bytes; rsi = how many, 1 to 63; edx = 1 when the byte before them is whitespace
# (or there is none), else 0.
# Out: rax = newlines; rdx = word starts; rcx = 1 when the last byte is whitespace, else 0.
.type count_tail, @function
count_tail:
    test rdi, rdi
    jz .Lcount_tail_assert_bytes
    test rsi, rsi
    jz .Lcount_tail_assert_empty
    cmp rsi, VECTOR
    jae .Lcount_tail_assert_length
    mov r10d, edx
    mov r8, -1
    bzhi r8, r8, rsi
    kmovq k1, r8
    vmovdqu8 zmm0 {k1}{z}, zmmword ptr [rdi]
    mov eax, NEWLINE
    vpbroadcastb zmm1, eax
    mov eax, SPACE
    vpbroadcastb zmm2, eax
    mov eax, TAB
    vpbroadcastb zmm3, eax
    mov eax, TAB_TO_CR
    vpbroadcastb zmm4, eax
    vpcmpeqb k2 {k1}, zmm0, zmm1
    kmovq r9, k2
    popcnt r9, r9
    vpcmpeqb k3 {k1}, zmm0, zmm2
    vpsubb zmm5, zmm0, zmm3
    vpcmpub k4 {k1}, zmm5, zmm4, 2
    korq k3, k3, k4
    kmovq rax, k3
    lea rdx, [rax + rax]
    or rdx, r10
    andn rcx, rax, r8
    and rdx, rcx
    popcnt rdx, rdx
    lea rcx, [rsi - 1]
    shrx rcx, rax, rcx
    and ecx, 1
    mov rax, r9
    ret
.Lcount_tail_assert_bytes:
    mov edi, 6
    call assert_fail
.Lcount_tail_assert_empty:
    mov edi, 10
    call assert_fail
.Lcount_tail_assert_length:
    mov edi, 7
    call assert_fail
.size count_tail, . - count_tail

# Prints two whole numbers, a space between them and a newline after, to standard output.
# In: rdi = the first; rsi = the second.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_counts, @function
print_counts:
    push rbx
    sub rsp, 16
    mov rbx, rdi
    lea rdi, [rip + out]
    lea rax, [rdi + OUT_LEN - 1]
    mov byte ptr [rax], NEWLINE
    mov rdi, rsi
    mov rsi, rax
    call digits
    lea rdx, [rip + out]
    cmp rax, rdx
    jbe .Lprint_counts_assert_second
    dec rax
    mov byte ptr [rax], SPACE
    mov rdi, rbx
    mov rsi, rax
    call digits
    lea rdx, [rip + out]
    cmp rax, rdx
    jb .Lprint_counts_assert_first
    add rdx, OUT_LEN
    sub rdx, rax
    mov rsi, rax
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lprint_counts_return
    xor eax, eax
.Lprint_counts_return:
    add rsp, 16
    pop rbx
    ret
.Lprint_counts_assert_second:
    mov edi, 11
    call assert_fail
.Lprint_counts_assert_first:
    mov edi, 12
    call assert_fail
.size print_counts, . - print_counts

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
    mov edi, 8
    call assert_fail
.Ldigits_assert_digits:
    mov edi, 9
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
    mov edi, 19
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 20
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 21
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
