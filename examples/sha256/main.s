.intel_syntax noprefix

# SHA-256 in plain integer code (rotates, adds and xors) and SSE2, so it runs on every x86-64
# computer. The form that uses the CPU's SHA instructions, from the very same idea, is
# plugin/skills/compile/assets/fast/sha256-ni.

# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 2
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# fill's constant
# Sizes and limits: standard input is hashed in pieces of INPUT_LEN bytes, 64 blocks of 64
.equ INPUT_LEN, 4096
.equ BLOCK_BYTES, 64
.equ BLOCK_STEPS, 65
.equ TAIL_MASK, 63
.equ MAX_CHUNKS, 268435456
.equ STATE_WORDS, 8
.equ STATE_LEN, 32
.equ SCHEDULE_LEN, 256
.equ LOAD_STEPS, 16
.equ EXTEND_STEPS, 48
.equ ROUND_STEPS, 64
.equ WORK_A_LEN, 272
.equ WORK_E_LEN, 272
.equ PADDED_LEN, 128
.equ LENGTH_AT_ONE, 56
.equ LENGTH_AT_TWO, 120
.equ MAX_COPY, 63
.equ PAD_BYTE, 0x80
.equ HEX_DIGITS_COUNT, 64
.equ NIBBLES, 8
.equ OUT_LEN, 65
.equ NEWLINE, 10
.equ MAX_WRITES, 64

.section .rodata

# The 64 round constants: the first 32 bits of the fractional parts of the cube roots of the
# first 64 primes.
.type k_table, @object
k_table:
    .long 0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5
    .long 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5
    .long 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3
    .long 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174
    .long 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc
    .long 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da
    .long 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7
    .long 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967
    .long 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13
    .long 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85
    .long 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3
    .long 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070
    .long 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5
    .long 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3
    .long 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208
    .long 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
.size k_table, . - k_table

# The first hash value: the first 32 bits of the fractional parts of the square roots of the
# first 8 primes.
.type initial_state, @object
initial_state:
    .long 0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a
    .long 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
.size initial_state, . - initial_state

.type hex_digits, @object
hex_digits:
    .ascii "0123456789abcdef"
.size hex_digits, . - hex_digits

.section .bss

# Standard input as it comes, one piece at a time.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

# The eight words of the hash value, a to h.
.type state, @object
state:
    .zero STATE_LEN
.size state, . - state

# The 64 words of one block's message schedule.
# shared: schedule_words writes it and sha_round reads it, one block at a time
.type schedule, @object
schedule:
    .zero SCHEDULE_LEN
.size schedule, . - schedule

# The working variable a of every round, in time order: d, c, b and a before the first round,
# then the a each round makes. Round t reads words t to t + 3 (d, c, b, a) and writes word t + 4.
# shared: rounds sets the first four words and reads the last four; sha_round writes the rest
.type work_a, @object
work_a:
    .zero WORK_A_LEN
.size work_a, . - work_a

# The working variable e of every round, in time order, as work_a keeps a: h, g, f and e first.
# shared: rounds sets the first four words and reads the last four; sha_round writes the rest
.type work_e, @object
work_e:
    .zero WORK_E_LEN
.size work_e, . - work_e

# The last one or two blocks: the message's tail, a 1 bit, zeros and its length in bits.
.type padded, @object
padded:
    .zero PADDED_LEN
.size padded, . - padded

# The digest in hexadecimal and a newline.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
# Hashes standard input a piece at a time, whole blocks as they come, then the tail with the
# padding, and prints the digest.
.type main, @function
main:
    sub rsp, 8
    movdqu xmm0, xmmword ptr [rip + initial_state]
    movdqu xmmword ptr [rip + state], xmm0
    movdqu xmm0, xmmword ptr [rip + initial_state + 16]
    movdqu xmmword ptr [rip + state + 16], xmm0
    xor r12d, r12d
    mov rbx, MAX_CHUNKS
