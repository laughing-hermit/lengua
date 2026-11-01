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
# fill's reads at most, and what makes it read to the end
.equ MAX_FILL_READS, 1099511627776
.equ READ_TO_END, -1
# Bytes that matter
.equ NEWLINE, 10
.equ SPACE, 32
.equ TAB, 9
# Tab through carriage return are the 5 bytes from 9 to 13.
.equ TAB_TO_CR, 4
# A buffer-full of input, the most the main loop takes at a time, and its 64-byte steps
.equ BUF_LEN, 65536
.equ VECTOR, 64
.equ STEPS, BUF_LEN / VECTOR
# The buffer-fulls the main loop takes at most
.equ MAX_FULLS, 1099511627776
.equ MAX_DIGITS, 20
.equ MAX_WRITES, 64
.equ OUT_LEN, 42

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# One buffer-full of standard input.
.type buffer, @object
buffer:
    .zero BUF_LEN
.size buffer, . - buffer

# The line it prints: two numbers of up to 20 digits, a space and a newline.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
.globl _start

# Counts the lines and words of standard input of any length, a buffer-full at a time through
# fill, and prints both. What crosses from one buffer-full to the next is r14: 1 when the last
# byte read is whitespace, or nothing has been read.
.type _start, @function
_start:
    xor r12d, r12d
    xor r13d, r13d
    mov r14d, 1
    mov rbx, MAX_FULLS
.Lstart_full:  # bound: 1099511627776
    mov edi, STDIN
    lea rsi, [rip + buffer]
    mov edx, BUF_LEN
    mov ecx, READ_TO_END
    call fill
    test rax, rax
    js .Lstart_failed
    jz .Lstart_print
    cmp rax, BUF_LEN
    ja .Lstart_assert_count
    mov r15, rax
    lea rdi, [rip + buffer]
    mov rsi, rax
    mov edx, r14d
    call count_piece
    add r12, rax
    add r13, rdx
    mov r14, rcx
    cmp r15, BUF_LEN
    jb .Lstart_print
    dec rbx
    jnz .Lstart_full
    jmp .Lstart_assert_fulls
.Lstart_print:
    mov rdi, r12
    mov rsi, r13
    call print_counts
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
.Lstart_assert_fulls:
    mov edi, 2
    call assert_fail
.size _start, . - _start

# Counts the newlines and word starts in a piece of the buffer, 64 bytes a step with AVX-512
# masks. Each step's bytes past the end are masked off, so the last partial step needs no code
# of its own, and no step's work depends on where the piece ends but what it counts.
# In: rdi = the bytes; rsi = how many, 1 to BUF_LEN; edx = 1 when the byte before them is
# whitespace (or there is none), else 0.
# Out: rax = newlines; rdx = word starts; rcx = 1 when the last byte is whitespace, else 0.
.type count_piece, @function
count_piece:
    test rdi, rdi
    jz .Lcount_piece_assert_bytes
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
    mov r11, STEPS
.Lcount_piece_loop:  # bound: 1024
    cmp rcx, rsi
    jae .Lcount_piece_done
    cmp rcx, BUF_LEN - VECTOR + 1
    jae .Lcount_piece_assert_index
    # k1: this step's bytes before the end, min(rsi - rcx, 64) of them.
    mov rax, rsi
    sub rax, rcx
    mov edx, VECTOR
    cmp rax, rdx
    cmova rax, rdx
    mov rdx, -1
    bzhi rdx, rdx, rax
    kmovq k1, rdx
    dec eax
    vmovdqu8 zmm0, zmmword ptr [rdi + rcx]
    vpcmpeqb k2 {k1}, zmm0, zmm1
    kmovq rdx, k2
    popcnt rdx, rdx
    add r8, rdx
    # k3: whitespace, a space or a byte from tab to carriage return.
    vpcmpeqb k3 {k1}, zmm0, zmm2
    vpsubb zmm5, zmm0, zmm3
    vpcmpub k4 {k1}, zmm5, zmm4, 2
    korq k3, k3, k4
    kmovq rdx, k3
    # Whether this step's last byte is whitespace, for the next step.
    shrx rax, rdx, rax
    and eax, 1
    # A word starts at a byte that isn't whitespace where the one before is.
    add rdx, rdx
    or rdx, r10
    kmovq k6, rdx
    kandnq k5, k3, k1
    kandq k5, k5, k6
    kmovq rdx, k5
    popcnt rdx, rdx
    add r9, rdx
    mov r10, rax
    add rcx, VECTOR
    dec r11
    jnz .Lcount_piece_loop
.Lcount_piece_done:
    mov rax, r8
    mov rdx, r9
    mov rcx, r10
    ret
.Lcount_piece_assert_bytes:
    mov edi, 3
    call assert_fail
.Lcount_piece_assert_index:
    mov edi, 4
    call assert_fail
.size count_piece, . - count_piece

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
