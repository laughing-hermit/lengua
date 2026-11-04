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
.equ EXIT_IO_FAILED, 2
.equ EXIT_BAD_INPUT, 3
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
# fill's constant
.equ MAX_FILL_READS, 1099511627776
# Sizes and limits: a key line of 32 digits, a newline, 32 to 131072 digits and a newline
.equ KEY_HEX, 32
.equ DATA_AT, 33
.equ MIN_INPUT, 65
.equ HEX_MAX, 131072
.equ DATA_BYTES, 65536
.equ DATA_STEPS, 65537
.equ MAX_BLOCKS, 4096
.equ BLOCK_STEPS, 4097
.equ INPUT_LEN, 131107
.equ KEY_LEN, 16
.equ PLAIN_LEN, 65536
.equ CIPHER_LEN, 65536
.equ ROUND_KEYS_LEN, 176
.equ RCON_TABLE_LEN, 160
.equ OUT_LEN, 131073
.equ EXPAND_STEPS, 10
.equ ROUNDS_MIDDLE, 9
.equ BLOCK_MASK, 31
.equ NEWLINE, 10
.equ BAD_MSG_LEN, 15
.equ MAX_WRITES, 64

.section .rodata

.type bad_msg, @object
bad_msg:
    .ascii "aes: bad input\n"
.size bad_msg, . - bad_msg

# Moves the last word of a round key, rotated by a byte, into all four words: the first step of
# the key schedule.
.type rotate_mask, @object
rotate_mask:
    .byte 13, 14, 15, 12, 13, 14, 15, 12, 13, 14, 15, 12, 13, 14, 15, 12
.size rotate_mask, . - rotate_mask

# The round constants, one in each word.
.type rcon_table, @object
rcon_table:
    .long 0x01, 0x01, 0x01, 0x01
    .long 0x02, 0x02, 0x02, 0x02
    .long 0x04, 0x04, 0x04, 0x04
    .long 0x08, 0x08, 0x08, 0x08
    .long 0x10, 0x10, 0x10, 0x10
    .long 0x20, 0x20, 0x20, 0x20
    .long 0x40, 0x40, 0x40, 0x40
    .long 0x80, 0x80, 0x80, 0x80
    .long 0x1b, 0x1b, 0x1b, 0x1b
    .long 0x36, 0x36, 0x36, 0x36
.size rcon_table, . - rcon_table

.type hex_digits, @object
hex_digits:
    .ascii "0123456789abcdef"
.size hex_digits, . - hex_digits

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# All of standard input, plus one byte to notice input that is too long.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

# The key's 16 bytes.
# shared: _start passes it to expand_key; parse_input decodes the key line into it
.type key, @object
key:
    .zero KEY_LEN
.size key, . - key

# The data's bytes, up to 4096 blocks.
# shared: parse_input decodes the data into it; _start passes it to encrypt_blocks
.type plain, @object
plain:
    .zero PLAIN_LEN
.size plain, . - plain

# The same blocks encrypted.
.type cipher, @object
cipher:
    .zero CIPHER_LEN
.size cipher, . - cipher

# The 11 round keys of 16 bytes.
.type round_keys, @object
round_keys:
    .zero ROUND_KEYS_LEN
.size round_keys, . - round_keys

# The ciphertext in hexadecimal and a newline.
# shared: print_hex fills it with digits and write_line adds the newline and writes it
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
.globl _start

# Reads the key and the data, encrypts every block and prints the ciphertext.
.type _start, @function
_start:
    mov edi, STDIN
    lea rsi, [rip + input]
    mov edx, INPUT_LEN
    mov ecx, -1
    call fill
    test rax, rax
    js .Lstart_failed
    mov rbx, rax
    cmp rbx, INPUT_LEN
    jae .Lstart_bad
    lea rdi, [rip + input]
    mov rsi, rbx
    call parse_input
    test rax, rax
    js .Lstart_bad
    mov r12, rax
    lea rdi, [rip + key]
    lea rsi, [rip + round_keys]
    call expand_key
    lea rdi, [rip + round_keys]
    lea rsi, [rip + plain]
    lea rdx, [rip + cipher]
    mov rcx, r12
    call encrypt_blocks
    lea rdi, [rip + cipher]
    mov rsi, r12
    shl rsi, 4
    call print_hex
    test rax, rax
    js .Lstart_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_bad:
    mov edi, STDERR
    lea rsi, [rip + bad_msg]
    mov edx, BAD_MSG_LEN
    call write_all
    test rax, rax
    js .Lstart_failed
    mov edi, EXIT_BAD_INPUT
    jmp .Lstart_exit
