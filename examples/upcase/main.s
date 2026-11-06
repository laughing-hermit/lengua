.intel_syntax noprefix

# File descriptors and os_open's modes
.equ STDERR, 2
.equ OPEN_READ, 0
.equ OPEN_WRITE, 1
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_CANNOT_OPEN, 2
.equ EXIT_TOO_LARGE, 3
.equ EXIT_IO_FAILED, 4
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# Sizes and limits
.equ ARGC_EXPECTED, 3
.equ MAX_INPUT, 1048576
.equ READ_CAP, MAX_INPUT + 1
.equ MAX_READS, READ_CAP + 1
.equ CHUNK, 16
.equ BUF_LEN, 1048592
.equ MAX_CHUNKS, MAX_INPUT / CHUNK
.equ MAX_WRITES, 4096
.equ MAX_ARG_LEN, 131072
.equ USAGE_MSG_LEN, 31
.equ OPEN_PREFIX_LEN, 20
.equ NEWLINE_MSG_LEN, 1
.equ LARGE_PREFIX_LEN, 8
.equ LARGE_SUFFIX_LEN, 30

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: upcase <input> <output>\n"
.size usage_msg, . - usage_msg

.type open_prefix, @object
open_prefix:
    .ascii "upcase: cannot open "
.size open_prefix, . - open_prefix

.type newline_msg, @object
newline_msg:
    .ascii "\n"
.size newline_msg, . - newline_msg

.type large_prefix, @object
large_prefix:
    .ascii "upcase: "
.size large_prefix, . - large_prefix

.type large_suffix, @object
large_suffix:
    .ascii " is larger than 1048576 bytes\n"
.size large_suffix, . - large_suffix

.section .bss

# The input file, one byte more than the largest allowed, rounded up to whole chunks.
.type buf, @object
buf:
    .zero BUF_LEN
.size buf, . - buf

.text
# Checks the arguments, reads the input, capitalizes it and writes the output, then exits.
.type main, @function
main:
    sub rsp, 8
    mov r14, rsi
    cmp rdi, ARGC_EXPECTED
    jne .Lmain_usage
    mov rdi, qword ptr [r14 + 8]
    test rdi, rdi
    jz .Lmain_assert_argv
    mov esi, OPEN_READ
    call os_open
    test rax, rax
    js .Lmain_cannot_open
    mov edi, eax
    lea rsi, [rip + buf]
    call load_input
    test rax, rax
    js .Lmain_io_failed
    mov r12, rdx
    cmp r12, MAX_INPUT
    ja .Lmain_too_large
    lea rdi, [rip + buf]
    mov rsi, r12
    call upcase_buf
    mov rdi, qword ptr [r14 + 16]
    lea rsi, [rip + buf]
    mov rdx, r12
    call write_output
    test rax, rax
    js .Lmain_io_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_usage:
    mov edi, STDERR
    lea rsi, [rip + usage_msg]
    mov edx, USAGE_MSG_LEN
    call write_all
    test rax, rax
    js .Lmain_io_failed
    mov edi, EXIT_USAGE
    jmp .Lmain_exit
.Lmain_cannot_open:
    mov rdi, qword ptr [r14 + 8]
    call report_cannot_open
    test rax, rax
    js .Lmain_io_failed
    mov edi, EXIT_CANNOT_OPEN
    jmp .Lmain_exit
.Lmain_too_large:
    mov rdi, qword ptr [r14 + 8]
    call report_too_large
    test rax, rax
    js .Lmain_io_failed
    mov edi, EXIT_TOO_LARGE
    jmp .Lmain_exit
.Lmain_io_failed:
    mov edi, EXIT_IO_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_argv:
    mov edi, 1
    call assert_fail
.size main, . - main

# Reads all of file descriptor edi into rsi, then closes it.
# In: edi = the file descriptor, rsi = a buffer of at least READ_CAP bytes.
# Out: rax = 0 and rdx = the bytes read, or rax = a negative error code.
# returns: result
.type load_input, @function
load_input:
    push rbx
    push r12
    sub rsp, 8
    test rsi, rsi
    jz .Lload_assert_null
    cmp edi, STDERR
    jbe .Lload_assert_fd
    mov ebx, edi
    call read_all
    test rax, rax
    js .Lload_return
    mov r12, rdx
    mov edi, ebx
    call os_close
    test rax, rax
    js .Lload_return
    mov rdx, r12
.Lload_return:
    add rsp, 8
    pop r12
    pop rbx
    ret