.Lmain_read:  # bound: 268435456
    mov edi, STDIN
    lea rsi, [rip + input]
    mov edx, INPUT_LEN
    mov ecx, -1
    call fill
    test rax, rax
    js .Lmain_failed
    mov r13, rax
    cmp r13, INPUT_LEN
    ja .Lmain_assert_count
    add r12, r13
    lea rdi, [rip + state]
    lea rsi, [rip + input]
    mov rdx, r13
    call hash_blocks
    cmp r13, INPUT_LEN
    jb .Lmain_finish
    dec rbx
    jnz .Lmain_read
    jmp .Lmain_assert_reads
.Lmain_finish:
    mov r14, r13
    and r14, TAIL_MASK
    mov r15, r13
    sub r15, r14
    lea rdi, [rip + state]
    lea rsi, [rip + input]
    add rsi, r15
    mov rdx, r14
    mov rcx, r12
    call finish
    lea rdi, [rip + state]
    call print_digest
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
.Lmain_assert_reads:
    mov edi, 2
    call assert_fail
.size main, . - main

# Hashes every whole block of a piece of the message.
# In: rdi = the state; rsi = the bytes; rdx = how many, up to INPUT_LEN. A partial block at the
# end is left alone.
.type hash_blocks, @function
hash_blocks:
    push rbx
    push r12
    push r13
    push r14
    sub rsp, 8
    test rdi, rdi
    jz .Lhash_blocks_assert_state
    test rsi, rsi
    jz .Lhash_blocks_assert_bytes
    cmp rdx, INPUT_LEN
    ja .Lhash_blocks_assert_length
    mov rbx, rdi
    mov r12, rsi
    mov r13, rdx
    mov r14, BLOCK_STEPS
.Lhash_blocks_loop:  # bound: 65
    cmp r13, BLOCK_BYTES
    jb .Lhash_blocks_done
    mov rdi, r12
    call schedule_words
    mov rdi, rbx
    call rounds
    add r12, BLOCK_BYTES
    sub r13, BLOCK_BYTES
    dec r14
    jnz .Lhash_blocks_loop
    jmp .Lhash_blocks_assert_blocks
.Lhash_blocks_done:
    add rsp, 8
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Lhash_blocks_assert_state:
    mov edi, 3
    call assert_fail
.Lhash_blocks_assert_bytes:
    mov edi, 4
    call assert_fail
.Lhash_blocks_assert_length:
    mov edi, 5
    call assert_fail
.Lhash_blocks_assert_blocks:
    mov edi, 6
    call assert_fail
.size hash_blocks, . - hash_blocks

# Lays out one block's 64-word message schedule, with plain integer code: the block's 16 words,
# big-endian, then each of the other 48 from four before it:
# W[t] = sigma1(W[t-2]) + W[t-7] + sigma0(W[t-15]) + W[t-16].
# In: rdi = the block, 64 bytes.
.type schedule_words, @function
schedule_words:
    test rdi, rdi
    jz .Lschedule_words_assert_block
    lea rdx, [rip + schedule]
    xor ecx, ecx
    mov r8, LOAD_STEPS
.Lschedule_words_load:  # bound: 16
    cmp rcx, BLOCK_BYTES - 4 + 1
    jae .Lschedule_words_assert_load
    mov eax, dword ptr [rdi + rcx]
    bswap eax
    mov dword ptr [rdx + rcx], eax
    add rcx, 4
    dec r8
    jnz .Lschedule_words_load
    xor ecx, ecx
    mov r8, EXTEND_STEPS
.Lschedule_words_extend:  # bound: 48
    cmp rcx, SCHEDULE_LEN - 64 - 4 + 1
    jae .Lschedule_words_assert_index
    mov eax, dword ptr [rdx + rcx + 56]
    mov r9d, eax
    ror r9d, 17
    mov r10d, eax
    ror r10d, 19
    xor r9d, r10d
    shr eax, 10
    xor r9d, eax
    mov eax, dword ptr [rdx + rcx + 4]
    mov r10d, eax
    ror r10d, 7
    mov r11d, eax
    ror r11d, 18
    xor r10d, r11d
    shr eax, 3
    xor r10d, eax
    add r9d, r10d
    add r9d, dword ptr [rdx + rcx + 36]
    add r9d, dword ptr [rdx + rcx]
    mov dword ptr [rdx + rcx + 64], r9d
    add rcx, 4
    dec r8
    jnz .Lschedule_words_extend
    ret