.Lstart_failed:
    mov edi, EXIT_IO_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.size _start, . - _start

# Checks the input's layout and decodes the key and the data.
# In: rdi = the input; rsi = its length, less than INPUT_LEN.
# Out: rax = how many blocks the data holds, or ERROR_INVALID.
# returns: result
.type parse_input, @function
parse_input:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Lparse_input_assert_null
    cmp rsi, INPUT_LEN
    jae .Lparse_input_assert_length
    mov rbx, rdi
    mov r12, rsi
    mov rax, ERROR_INVALID
    cmp r12, MIN_INPUT
    jb .Lparse_input_return
    movzx ecx, byte ptr [rbx + KEY_HEX]
    cmp ecx, NEWLINE
    jne .Lparse_input_return
    mov r13, r12
    cmp r12, INPUT_LEN
    jae .Lparse_input_assert_end
    movzx ecx, byte ptr [rbx + r12 - 1]
    cmp ecx, NEWLINE
    jne .Lparse_input_digits
    dec r13
.Lparse_input_digits:
    sub r13, DATA_AT
    test r13, BLOCK_MASK
    jnz .Lparse_input_return
    cmp r13, HEX_MAX
    ja .Lparse_input_return
    lea rdi, [rip + key]
    mov rsi, rbx
    mov edx, KEY_HEX
    call decode_hex
    test rax, rax
    js .Lparse_input_return
    lea rdi, [rip + plain]
    lea rsi, [rbx + DATA_AT]
    mov rdx, r13
    call decode_hex
    test rax, rax
    js .Lparse_input_return
    mov rax, r13
    shr rax, 5
.Lparse_input_return:
    pop r13
    pop r12
    pop rbx
    ret
.Lparse_input_assert_null:
    mov edi, 1
    call assert_fail
.Lparse_input_assert_length:
    mov edi, 2
    call assert_fail
.Lparse_input_assert_end:
    mov edi, 28
    call assert_fail
.size parse_input, . - parse_input

# Turns hexadecimal digits into bytes.
# In: rdi = where the bytes go; rsi = the digits; rdx = how many, an even number up to HEX_MAX.
# Out: rax = 0, or ERROR_INVALID when a character isn't a hexadecimal digit.
# returns: result
.type decode_hex, @function
decode_hex:
    push rbx
    push r12
    push r13
    push r14
    push r15
    test rdi, rdi
    jz .Ldecode_hex_assert_dest
    test rsi, rsi
    jz .Ldecode_hex_assert_digits
    cmp rdx, HEX_MAX
    ja .Ldecode_hex_assert_count
    mov r12, rdi
    mov r13, rsi
    mov r14, rdx
    xor r15d, r15d
    mov rbx, DATA_STEPS
.Ldecode_hex_loop:  # bound: 65537
    lea rax, [r15 + r15]
    cmp rax, r14
    jae .Ldecode_hex_done
    cmp rax, HEX_MAX - 1
    jae .Ldecode_hex_assert_index
    movzx edi, byte ptr [r13 + rax]
    movzx esi, byte ptr [r13 + rax + 1]
    call hex_pair
    test rax, rax
    js .Ldecode_hex_return
    cmp r15, DATA_BYTES
    jae .Ldecode_hex_assert_dest_index
    mov byte ptr [r12 + r15], al
    inc r15
    dec rbx
    jnz .Ldecode_hex_loop
    jmp .Ldecode_hex_assert_steps
.Ldecode_hex_done:
    xor eax, eax
.Ldecode_hex_return:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Ldecode_hex_assert_dest:
    mov edi, 3
    call assert_fail
.Ldecode_hex_assert_digits:
    mov edi, 4
    call assert_fail
.Ldecode_hex_assert_count:
    mov edi, 5
    call assert_fail
.Ldecode_hex_assert_index:
    mov edi, 6
    call assert_fail
.Ldecode_hex_assert_dest_index:
    mov edi, 7
    call assert_fail
.Ldecode_hex_assert_steps:
    mov edi, 8
    call assert_fail
.size decode_hex, . - decode_hex

