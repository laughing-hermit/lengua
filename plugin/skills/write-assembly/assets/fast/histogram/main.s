# lengua: spec sha256:51f6ce69592c9964db7f83e4029fa6a30f2b472c1934d3308700918d7c96549b
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
# Four tables of 256 counts each, so neighboring bytes never wait on the same counter.
.equ VALUES, 256
.equ TABLE_BYTES, 2048
.equ COUNTS_LEN, 8192
.equ EIGHT, 8
.equ EIGHTS, 131073
# Sizes and limits
.equ INPUT_LEN, 1048576
.equ MAX_READS, 1099511627776
.equ MAX_DIGITS, 20
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ SPACE, 32
# At most 256 lines of a 3-digit value, a space, a 20-digit count and a newline.
.equ OUT_LEN, 6400

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# Standard input as it comes.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

# The four tables of counts, one after another.
.type counts, @object
counts:
    .zero COUNTS_LEN
.size counts, . - counts

# Every line it prints, built from the end.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
.globl _start

# Reads standard input as it comes, counting each byte value, then prints the counts.
.type _start, @function
_start:
    mov rbx, MAX_READS
.Lstart_read:  # bound: 1099511627776
    mov eax, SYS_READ
    mov edi, STDIN
    lea rsi, [rip + input]
    mov edx, INPUT_LEN
    syscall
    test rax, rax
    js .Lstart_failed
    jz .Lstart_print
    cmp rax, INPUT_LEN
    ja .Lstart_assert_count
    lea rdi, [rip + input]
    mov rsi, rax
    lea rdx, [rip + counts]
    call count_bytes
    dec rbx
    jnz .Lstart_read
    jmp .Lstart_assert_reads
.Lstart_print:
    lea rdi, [rip + counts]
    call print_histogram
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

# Adds each byte to the count of its value, eight at a step, spreading neighbors over the four
# tables.
# In: rdi = the bytes; rsi = how many, up to INPUT_LEN; rdx = the four tables, COUNTS_LEN bytes.
.type count_bytes, @function
count_bytes:
    test rdi, rdi
    jz .Lcount_bytes_assert_bytes
    test rdx, rdx
    jz .Lcount_bytes_assert_counts
    cmp rsi, INPUT_LEN
    ja .Lcount_bytes_assert_length
    xor ecx, ecx
    mov r11, EIGHTS
.Lcount_bytes_eight:  # bound: 131073
    lea r8, [rcx + EIGHT]
    cmp r8, rsi
    ja .Lcount_bytes_rest
    cmp rcx, INPUT_LEN - EIGHT + 1
    jae .Lcount_bytes_assert_index
    # unsafe: each index is one byte, 0 to 255, and each table holds 256 counts
    movzx r9d, byte ptr [rdi + rcx + 0]
    inc qword ptr [rdx + r9 * 8 + 0 * TABLE_BYTES]
    movzx r10d, byte ptr [rdi + rcx + 1]
    inc qword ptr [rdx + r10 * 8 + 1 * TABLE_BYTES]
    movzx r9d, byte ptr [rdi + rcx + 2]
    inc qword ptr [rdx + r9 * 8 + 2 * TABLE_BYTES]
    movzx r10d, byte ptr [rdi + rcx + 3]
    inc qword ptr [rdx + r10 * 8 + 3 * TABLE_BYTES]
    movzx r9d, byte ptr [rdi + rcx + 4]
    inc qword ptr [rdx + r9 * 8 + 0 * TABLE_BYTES]
    movzx r10d, byte ptr [rdi + rcx + 5]
    inc qword ptr [rdx + r10 * 8 + 1 * TABLE_BYTES]
    movzx r9d, byte ptr [rdi + rcx + 6]
    inc qword ptr [rdx + r9 * 8 + 2 * TABLE_BYTES]
    movzx r10d, byte ptr [rdi + rcx + 7]
    inc qword ptr [rdx + r10 * 8 + 3 * TABLE_BYTES]
    # end unsafe
    add rcx, EIGHT
    dec r11
    jnz .Lcount_bytes_eight
    jmp .Lcount_bytes_assert_eights
