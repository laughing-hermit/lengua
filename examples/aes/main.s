.intel_syntax noprefix

# AES-128 in plain integer code and a table of 256 words, so it runs on every x86-64 computer. The
# form that uses the CPU's AES instructions, from the very same spec, is
# plugin/skills/compile/assets/fast/aes-ni.

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
.equ CURRENT_LEN, 32
.equ TABLE_ENTRIES, 256
.equ BLOCK_MASK, 31
.equ NEWLINE, 10
.equ BAD_MSG_LEN, 15
.equ MAX_WRITES, 64

.section .rodata

.type bad_msg, @object
bad_msg:
    .ascii "aes: bad input\n"
.size bad_msg, . - bad_msg

# SubBytes and MixColumns in one table, for the byte in row 0 of a column. S is the S-box; each
# word holds, from its lowest byte, 2 * S[x], S[x], S[x] and 3 * S[x] (products in GF(2^8)), so
# the bytes in rows 1, 2 and 3 use the same word rotated left by 8, 16 and 24 bits, and S[x]
# itself is the word's byte 1.
.type te0, @object
te0:
    .long 0xa56363c6, 0x847c7cf8, 0x997777ee, 0x8d7b7bf6
    .long 0x0df2f2ff, 0xbd6b6bd6, 0xb16f6fde, 0x54c5c591
    .long 0x50303060, 0x03010102, 0xa96767ce, 0x7d2b2b56
    .long 0x19fefee7, 0x62d7d7b5, 0xe6abab4d, 0x9a7676ec
    .long 0x45caca8f, 0x9d82821f, 0x40c9c989, 0x877d7dfa
    .long 0x15fafaef, 0xeb5959b2, 0xc947478e, 0x0bf0f0fb
    .long 0xecadad41, 0x67d4d4b3, 0xfda2a25f, 0xeaafaf45
    .long 0xbf9c9c23, 0xf7a4a453, 0x967272e4, 0x5bc0c09b
    .long 0xc2b7b775, 0x1cfdfde1, 0xae93933d, 0x6a26264c
    .long 0x5a36366c, 0x413f3f7e, 0x02f7f7f5, 0x4fcccc83
    .long 0x5c343468, 0xf4a5a551, 0x34e5e5d1, 0x08f1f1f9
    .long 0x937171e2, 0x73d8d8ab, 0x53313162, 0x3f15152a
    .long 0x0c040408, 0x52c7c795, 0x65232346, 0x5ec3c39d
    .long 0x28181830, 0xa1969637, 0x0f05050a, 0xb59a9a2f
    .long 0x0907070e, 0x36121224, 0x9b80801b, 0x3de2e2df
    .long 0x26ebebcd, 0x6927274e, 0xcdb2b27f, 0x9f7575ea
    .long 0x1b090912, 0x9e83831d, 0x742c2c58, 0x2e1a1a34
    .long 0x2d1b1b36, 0xb26e6edc, 0xee5a5ab4, 0xfba0a05b
    .long 0xf65252a4, 0x4d3b3b76, 0x61d6d6b7, 0xceb3b37d
    .long 0x7b292952, 0x3ee3e3dd, 0x712f2f5e, 0x97848413
    .long 0xf55353a6, 0x68d1d1b9, 0x00000000, 0x2cededc1
    .long 0x60202040, 0x1ffcfce3, 0xc8b1b179, 0xed5b5bb6
    .long 0xbe6a6ad4, 0x46cbcb8d, 0xd9bebe67, 0x4b393972
    .long 0xde4a4a94, 0xd44c4c98, 0xe85858b0, 0x4acfcf85
    .long 0x6bd0d0bb, 0x2aefefc5, 0xe5aaaa4f, 0x16fbfbed
    .long 0xc5434386, 0xd74d4d9a, 0x55333366, 0x94858511
    .long 0xcf45458a, 0x10f9f9e9, 0x06020204, 0x817f7ffe
    .long 0xf05050a0, 0x443c3c78, 0xba9f9f25, 0xe3a8a84b
    .long 0xf35151a2, 0xfea3a35d, 0xc0404080, 0x8a8f8f05
    .long 0xad92923f, 0xbc9d9d21, 0x48383870, 0x04f5f5f1
    .long 0xdfbcbc63, 0xc1b6b677, 0x75dadaaf, 0x63212142
    .long 0x30101020, 0x1affffe5, 0x0ef3f3fd, 0x6dd2d2bf
    .long 0x4ccdcd81, 0x140c0c18, 0x35131326, 0x2fececc3
    .long 0xe15f5fbe, 0xa2979735, 0xcc444488, 0x3917172e
    .long 0x57c4c493, 0xf2a7a755, 0x827e7efc, 0x473d3d7a
    .long 0xac6464c8, 0xe75d5dba, 0x2b191932, 0x957373e6
    .long 0xa06060c0, 0x98818119, 0xd14f4f9e, 0x7fdcdca3
    .long 0x66222244, 0x7e2a2a54, 0xab90903b, 0x8388880b
    .long 0xca46468c, 0x29eeeec7, 0xd3b8b86b, 0x3c141428
    .long 0x79dedea7, 0xe25e5ebc, 0x1d0b0b16, 0x76dbdbad
    .long 0x3be0e0db, 0x56323264, 0x4e3a3a74, 0x1e0a0a14
    .long 0xdb494992, 0x0a06060c, 0x6c242448, 0xe45c5cb8
    .long 0x5dc2c29f, 0x6ed3d3bd, 0xefacac43, 0xa66262c4
    .long 0xa8919139, 0xa4959531, 0x37e4e4d3, 0x8b7979f2
    .long 0x32e7e7d5, 0x43c8c88b, 0x5937376e, 0xb76d6dda
    .long 0x8c8d8d01, 0x64d5d5b1, 0xd24e4e9c, 0xe0a9a949
    .long 0xb46c6cd8, 0xfa5656ac, 0x07f4f4f3, 0x25eaeacf
    .long 0xaf6565ca, 0x8e7a7af4, 0xe9aeae47, 0x18080810
    .long 0xd5baba6f, 0x887878f0, 0x6f25254a, 0x722e2e5c
    .long 0x241c1c38, 0xf1a6a657, 0xc7b4b473, 0x51c6c697
    .long 0x23e8e8cb, 0x7cdddda1, 0x9c7474e8, 0x211f1f3e
    .long 0xdd4b4b96, 0xdcbdbd61, 0x868b8b0d, 0x858a8a0f
    .long 0x907070e0, 0x423e3e7c, 0xc4b5b571, 0xaa6666cc
    .long 0xd8484890, 0x05030306, 0x01f6f6f7, 0x120e0e1c
    .long 0xa36161c2, 0x5f35356a, 0xf95757ae, 0xd0b9b969
    .long 0x91868617, 0x58c1c199, 0x271d1d3a, 0xb99e9e27
    .long 0x38e1e1d9, 0x13f8f8eb, 0xb398982b, 0x33111122
    .long 0xbb6969d2, 0x70d9d9a9, 0x898e8e07, 0xa7949433
    .long 0xb69b9b2d, 0x221e1e3c, 0x92878715, 0x20e9e9c9
    .long 0x49cece87, 0xff5555aa, 0x78282850, 0x7adfdfa5
    .long 0x8f8c8c03, 0xf8a1a159, 0x80898909, 0x170d0d1a
    .long 0xdabfbf65, 0x31e6e6d7, 0xc6424284, 0xb86868d0
    .long 0xc3414182, 0xb0999929, 0x772d2d5a, 0x110f0f1e
    .long 0xcbb0b07b, 0xfc5454a8, 0xd6bbbb6d, 0x3a16162c