.Lload_assert_null:
    mov edi, 19
    call assert_fail
.Lload_assert_fd:
    mov edi, 20
    call assert_fail
.size load_input, . - load_input

# Prints "upcase: cannot open <name>" and a newline to standard error.
# In: rdi = the NUL-terminated file name.
# Out: rax = 0, or a negative error code.
# returns: result
.type report_cannot_open, @function
report_cannot_open:
    sub rsp, 8
    test rdi, rdi
    jz .Lreport_open_assert_null
    mov rdx, rdi
    lea rdi, [rip + open_prefix]
    mov esi, OPEN_PREFIX_LEN
    lea rcx, [rip + newline_msg]
    mov r8d, NEWLINE_MSG_LEN
    call report_name
    test rax, rax
    js .Lreport_open_return
    xor eax, eax
.Lreport_open_return:
    add rsp, 8
    ret
.Lreport_open_assert_null:
    mov edi, 21
    call assert_fail
.size report_cannot_open, . - report_cannot_open

# Prints "upcase: <name> is larger than 1048576 bytes" and a newline to standard error.
# In: rdi = the NUL-terminated file name.
# Out: rax = 0, or a negative error code.
# returns: result
.type report_too_large, @function
report_too_large:
    sub rsp, 8
    test rdi, rdi
    jz .Lreport_large_assert_null
    mov rdx, rdi
    lea rdi, [rip + large_prefix]
    mov esi, LARGE_PREFIX_LEN
    lea rcx, [rip + large_suffix]
    mov r8d, LARGE_SUFFIX_LEN
    call report_name
    test rax, rax
    js .Lreport_large_return
    xor eax, eax
.Lreport_large_return:
    add rsp, 8
    ret
.Lreport_large_assert_null:
    mov edi, 22
    call assert_fail
.size report_too_large, . - report_too_large

# Reads file descriptor edi into rsi until the end of input or READ_CAP bytes.
# In: edi = the file descriptor, rsi = a buffer of at least READ_CAP bytes.
# Out: rax = 0 and rdx = the bytes read, or rax = a negative error code.
# returns: result
.type read_all, @function
read_all:
    sub rsp, 8
    test rsi, rsi
    jz .Lread_all_assert_null
    mov r10, rsi
    xor r9d, r9d
    mov r8, MAX_READS
.Lread_all_loop:  # bound: 1048578
    cmp r9, READ_CAP
    jae .Lread_all_done
    lea rsi, [r10 + r9]
    mov edx, READ_CAP
    sub rdx, r9
    call os_read
    test rax, rax
    js .Lread_all_return
    jz .Lread_all_done
    cmp rax, rdx
    ja .Lread_all_assert_count
    add r9, rax
    dec r8
    jnz .Lread_all_loop
    jmp .Lread_all_assert_reads
.Lread_all_done:
    mov rdx, r9
    xor eax, eax
.Lread_all_return:
    add rsp, 8
    ret
.Lread_all_assert_null:
    mov edi, 2
    call assert_fail
.Lread_all_assert_count:
    mov edi, 3
    call assert_fail
.Lread_all_assert_reads:
    mov edi, 4
    call assert_fail
.size read_all, . - read_all

# Turns the ASCII letters a to z into capitals, 16 bytes at a time, in place.
# In: rdi = a buffer of BUF_LEN bytes, rsi = how many bytes hold text, at most MAX_INPUT.
# Bytes past the text, up to the next whole chunk, change too but are never written out.
.type upcase_buf, @function
upcase_buf:
    test rdi, rdi
    jz .Lupcase_assert_null
    cmp rsi, MAX_INPUT
    ja .Lupcase_assert_len
    test rsi, rsi
    jz .Lupcase_return
    mov eax, 0x1f1f1f1f
    movd xmm1, eax
    pshufd xmm1, xmm1, 0
    mov eax, 0x9a9a9a9a
    movd xmm2, eax
    pshufd xmm2, xmm2, 0
    mov eax, 0x20202020
    movd xmm3, eax
    pshufd xmm3, xmm3, 0
    lea r9, [rsi + CHUNK - 1]
    shr r9, 4
    xor ecx, ecx
    mov r8, MAX_CHUNKS
