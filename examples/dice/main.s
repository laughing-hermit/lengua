# lengua: spec sha256:721c17ef6bd6170dd13d9304a98c0aef298603a55f4f3b1eb3960572593eeac9
.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_WRITE, 1
.equ SYS_EXIT, 60
.equ SYS_GETRANDOM, 318
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

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

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
.globl _start

# Draws random bytes, rolls with the first one below SET_ASIDE_FROM, prints the roll and exits.
.type _start, @function
_start:
    call draw
    test rax, rax
    js .Lstart_no_random
    cmp edx, SET_ASIDE_FROM
    jae .Lstart_assert_byte
    mov eax, edx
    xor edx, edx
    mov ecx, FACES
    div ecx
    cmp edx, FACES
    jae .Lstart_assert_face
    add edx, '1'
    mov byte ptr [rip + line + ROLL_INDEX], dl
    mov edi, STDOUT
    lea rsi, [rip + line]
    mov edx, LINE_LEN
    call write_all
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_no_random:
    mov edi, EXIT_NO_RANDOM
    jmp .Lstart_exit
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_byte:
    mov edi, 5
    call assert_fail
.Lstart_assert_face:
    mov edi, 6
    call assert_fail
.size _start, . - _start

# Draws DRAW_BYTES random bytes from the kernel and finds the first one below SET_ASIDE_FROM.
# Out: rax = 0 and rdx = that byte, or rax = ERROR_IO when the kernel gives too few bytes or
# sets every one aside.
# returns: result
.type draw, @function
draw:
    mov eax, SYS_GETRANDOM
    lea rdi, [rip + random]
    mov esi, DRAW_BYTES
    xor edx, edx
    syscall
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
    mov edi, 2
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 3
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 4
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