.size te0, . - te0

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

# The block being encrypted: its four words twice over, so that the four words from any word on
# (the ones ShiftRows brings into a column) are next to each other.
# shared: encrypt_block sets it, aes_round replaces it, and column and last_column read it
.type current, @object
current:
    .zero CURRENT_LEN
.size current, . - current

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

# Expands a key into the 11 round keys, a word at a time: each round key's first word is the
# previous round key's first word xor SubWord(RotWord(its last word)) xor the round constant, and
# each other word the word before it xor the one four words back.
# In: rdi = the key, 16 bytes; rsi = where the round keys go, ROUND_KEYS_LEN bytes.
.type expand_key, @function
expand_key:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Lexpand_key_assert_key
    test rsi, rsi
    jz .Lexpand_key_assert_keys
    movdqu xmm0, xmmword ptr [rdi]
    movdqu xmmword ptr [rsi], xmm0
    mov r12, rsi
    xor r13d, r13d
    mov rbx, EXPAND_STEPS
.Lexpand_key_loop:  # bound: 10
    cmp r13, ROUND_KEYS_LEN - 32 + 1
    jae .Lexpand_key_assert_index
    mov edi, dword ptr [r12 + r13 + 12]
    ror edi, 8
    call sub_word
    lea rdx, [rip + rcon_table]
    xor eax, dword ptr [rdx + r13]
    xor eax, dword ptr [r12 + r13]
    mov dword ptr [r12 + r13 + 16], eax
    xor eax, dword ptr [r12 + r13 + 4]
    mov dword ptr [r12 + r13 + 20], eax
    xor eax, dword ptr [r12 + r13 + 8]
    mov dword ptr [r12 + r13 + 24], eax
    xor eax, dword ptr [r12 + r13 + 12]
    mov dword ptr [r12 + r13 + 28], eax
    add r13, 16
    dec rbx
    jnz .Lexpand_key_loop
    pop r13
    pop r12
    pop rbx
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