# Turns two hexadecimal digits into a byte.
# In: edi = the first digit's character; esi = the second's.
# Out: rax = the byte, 0 to 255, or ERROR_INVALID.
# returns: result
.type hex_pair, @function
hex_pair:
    push rbx
    push r12
    sub rsp, 8
    mov r12d, esi
    call hex_value
    test rax, rax
    js .Lhex_pair_return
    mov ebx, eax
    shl ebx, 4
    mov edi, r12d
    call hex_value
    test rax, rax
    js .Lhex_pair_return
    or eax, ebx
.Lhex_pair_return:
    add rsp, 8
    pop r12
    pop rbx
    ret
.size hex_pair, . - hex_pair

# The value of a hexadecimal digit: 0 to 9, a to f or A to F.
# In: edi = the character.
# Out: rax = 0 to 15, or ERROR_INVALID.
# returns: result
.type hex_value, @function
hex_value:
    mov eax, edi
    sub eax, '0'
    cmp eax, 9
    jbe .Lhex_value_return
    mov eax, edi
    sub eax, 'a'
    cmp eax, 5
    jbe .Lhex_value_letter
    mov eax, edi
    sub eax, 'A'
    cmp eax, 5
    ja .Lhex_value_invalid
.Lhex_value_letter:
    add eax, 10
    jmp .Lhex_value_return
.Lhex_value_invalid:
    mov rax, ERROR_INVALID
.Lhex_value_return:
    ret
.size hex_value, . - hex_value

# Expands a key into the 11 round keys, with the CPU's AES instructions.
# In: rdi = the key, 16 bytes; rsi = where the round keys go, ROUND_KEYS_LEN bytes.
.type expand_key, @function
expand_key:
    test rdi, rdi
    jz .Lexpand_key_assert_key
    test rsi, rsi
    jz .Lexpand_key_assert_keys
    movdqu xmm1, xmmword ptr [rdi]
    movdqu xmmword ptr [rsi], xmm1
    movdqu xmm6, xmmword ptr [rip + rotate_mask]
    lea r9, [rip + rcon_table]
    xor ecx, ecx
    mov r8, EXPAND_STEPS
.Lexpand_key_loop:  # bound: 10
    cmp rcx, RCON_TABLE_LEN - 16 + 1
    jae .Lexpand_key_assert_index
    movdqu xmm5, xmmword ptr [r9 + rcx]
    movdqu xmm2, xmm1
    pshufb xmm2, xmm6
    aesenclast xmm2, xmm5
    movdqu xmm3, xmm1
    pslldq xmm3, 4
    pxor xmm1, xmm3
    pslldq xmm3, 4
    pxor xmm1, xmm3
    pslldq xmm3, 4
    pxor xmm1, xmm3
    pxor xmm1, xmm2
    movdqu xmmword ptr [rsi + rcx + 16], xmm1
    add rcx, 16
    dec r8
    jnz .Lexpand_key_loop
    ret
.Lexpand_key_assert_key:
    mov edi, 9
    call assert_fail
.Lexpand_key_assert_keys:
    mov edi, 10
    call assert_fail
.Lexpand_key_assert_index:
    mov edi, 11
    call assert_fail
.size expand_key, . - expand_key

# Encrypts blocks of 16 bytes one at a time: the first round key, nine rounds and the last.
# In: rdi = the round keys; rsi = the blocks; rdx = where the encrypted blocks go; rcx = how
# many blocks, up to MAX_BLOCKS.
.type encrypt_blocks, @function
encrypt_blocks:
    test rdi, rdi
    jz .Lencrypt_blocks_assert_keys
    test rsi, rsi
    jz .Lencrypt_blocks_assert_blocks
    test rdx, rdx
    jz .Lencrypt_blocks_assert_out
    cmp rcx, MAX_BLOCKS
    ja .Lencrypt_blocks_assert_count
    movdqu xmm1, xmmword ptr [rdi]
    movdqu xmm7, xmmword ptr [rdi + 160]
    mov r9, rcx
    shl r9, 4
    xor r10d, r10d
    mov r11, BLOCK_STEPS
.Lencrypt_blocks_block:  # bound: 4097
    cmp r10, r9
    jae .Lencrypt_blocks_done
    cmp r10, PLAIN_LEN - 16 + 1
    jae .Lencrypt_blocks_assert_index
    movdqu xmm0, xmmword ptr [rsi + r10]
    pxor xmm0, xmm1
    mov eax, 16
    mov r8, ROUNDS_MIDDLE
