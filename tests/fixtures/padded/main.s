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
.equ EXIT_READ_FAILED, 1
.equ EXIT_WRITE_FAILED, 2
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# Bytes that matter
.equ NEWLINE, 10
.equ SPACE, 32
.equ TAB, 9
# Tab through carriage return are the 5 bytes from 9 to 13.
.equ TAB_TO_CR, 4
# Sizes and limits: a read's bytes at most, and its 16-byte steps
.equ INPUT_LEN, 65536
.equ VECTOR, 16
.equ STEPS, INPUT_LEN / VECTOR
.equ MAX_READS, 1099511627776
.equ MAX_DIGITS, 20
.equ MAX_WRITES, 64
.equ OUT_LEN, 42
# Each count is right-aligned in a field this wide, as GNU wc does for standard input.
.equ FIELD, 7

.section .rodata

# What each step compares its 16 bytes with, one vector of 16 lanes each: newlines, spaces,
# tabs, the distance from tab to carriage return, and each lane's place plus one, 1 to 16. A
# lane is a byte of the input where its place plus one is at most the bytes left.
.type classes, @object
classes:
    .byte NEWLINE, NEWLINE, NEWLINE, NEWLINE, NEWLINE, NEWLINE, NEWLINE, NEWLINE
    .byte NEWLINE, NEWLINE, NEWLINE, NEWLINE, NEWLINE, NEWLINE, NEWLINE, NEWLINE
    .byte SPACE, SPACE, SPACE, SPACE, SPACE, SPACE, SPACE, SPACE
    .byte SPACE, SPACE, SPACE, SPACE, SPACE, SPACE, SPACE, SPACE
    .byte TAB, TAB, TAB, TAB, TAB, TAB, TAB, TAB
    .byte TAB, TAB, TAB, TAB, TAB, TAB, TAB, TAB
    .byte TAB_TO_CR, TAB_TO_CR, TAB_TO_CR, TAB_TO_CR, TAB_TO_CR, TAB_TO_CR, TAB_TO_CR, TAB_TO_CR
    .byte TAB_TO_CR, TAB_TO_CR, TAB_TO_CR, TAB_TO_CR, TAB_TO_CR, TAB_TO_CR, TAB_TO_CR, TAB_TO_CR
    .byte 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16
.size classes, . - classes

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

# Reads standard input 64 KiB at a time, counting newlines and word starts, then prints both.
.type _start, @function
_start:
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
    mov r10d, r14d
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

# Counts the newlines and word starts in one read's worth of input, 16 bytes a step with SSE2,
# plus pshufb and popcnt, so it runs on every x86-64 computer. Each step's bytes past the end
# are masked off, lanes whose place isn't below the bytes left, so the last partial step needs
# no code of its own. A comparison's lanes, all ones or zero, become one bit each with pmovmskb,
# and popcnt adds them.
# In: rdi = the bytes; rsi = how many, 1 to INPUT_LEN; r10d = 1 when the byte before them is
# whitespace (or there is none), else 0.
# Out: rax = newlines; rdx = word starts; rcx = 1 when the last byte is whitespace, else 0.
.type count_piece, @function
count_piece:
    test rdi, rdi
    jz .Lcount_piece_assert_bytes
    movdqu xmm1, xmmword ptr [rip + classes]
    movdqu xmm5, xmmword ptr [rip + classes + 16]
    movdqu xmm7, xmmword ptr [rip + classes + 32]
    movdqu xmm8, xmmword ptr [rip + classes + 48]
    movdqu xmm9, xmmword ptr [rip + classes + 64]
    pxor xmm10, xmm10
    xor eax, eax
    xor edx, edx
    xor ecx, ecx
    mov r11, STEPS
.Lcount_piece_loop:  # bound: 4096
    cmp rcx, rsi
    jae .Lcount_piece_done
    cmp rcx, INPUT_LEN - VECTOR + 1
    jae .Lcount_piece_assert_index
    # xmm6: the lanes of this step that hold bytes before the end, those whose place plus one
    # is at most min(rsi - rcx, 16): that number in every lane, its smaller with the lane's place
    # plus one, and the lanes where that is the place plus one.
    mov r8, rsi
    sub r8, rcx
    mov r9d, VECTOR
    cmp r8, r9
    cmova r8, r9
    movd xmm6, r8d
    pshufb xmm6, xmm10
    pminub xmm6, xmm9
    pcmpeqb xmm6, xmm9
    movdqu xmm0, xmmword ptr [rdi + rcx]
    # Newlines before the end.
    movdqu xmm2, xmm0
    pcmpeqb xmm2, xmm1
    pand xmm2, xmm6
    pmovmskb r8d, xmm2
    popcnt r8d, r8d
    add rax, r8
    # Whitespace: a space, or a byte from tab to carriage return.
    movdqu xmm3, xmm0
    pcmpeqb xmm3, xmm5
    psubb xmm0, xmm7
    psubusb xmm0, xmm8
    pcmpeqb xmm0, xmm10
    por xmm3, xmm0
    pmovmskb r8d, xmm3
    # A word starts at a byte that isn't whitespace where the one before is: the whitespace
    # mask moved up a place, with the byte before the step as its first bit.
    lea r9d, [r8 + r8]
    or r9d, r10d
    # Whether this step's last byte is whitespace, for the next step: the mask's top bit.
    mov r10d, r8d
    shr r10d, 15
    # The bytes before the end that aren't whitespace.
    pandn xmm3, xmm6
    pmovmskb r8d, xmm3
    and r9d, r8d
    popcnt r9d, r9d
    add rdx, r9
    add rcx, VECTOR
    dec r11
    jnz .Lcount_piece_loop
.Lcount_piece_done:
    # Whether the piece's last byte is whitespace, for the next piece: it is where the last
    # step's lane for it, place (rsi - 1) mod 16, isn't one of the bytes that aren't.
    lea r9d, [rsi - 1]
    bt r8w, r9w
    setae cl
    movzx ecx, cl
    ret
.Lcount_piece_assert_bytes:
    mov edi, 3
    call assert_fail
.Lcount_piece_assert_index:
    mov edi, 4
    call assert_fail
.size count_piece, . - count_piece

# Prints two whole numbers, each right-aligned in FIELD columns, a space between them and a
# newline after, to standard output.
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
    mov rdi, rax
    lea rsi, [rip + out + OUT_LEN - 1]
    call pad
    lea rdx, [rip + out]
    cmp rax, rdx
    jbe .Lprint_counts_assert_second
    dec rax
    mov byte ptr [rax], SPACE
    mov rdi, rbx
    mov rsi, rax
    mov rbx, rax
    call digits
    mov rdi, rax
    mov rsi, rbx
    call pad
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

# Puts spaces before a number's digits until the field is FIELD characters wide.
# In: rdi = where the digits start; rsi = where they end, at or after rdi.
# Out: rax = where the field starts.
.type pad, @function
pad:
    test rdi, rdi
    jz .Lpad_assert_start
    cmp rdi, rsi
    ja .Lpad_assert_order
    mov rcx, FIELD
.Lpad_loop:  # bound: 7
    mov rax, rsi
    sub rax, rdi
    cmp rax, FIELD
    jae .Lpad_done
    dec rdi
    mov byte ptr [rdi], SPACE
    dec rcx
    jnz .Lpad_loop
.Lpad_done:
    mov rax, rdi
    ret
.Lpad_assert_start:
    mov edi, 13
    call assert_fail
.Lpad_assert_order:
    mov edi, 14
    call assert_fail
.size pad, . - pad

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