# SubWord: the S-box applied to each byte of a word, which is byte 1 of the table's words.
# In: edi = the word.
# Out: eax = the word with each byte replaced.
.type sub_word, @function
sub_word:
    lea r9, [rip + te0]
    movzx edx, dil
    cmp rdx, TABLE_ENTRIES
    jae .Lsub_word_assert_row0
    movzx eax, byte ptr [r9 + rdx*4 + 1]
    mov edx, edi
    shr edx, 8
    movzx edx, dl
    cmp rdx, TABLE_ENTRIES
    jae .Lsub_word_assert_row1
    movzx ecx, byte ptr [r9 + rdx*4 + 1]
    shl ecx, 8
    or eax, ecx
    mov edx, edi
    shr edx, 16
    movzx edx, dl
    cmp rdx, TABLE_ENTRIES
    jae .Lsub_word_assert_row2
    movzx ecx, byte ptr [r9 + rdx*4 + 1]
    shl ecx, 16
    or eax, ecx
    mov edx, edi
    shr edx, 24
    cmp rdx, TABLE_ENTRIES
    jae .Lsub_word_assert_row3
    movzx ecx, byte ptr [r9 + rdx*4 + 1]
    shl ecx, 24
    or eax, ecx
    ret
.Lsub_word_assert_row0:
    mov edi, 35
    call assert_fail
.Lsub_word_assert_row1:
    mov edi, 36
    call assert_fail
.Lsub_word_assert_row2:
    mov edi, 37
    call assert_fail
.Lsub_word_assert_row3:
    mov edi, 38
    call assert_fail
.size sub_word, . - sub_word

# Encrypts blocks of 16 bytes one at a time.
# In: rdi = the round keys; rsi = the blocks; rdx = where the encrypted blocks go; rcx = how
# many blocks, up to MAX_BLOCKS.
.type encrypt_blocks, @function
encrypt_blocks:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8
    test rdi, rdi
    jz .Lencrypt_blocks_assert_keys
    test rsi, rsi
    jz .Lencrypt_blocks_assert_blocks
    test rdx, rdx
    jz .Lencrypt_blocks_assert_out
    cmp rcx, MAX_BLOCKS
    ja .Lencrypt_blocks_assert_count
    mov r12, rdi
    mov r13, rsi
    mov r14, rdx
    mov r15, rcx
    shl r15, 4
    xor ebx, ebx
    mov rbp, BLOCK_STEPS
.Lencrypt_blocks_block:  # bound: 4097
    cmp rbx, r15
    jae .Lencrypt_blocks_done
    cmp rbx, PLAIN_LEN - 16 + 1
    jae .Lencrypt_blocks_assert_index
    mov rdi, r12
    lea rsi, [r13 + rbx]
    lea rdx, [r14 + rbx]
    call encrypt_block
    add rbx, 16
    dec rbp
    jnz .Lencrypt_blocks_block
    jmp .Lencrypt_blocks_assert_steps