.Lcount_bytes_rest:
    sub rsp, 8
    add rdi, rcx
    sub rsi, rcx
    call count_rest
    add rsp, 8
    ret
.Lcount_bytes_assert_bytes:
    mov edi, 6
    call assert_fail
.Lcount_bytes_assert_counts:
    mov edi, 7
    call assert_fail
.Lcount_bytes_assert_length:
    mov edi, 8
    call assert_fail
.Lcount_bytes_assert_index:
    mov edi, 9
    call assert_fail
.Lcount_bytes_assert_eights:
    mov edi, 10
    call assert_fail
.size count_bytes, . - count_bytes

# Adds the last bytes, fewer than eight, to the counts of their values in the first table.
# In: rdi = the bytes; rsi = how many, 0 to 7; rdx = the four tables, COUNTS_LEN bytes.
.type count_rest, @function
count_rest:
    test rdx, rdx
    jz .Lcount_rest_assert_counts
    cmp rsi, EIGHT
    jae .Lcount_rest_assert_length
    xor ecx, ecx
    mov r11, EIGHT
.Lcount_rest_loop:  # bound: 8
    cmp rcx, rsi
    jae .Lcount_rest_done
    cmp rcx, EIGHT
    jae .Lcount_rest_assert_index
    movzx r9d, byte ptr [rdi + rcx]
    # unsafe: the index is one byte, 0 to 255, and the table holds 256 counts
    inc qword ptr [rdx + r9 * 8]
    # end unsafe
    inc rcx
    dec r11
    jnz .Lcount_rest_loop
.Lcount_rest_done:
    ret
.Lcount_rest_assert_counts:
    mov edi, 11
    call assert_fail
.Lcount_rest_assert_length:
    mov edi, 14
    call assert_fail
.Lcount_rest_assert_index:
    mov edi, 15
    call assert_fail
.size count_rest, . - count_rest

# Prints a line for each byte value that occurs, smallest first: the value, a space, the count.
# In: rdi = the four tables of counts.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_histogram, @function
print_histogram:
    push rbx
    push r12
    push r13
    push r14
    push r15
    test rdi, rdi
    jz .Lprint_histogram_assert_counts
    mov rbx, rdi
    lea r12, [rip + out]
    add r12, OUT_LEN
    mov r13, r12
    mov r14d, VALUES - 1
    mov r15, VALUES
.Lprint_histogram_loop:  # bound: 256
    cmp r14, VALUES
    jae .Lprint_histogram_assert_value
    mov rax, qword ptr [rbx + r14 * 8]
    add rax, qword ptr [rbx + r14 * 8 + TABLE_BYTES]
    add rax, qword ptr [rbx + r14 * 8 + 2 * TABLE_BYTES]
    add rax, qword ptr [rbx + r14 * 8 + 3 * TABLE_BYTES]
    test rax, rax
    jz .Lprint_histogram_next
    dec r13
    mov byte ptr [r13], NEWLINE
    mov rdi, rax
    mov rsi, r13
    call digits
    dec rax
    mov byte ptr [rax], SPACE
    mov rdi, r14
    mov rsi, rax
    call digits
    mov r13, rax
.Lprint_histogram_next:
    dec r14
    dec r15
    jnz .Lprint_histogram_loop
    mov rdx, r12
    sub rdx, r13
    jz .Lprint_histogram_empty
    mov rsi, r13
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lprint_histogram_return
.Lprint_histogram_empty:
    xor eax, eax
.Lprint_histogram_return:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Lprint_histogram_assert_counts:
    mov edi, 12
    call assert_fail
.Lprint_histogram_assert_value:
    mov edi, 13
    call assert_fail
.size print_histogram, . - print_histogram

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
