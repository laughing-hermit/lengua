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
# A buffer-full of input, the most the main loop takes at a time, and its 16-byte chunks
.equ BUF_LEN, 4096
.equ CHUNK, 16
.equ BUF_CHUNKS, BUF_LEN / CHUNK
# The buffer-fulls the main loop takes at most
.equ MAX_FULLS, 1099511627776
.equ MAX_WRITES, 64
# Bytes repeated four times, spread over a vector register with pshufd
.equ BELOW_A_X4, 0x60606060
.equ ABOVE_Z_X4, 0x7b7b7b7b
.equ CASE_BIT_X4, 0x20202020

.section .bss

# One buffer-full of standard input, capitalized in place, then written.
# shared: main fills and writes it, and shout capitalizes it
.type buffer, @object
buffer:
    .zero BUF_LEN
.size buffer, . - buffer

.text
# Copies standard input of any length to standard output with every ASCII letter in capitals, a
# buffer-full at a time, until the input ends.
.type main, @function
main:
    sub rsp, 8
    mov rbx, MAX_FULLS
.Lmain_full:  # bound: 1099511627776
    mov edi, STDIN
    lea rsi, [rip + buffer]
    mov edx, BUF_LEN
    mov ecx, READ_TO_END
    call fill
    test rax, rax
    js .Lmain_read_failed
    jz .Lmain_done
    cmp rax, BUF_LEN
    ja .Lmain_assert_count
    mov r12, rax
    mov rdi, rax
    call shout
    mov edi, STDOUT
    lea rsi, [rip + buffer]
    mov rdx, r12
    call write_all
    test rax, rax
    js .Lmain_write_failed
    cmp r12, BUF_LEN
    jb .Lmain_done
    dec rbx
    jnz .Lmain_full
.Lmain_done:
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

# Capitalizes the first rdi bytes of buffer, 16 at a time: a to z become A to Z, other bytes stay.
# The last chunk may change bytes past them, which is harmless: only those bytes are written.
# In: rdi = how many, 1 to BUF_LEN.
.type shout, @function
shout:
    cmp rdi, BUF_LEN
    ja .Lshout_assert_length
    lea rsi, [rip + buffer]
    mov eax, BELOW_A_X4
    movd xmm1, eax
    pshufd xmm1, xmm1, 0
    mov eax, ABOVE_Z_X4
    movd xmm2, eax
    pshufd xmm2, xmm2, 0
    mov eax, CASE_BIT_X4
    movd xmm3, eax
    pshufd xmm3, xmm3, 0
    xor ecx, ecx
    mov r8, BUF_CHUNKS
.Lshout_loop:  # bound: 256
    cmp rcx, rdi
    jae .Lshout_return
    cmp rcx, BUF_LEN - CHUNK + 1
    jae .Lshout_assert_index
    movdqu xmm0, xmmword ptr [rsi + rcx]
    movdqu xmm4, xmm0
    pcmpgtb xmm4, xmm1
    movdqu xmm5, xmm2
    pcmpgtb xmm5, xmm0
    pand xmm4, xmm5
    pand xmm4, xmm3
    psubb xmm0, xmm4
    movdqu xmmword ptr [rsi + rcx], xmm0
    add rcx, CHUNK
    dec r8
    jnz .Lshout_loop
.Lshout_return:
    ret
.Lshout_assert_length:
    mov edi, 2
    call assert_fail
.Lshout_assert_index:
    mov edi, 3
    call assert_fail
.size shout, . - shout

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