.Lupcase_loop:  # bound: 65536
    cmp rcx, BUF_LEN - CHUNK + 1
    jae .Lupcase_assert_index
    movdqu xmm0, xmmword ptr [rdi + rcx]
    pshufd xmm4, xmm0, 0xe4
    paddb xmm4, xmm1
    pshufd xmm5, xmm2, 0xe4
    pcmpgtb xmm5, xmm4
    pand xmm5, xmm3
    pxor xmm0, xmm5
    movdqu xmmword ptr [rdi + rcx], xmm0
    add rcx, CHUNK
    dec r9
    jz .Lupcase_return
    dec r8
    jnz .Lupcase_loop
    jmp .Lupcase_assert_chunks
.Lupcase_return:
    ret
.Lupcase_assert_null:
    mov edi, 5
    call assert_fail
.Lupcase_assert_len:
    mov edi, 6
    call assert_fail
.Lupcase_assert_index:
    mov edi, 7
    call assert_fail
.Lupcase_assert_chunks:
    mov edi, 8
    call assert_fail
.size upcase_buf, . - upcase_buf

# Creates or replaces the file named rdi and writes rdx bytes from rsi to it.
# In: rdi = a NUL-terminated file name, rsi = the bytes, rdx = how many, at most MAX_INPUT.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_output, @function
write_output:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Lwrite_output_assert_null
    cmp rdx, MAX_INPUT
    ja .Lwrite_output_assert_len
    mov r12, rsi
    mov r13, rdx
    mov esi, OPEN_WRITE
    call os_open
    test rax, rax
    js .Lwrite_output_return
    mov rbx, rax
    test r13, r13
    jz .Lwrite_output_close
    mov edi, ebx
    mov rsi, r12
    mov rdx, r13
    call write_all
    test rax, rax
    js .Lwrite_output_return
.Lwrite_output_close:
    mov edi, ebx
    call os_close
    test rax, rax
    js .Lwrite_output_return
    xor eax, eax
.Lwrite_output_return:
    pop r13
    pop r12
    pop rbx
    ret
.Lwrite_output_assert_null:
    mov edi, 9
    call assert_fail
.Lwrite_output_assert_len:
    mov edi, 10
    call assert_fail
.size write_output, . - write_output

# Writes a prefix, a file name and a suffix to standard error.
# In: rdi = the prefix, rsi = its length, rdx = a NUL-terminated file name, rcx = the suffix,
# r8 = its length. Both lengths are at least 1.
# Out: rax = 0, or a negative error code.
# returns: result
.type report_name, @function
report_name:
    push rbx
    push r12
    push r13
    test rdx, rdx
    jz .Lreport_assert_name
    test rcx, rcx
    jz .Lreport_assert_suffix
    mov rbx, rdx
    mov r12, rcx
    mov r13, r8
    mov rdx, rsi
    mov rsi, rdi
    mov edi, STDERR
    call write_all
    test rax, rax
    js .Lreport_return
    mov rdi, rbx
    call str_len
    test rax, rax
    jz .Lreport_suffix
    mov rdx, rax
    mov rsi, rbx
    mov edi, STDERR
    call write_all
    test rax, rax
    js .Lreport_return
.Lreport_suffix:
    mov edi, STDERR
    mov rsi, r12
    mov rdx, r13
    call write_all
    test rax, rax
    js .Lreport_return
    xor eax, eax
.Lreport_return:
    pop r13
    pop r12
    pop rbx
    ret
.Lreport_assert_name:
    mov edi, 11
    call assert_fail
.Lreport_assert_suffix:
    mov edi, 12
    call assert_fail
.size report_name, . - report_name

# Counts the bytes of a NUL-terminated string.
# In: rdi = the string, shorter than MAX_ARG_LEN bytes.
# Out: rax = its length.
.type str_len, @function
str_len:
    test rdi, rdi
    jz .Lstr_len_assert_null
    xor eax, eax
    mov r8, MAX_ARG_LEN
.Lstr_len_loop:  # bound: 131072
    cmp rax, MAX_ARG_LEN
    jae .Lstr_len_assert_index
    cmp byte ptr [rdi + rax], 0
    je .Lstr_len_return
    inc rax
    dec r8
    jnz .Lstr_len_loop
    jmp .Lstr_len_assert_long
.Lstr_len_return:
    ret
.Lstr_len_assert_null:
    mov edi, 13
    call assert_fail
.Lstr_len_assert_index:
    mov edi, 14
    call assert_fail
.Lstr_len_assert_long:
    mov edi, 15
    call assert_fail
.size str_len, . - str_len

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
.Lwrite_all_loop:  # bound: 4096
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
    mov edi, 16
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 17
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 18
    call assert_fail
.size write_all, . - write_all

