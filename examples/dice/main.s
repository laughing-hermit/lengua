.intel_syntax noprefix

# File descriptors
.equ STDOUT, 1
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_WRITE_FAILED, 2
.equ EXIT_NO_RANDOM, 3
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# Sizes and limits
.equ DRAW_BYTES, 64
.equ SET_ASIDE_FROM, 252
.equ FACES, 6
.equ MAX_WRITES, 64
.equ LINE_LEN, 9
.equ ROLL_INDEX, 7

.section .data

# The line it prints, with the roll's digit at ROLL_INDEX.
.type line, @object
line:
    .ascii "rolled ?\n"
.size line, . - line

.section .bss

# The random bytes drawn for one roll.
.type random, @object
random:
    .zero DRAW_BYTES
.size random, . - random

.text
# Draws random bytes, rolls with the first one below SET_ASIDE_FROM, prints the roll and exits.
.type main, @function
main:
    sub rsp, 8
    call draw
    test rax, rax
    js .Lmain_no_random
    cmp edx, SET_ASIDE_FROM
    jae .Lmain_assert_byte
    mov eax, edx
    xor edx, edx
    mov ecx, FACES
    div ecx
    cmp edx, FACES
    jae .Lmain_assert_face
    add edx, '1'
    mov byte ptr [rip + line + ROLL_INDEX], dl
    mov edi, STDOUT
    lea rsi, [rip + line]
    mov edx, LINE_LEN
    call write_all
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_no_random:
    mov edi, EXIT_NO_RANDOM
    jmp .Lmain_exit
.Lmain_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_byte:
    mov edi, 5
    call assert_fail
.Lmain_assert_face:
    mov edi, 6
    call assert_fail
.size main, . - main

# Draws DRAW_BYTES random bytes from the kernel and finds the first one below SET_ASIDE_FROM.
# Out: rax = 0 and rdx = that byte, or rax = ERROR_IO when the kernel gives too few bytes or
# sets every one aside.
# returns: result
.type draw, @function
draw:
    sub rsp, 8
    lea rdi, [rip + random]
    mov esi, DRAW_BYTES
    call os_random
    cmp rax, DRAW_BYTES
    jne .Ldraw_failed
    lea rsi, [rip + random]
    xor ecx, ecx
    mov r8, DRAW_BYTES
.Ldraw_loop:  # bound: 64
    cmp rcx, DRAW_BYTES
    jae .Ldraw_assert_index
    movzx edx, byte ptr [rsi + rcx]
    cmp edx, SET_ASIDE_FROM
    jb .Ldraw_found
    inc rcx
    dec r8
    jnz .Ldraw_loop
.Ldraw_failed:
    mov rax, ERROR_IO
    jmp .Ldraw_return
.Ldraw_found:
    xor eax, eax
.Ldraw_return:
    add rsp, 8
    ret
.Ldraw_assert_index:
    mov edi, 1
    call assert_fail
.size draw, . - draw

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
    mov edi, 2
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 3
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 4
    call assert_fail
.size write_all, . - write_all