.Lencrypt_blocks_round:  # bound: 9
    cmp rax, ROUND_KEYS_LEN - 16 + 1
    jae .Lencrypt_blocks_assert_round
    movdqu xmm2, xmmword ptr [rdi + rax]
    aesenc xmm0, xmm2
    add rax, 16
    dec r8
    jnz .Lencrypt_blocks_round
    aesenclast xmm0, xmm7
    cmp r10, CIPHER_LEN - 16 + 1
    jae .Lencrypt_blocks_assert_out_index
    movdqu xmmword ptr [rdx + r10], xmm0
    add r10, 16
    dec r11
    jnz .Lencrypt_blocks_block
    jmp .Lencrypt_blocks_assert_steps
.Lencrypt_blocks_done:
    ret
.Lencrypt_blocks_assert_keys:
    mov edi, 12
    call assert_fail
.Lencrypt_blocks_assert_blocks:
    mov edi, 13
    call assert_fail
.Lencrypt_blocks_assert_out:
    mov edi, 14
    call assert_fail
.Lencrypt_blocks_assert_count:
    mov edi, 15
    call assert_fail
.Lencrypt_blocks_assert_index:
    mov edi, 16
    call assert_fail
.Lencrypt_blocks_assert_round:
    mov edi, 17
    call assert_fail
.Lencrypt_blocks_assert_out_index:
    mov edi, 18
    call assert_fail
.Lencrypt_blocks_assert_steps:
    mov edi, 19
    call assert_fail
.size encrypt_blocks, . - encrypt_blocks

# Prints bytes as lowercase hexadecimal digits and a newline.
# In: rdi = the bytes; rsi = how many, up to DATA_BYTES.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_hex, @function
print_hex:
    sub rsp, 8
    test rdi, rdi
    jz .Lprint_hex_assert_bytes
    cmp rsi, DATA_BYTES
    ja .Lprint_hex_assert_count
    lea r8, [rip + hex_digits]
    lea r9, [rip + out]
    xor ecx, ecx
    mov r11, DATA_STEPS
.Lprint_hex_loop:  # bound: 65537
    cmp rcx, rsi
    jae .Lprint_hex_done
    cmp rcx, DATA_BYTES
    jae .Lprint_hex_assert_index
    movzx eax, byte ptr [rdi + rcx]
    mov edx, eax
    shr eax, 4
    and edx, 15
    cmp eax, 16
    jae .Lprint_hex_assert_high
    movzx eax, byte ptr [r8 + rax]
    lea r10, [rcx + rcx]
    cmp r10, HEX_MAX - 1
    jae .Lprint_hex_assert_out
    mov byte ptr [r9 + r10], al
    cmp edx, 16
    jae .Lprint_hex_assert_low
    movzx eax, byte ptr [r8 + rdx]
    mov byte ptr [r9 + r10 + 1], al
    inc rcx
    dec r11
    jnz .Lprint_hex_loop
    jmp .Lprint_hex_assert_steps
.Lprint_hex_done:
    lea rdi, [rsi + rsi]
    call write_line
    test rax, rax
    js .Lprint_hex_return
.Lprint_hex_return:
    add rsp, 8
    ret
.Lprint_hex_assert_bytes:
    mov edi, 20
    call assert_fail
.Lprint_hex_assert_count:
    mov edi, 21
    call assert_fail
.Lprint_hex_assert_index:
    mov edi, 22
    call assert_fail
.Lprint_hex_assert_high:
    mov edi, 23
    call assert_fail
.Lprint_hex_assert_out:
    mov edi, 24
    call assert_fail
.Lprint_hex_assert_low:
    mov edi, 25
    call assert_fail
.Lprint_hex_assert_steps:
    mov edi, 26
    call assert_fail
.size print_hex, . - print_hex

# Writes the hex digits in out, a newline after them, to standard output.
# In: rdi = how many digits, 1 to HEX_MAX.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_line, @function
write_line:
    sub rsp, 8
    test rdi, rdi
    jz .Lwrite_line_assert_empty
    cmp rdi, HEX_MAX
    ja .Lwrite_line_assert_end
    lea r9, [rip + out]
    mov byte ptr [r9 + rdi], NEWLINE
    lea rdx, [rdi + 1]
    mov rsi, r9
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lwrite_line_return
    xor eax, eax
.Lwrite_line_return:
    add rsp, 8
    ret
.Lwrite_line_assert_empty:
    mov edi, 34
    call assert_fail
.Lwrite_line_assert_end:
    mov edi, 27
    call assert_fail
.size write_line, . - write_line

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