.Lencrypt_blocks_done:
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
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
.Lencrypt_blocks_assert_steps:
    mov edi, 19
    call assert_fail
.size encrypt_blocks, . - encrypt_blocks

# Encrypts one block: the first round key added, nine rounds and the last, which has no
# MixColumns.
# In: rdi = the round keys; rsi = the block, 16 bytes; rdx = where the encrypted block goes.
.type encrypt_block, @function
encrypt_block:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Lencrypt_block_assert_keys
    test rsi, rsi
    jz .Lencrypt_block_assert_block
    test rdx, rdx
    jz .Lencrypt_block_assert_out
    mov rbx, rdi
    mov r12, rdx
    movdqu xmm0, xmmword ptr [rsi]
    movdqu xmm1, xmmword ptr [rbx]
    pxor xmm0, xmm1
    movdqu xmmword ptr [rip + current], xmm0
    movdqu xmmword ptr [rip + current + 16], xmm0
    mov r13, ROUNDS_MIDDLE
.Lencrypt_block_round:  # bound: 9
    add rbx, 16
    mov rdi, rbx
    call aes_round
    dec r13
    jnz .Lencrypt_block_round
    add rbx, 16
    mov rdi, rbx
    mov rsi, r12
    call last_round
    pop r13
    pop r12
    pop rbx
    ret
.Lencrypt_block_assert_keys:
    mov edi, 17
    call assert_fail
.Lencrypt_block_assert_block:
    mov edi, 18
    call assert_fail
.Lencrypt_block_assert_out:
    mov edi, 39
    call assert_fail
.size encrypt_block, . - encrypt_block

# One round that isn't the last: the four columns of the new block, each from the old, then both
# copies of the block replaced.
# In: rdi = the round key, 16 bytes.
.type aes_round, @function
aes_round:
    push rbx
    push r12
    push r13
    push r14
    push r15
    test rdi, rdi
    jz .Laes_round_assert_key
    mov r15, rdi
    xor edi, edi
    mov esi, dword ptr [r15]
    call column
    mov ebx, eax
    mov edi, 4
    mov esi, dword ptr [r15 + 4]
    call column
    mov r12d, eax
    mov edi, 8
    mov esi, dword ptr [r15 + 8]
    call column
    mov r13d, eax
    mov edi, 12
    mov esi, dword ptr [r15 + 12]
    call column
    mov r14d, eax
    mov dword ptr [rip + current], ebx
    mov dword ptr [rip + current + 4], r12d
    mov dword ptr [rip + current + 8], r13d
    mov dword ptr [rip + current + 12], r14d
    mov dword ptr [rip + current + 16], ebx
    mov dword ptr [rip + current + 20], r12d
    mov dword ptr [rip + current + 24], r13d
    mov dword ptr [rip + current + 28], r14d
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Laes_round_assert_key:
    mov edi, 40
    call assert_fail
.size aes_round, . - aes_round

# One column of a round that isn't the last, with SubBytes, ShiftRows, MixColumns and the round
# key's word in one: the byte of row r comes from the word r columns on, goes through the table
# and is rotated left by 8 * r bits.
# In: edi = 4 times the column, 0 to 12; esi = the round key's word for that column.
# Out: eax = the column's new word.
.type column, @function
column:
    lea r8, [rip + current]
    lea r9, [rip + te0]
    cmp rdi, CURRENT_LEN - 16 + 1
    jae .Lcolumn_assert_index
    mov eax, dword ptr [r8 + rdi]
    movzx edx, al
    cmp rdx, TABLE_ENTRIES
    jae .Lcolumn_assert_row0
    mov ecx, dword ptr [r9 + rdx*4]
    mov eax, dword ptr [r8 + rdi + 4]
    shr eax, 8
    movzx edx, al
    cmp rdx, TABLE_ENTRIES
    jae .Lcolumn_assert_row1
    mov edx, dword ptr [r9 + rdx*4]
    rol edx, 8
    xor ecx, edx
    mov eax, dword ptr [r8 + rdi + 8]
    shr eax, 16
    movzx edx, al
    cmp rdx, TABLE_ENTRIES
    jae .Lcolumn_assert_row2
    mov edx, dword ptr [r9 + rdx*4]
    rol edx, 16
    xor ecx, edx
    mov eax, dword ptr [r8 + rdi + 12]
    shr eax, 24
    cmp rax, TABLE_ENTRIES
    jae .Lcolumn_assert_row3
    mov edx, dword ptr [r9 + rax*4]
    rol edx, 24
    xor ecx, edx
    xor ecx, esi
    mov eax, ecx
    ret