.Lschedule_words_assert_block:
    mov edi, 7
    call assert_fail
.Lschedule_words_assert_index:
    mov edi, 8
    call assert_fail
.Lschedule_words_assert_load:
    mov edi, 21
    call assert_fail
.size schedule_words, . - schedule_words

# Runs the 64 rounds on the message schedule, one at a time, and adds the result to the state.
# The working variables of a round are the end of two lists in time order (work_a, work_e), not
# eight registers that each round moves: the state's words go in reversed, and the last four of
# each list come out, reversed again.
# In: rdi = the state, eight 32-bit words.
.type rounds, @function
rounds:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Lrounds_assert_state
    mov r12, rdi
    movdqu xmm0, xmmword ptr [r12]
    pshufd xmm0, xmm0, 0x1B
    movdqu xmmword ptr [rip + work_a], xmm0
    movdqu xmm0, xmmword ptr [r12 + 16]
    pshufd xmm0, xmm0, 0x1B
    movdqu xmmword ptr [rip + work_e], xmm0
    xor r13d, r13d
    mov rbx, ROUND_STEPS
.Lrounds_loop:  # bound: 64
    mov rdi, r13
    call sha_round
    add r13, 4
    dec rbx
    jnz .Lrounds_loop
    movdqu xmm0, xmmword ptr [rip + work_a + 256]
    pshufd xmm0, xmm0, 0x1B
    movdqu xmm1, xmmword ptr [r12]
    paddd xmm0, xmm1
    movdqu xmmword ptr [r12], xmm0
    movdqu xmm0, xmmword ptr [rip + work_e + 256]
    pshufd xmm0, xmm0, 0x1B
    movdqu xmm1, xmmword ptr [r12 + 16]
    paddd xmm0, xmm1
    movdqu xmmword ptr [r12 + 16], xmm0
    pop r13
    pop r12
    pop rbx
    ret
.Lrounds_assert_state:
    mov edi, 9
    call assert_fail
.size rounds, . - rounds

# One round of the compression function, the sum of its two halves:
# T1 = h + Sigma1(e) + Ch(e, f, g) + K[t] + W[t] and T2 = Sigma0(a) + Maj(a, b, c), then
# a = T1 + T2 and e = d + T1; the other six variables are the words before them in time order.
# In: rdi = 4 times the round's number, 0 to 252: its place in work_a and work_e (d at that
# place and a four words on, h and e likewise), in k_table and in schedule.
.type sha_round, @function
sha_round:
    lea rsi, [rip + work_a]
    lea rdx, [rip + work_e]
    lea r8, [rip + k_table]
    lea r9, [rip + schedule]
    cmp rdi, WORK_A_LEN - 20 + 1
    jae .Lsha_round_assert_index
    mov eax, dword ptr [rdx + rdi + 12]
    mov ecx, eax
    ror ecx, 6
    mov r10d, eax
    ror r10d, 11
    xor ecx, r10d
    mov r10d, eax
    ror r10d, 25
    xor ecx, r10d
    mov r10d, dword ptr [rdx + rdi + 8]
    mov r11d, dword ptr [rdx + rdi + 4]
    xor r10d, r11d
    and r10d, eax
    xor r10d, r11d
    add ecx, r10d
    add ecx, dword ptr [rdx + rdi]
    add ecx, dword ptr [r8 + rdi]
    add ecx, dword ptr [r9 + rdi]
    mov eax, dword ptr [rsi + rdi + 12]
    mov r10d, eax
    ror r10d, 2
    mov r11d, eax
    ror r11d, 13
    xor r10d, r11d
    mov r11d, eax
    ror r11d, 22
    xor r10d, r11d
    mov r8d, dword ptr [rsi + rdi + 8]
    mov r9d, dword ptr [rsi + rdi + 4]
    mov r11d, eax
    or r11d, r8d
    and r11d, r9d
    and eax, r8d
    or r11d, eax
    add r10d, r11d
    mov eax, dword ptr [rsi + rdi]
    add eax, ecx
    mov dword ptr [rdx + rdi + 16], eax
    add ecx, r10d
    mov dword ptr [rsi + rdi + 16], ecx
    ret
.Lsha_round_assert_index:
    mov edi, 22
    call assert_fail
.size sha_round, . - sha_round

# Pads the message's last bytes with a 1 bit, zeros and its length in bits, and hashes the one or
# two blocks that makes. It runs once, so padded still holds zeros.
# In: rdi = the state; rsi = the last bytes; rdx = how many, fewer than 64; rcx = the message's
# length in bytes.
.type finish, @function
finish:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Lfinish_assert_state
    test rsi, rsi
    jz .Lfinish_assert_tail
    cmp rdx, BLOCK_BYTES
    jae .Lfinish_assert_count
    mov rbx, rdi
    mov r12, rdx
    mov r13, rcx
    lea rdi, [rip + padded]
    xor eax, eax
    mov r8, MAX_COPY
.Lfinish_copy:  # bound: 63
    cmp rax, r12
    jae .Lfinish_copied
    cmp rax, PADDED_LEN
    jae .Lfinish_assert_copy
    movzx r9d, byte ptr [rsi + rax]
    mov byte ptr [rdi + rax], r9b
    inc rax
    dec r8
    jnz .Lfinish_copy
.Lfinish_copied:
    cmp rax, PADDED_LEN
    jae .Lfinish_assert_pad
    mov byte ptr [rdi + rax], PAD_BYTE
    mov r10d, LENGTH_AT_ONE
    cmp r12, LENGTH_AT_ONE
    jb .Lfinish_length
    mov r10d, LENGTH_AT_TWO
.Lfinish_length:
    shl r13, 3
    bswap r13
    cmp r10, PADDED_LEN - 8 + 1
    jae .Lfinish_assert_length
    mov qword ptr [rdi + r10], r13
    mov rdi, rbx
    lea rsi, [rip + padded]
    lea rdx, [r10 + 8]
    call hash_blocks
    pop r13
    pop r12
    pop rbx
    ret
.Lfinish_assert_state:
    mov edi, 11
    call assert_fail
.Lfinish_assert_tail:
    mov edi, 12
    call assert_fail
.Lfinish_assert_count:
    mov edi, 13
    call assert_fail
.Lfinish_assert_copy:
    mov edi, 14
    call assert_fail
.Lfinish_assert_pad:
    mov edi, 15
    call assert_fail
.Lfinish_assert_length:
    mov edi, 16
    call assert_fail
.size finish, . - finish

# Prints the state's eight words as 64 hexadecimal digits and a newline.
# In: rdi = the state.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_digest, @function
print_digest:
    sub rsp, 8
    test rdi, rdi
    jz .Lprint_digest_assert_state
    lea r8, [rip + hex_digits]
    lea rsi, [rip + out]
    xor ecx, ecx
    xor r9d, r9d
    mov r10, STATE_WORDS
.Lprint_digest_word:  # bound: 8
    cmp r9, STATE_WORDS
    jae .Lprint_digest_assert_word
    mov edx, dword ptr [rdi + r9*4]
    mov r11, NIBBLES
.Lprint_digest_nibble:  # bound: 8
    rol edx, 4
    mov eax, edx
    and eax, 15
    cmp eax, 16
    jae .Lprint_digest_assert_nibble
    movzx eax, byte ptr [r8 + rax]
    cmp rcx, HEX_DIGITS_COUNT
    jae .Lprint_digest_assert_out
    mov byte ptr [rsi + rcx], al
    inc rcx
    dec r11
    jnz .Lprint_digest_nibble
    inc r9
    dec r10
    jnz .Lprint_digest_word
    mov byte ptr [rsi + HEX_DIGITS_COUNT], NEWLINE
    mov edi, STDOUT
    mov edx, OUT_LEN
    call write_all
    test rax, rax
    js .Lprint_digest_return
    xor eax, eax
.Lprint_digest_return:
    add rsp, 8
    ret
.Lprint_digest_assert_state:
    mov edi, 17
    call assert_fail
.Lprint_digest_assert_word:
    mov edi, 18
    call assert_fail
.Lprint_digest_assert_nibble:
    mov edi, 19
    call assert_fail
.Lprint_digest_assert_out:
    mov edi, 20
    call assert_fail
.size print_digest, . - print_digest

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