.Lcolumn_assert_index:
    mov edi, 41
    call assert_fail
.Lcolumn_assert_row0:
    mov edi, 42
    call assert_fail
.Lcolumn_assert_row1:
    mov edi, 43
    call assert_fail
.Lcolumn_assert_row2:
    mov edi, 44
    call assert_fail
.Lcolumn_assert_row3:
    mov edi, 45
    call assert_fail
.size column, . - column

# The last round: SubBytes, ShiftRows and the last round key, the four columns written to the
# encrypted block.
# In: rdi = the last round key, 16 bytes; rsi = where the encrypted block goes.
.type last_round, @function
last_round:
    push rbx
    push r12
    sub rsp, 8
    test rdi, rdi
    jz .Llast_round_assert_key
    test rsi, rsi
    jz .Llast_round_assert_out
    mov rbx, rdi
    mov r12, rsi
    xor edi, edi
    mov esi, dword ptr [rbx]
    call last_column
    mov dword ptr [r12], eax
    mov edi, 4
    mov esi, dword ptr [rbx + 4]
    call last_column
    mov dword ptr [r12 + 4], eax
    mov edi, 8
    mov esi, dword ptr [rbx + 8]
    call last_column
    mov dword ptr [r12 + 8], eax
    mov edi, 12
    mov esi, dword ptr [rbx + 12]
    call last_column
    mov dword ptr [r12 + 12], eax
    add rsp, 8
    pop r12
    pop rbx
    ret
.Llast_round_assert_key:
    mov edi, 46
    call assert_fail
.Llast_round_assert_out:
    mov edi, 47
    call assert_fail
.size last_round, . - last_round

# One column of the last round: the bytes ShiftRows brings into it go through the S-box, which is
# byte 1 of the table's words, and the round key's word is added.
# In: edi = 4 times the column, 0 to 12; esi = the round key's word for that column.
# Out: eax = the column's new word.
.type last_column, @function
last_column:
    lea r8, [rip + current]
    lea r9, [rip + te0]
    cmp rdi, CURRENT_LEN - 16 + 1
    jae .Llast_column_assert_index
    mov eax, dword ptr [r8 + rdi]
    movzx edx, al
    cmp rdx, TABLE_ENTRIES
    jae .Llast_column_assert_row0
    movzx ecx, byte ptr [r9 + rdx*4 + 1]
    mov eax, dword ptr [r8 + rdi + 4]
    shr eax, 8
    movzx edx, al
    cmp rdx, TABLE_ENTRIES
    jae .Llast_column_assert_row1
    movzx edx, byte ptr [r9 + rdx*4 + 1]
    shl edx, 8
    or ecx, edx
    mov eax, dword ptr [r8 + rdi + 8]
    shr eax, 16
    movzx edx, al
    cmp rdx, TABLE_ENTRIES
    jae .Llast_column_assert_row2
    movzx edx, byte ptr [r9 + rdx*4 + 1]
    shl edx, 16
    or ecx, edx
    mov eax, dword ptr [r8 + rdi + 12]
    shr eax, 24
    cmp rax, TABLE_ENTRIES
    jae .Llast_column_assert_row3
    movzx edx, byte ptr [r9 + rax*4 + 1]
    shl edx, 24
    or ecx, edx
    xor ecx, esi
    mov eax, ecx
    ret
.Llast_column_assert_index:
    mov edi, 48
    call assert_fail
.Llast_column_assert_row0:
    mov edi, 49
    call assert_fail
.Llast_column_assert_row1:
    mov edi, 50
    call assert_fail
.Llast_column_assert_row2:
    mov edi, 51
    call assert_fail
.Llast_column_assert_row3:
    mov edi, 52
    call assert_fail
.size last_column, . - last_column

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
