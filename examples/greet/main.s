.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_WRITE, 1
.equ SYS_EXIT, 60
# File descriptors
.equ STDOUT, 1
.equ STDERR, 2
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_WRITE_FAILED, 3
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
# Sizes and limits
.equ MAX_ARGS, 4096
.equ MAX_VARIABLES, 256
.equ PREFIX_LEN, 5
.equ MAX_NAME, 64
.equ MAX_NAME_SCAN, 65
.equ HELLO_LEN, 7
.equ OUT_BUF_LEN, 73
.equ USAGE_LEN, 24
.equ MAX_WRITES, 64
# Characters
.equ NEWLINE, 10

.section .rodata

# What the variable that names who to greet starts with.
.type prefix, @object
prefix:
    .ascii "NAME="
.size prefix, . - prefix

.type hello, @object
hello:
    .ascii "Hello, "
.size hello, . - hello

.type usage_msg, @object
usage_msg:
    .ascii "usage: NAME=<who> greet\n"
.size usage_msg, . - usage_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# The greeting, as written.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

# Greets whoever NAME names, or prints how to use the program.
.type _start, @function
_start:
    mov rax, qword ptr [rsp]
    cmp rax, MAX_ARGS
    jae .Lstart_assert_argc
    lea rdi, [rsp + rax*8 + 16]
    call find_name
    test rax, rax
    jz .Lstart_usage
    mov rbx, rax
    mov rdi, rax
    call name_length
    test rax, rax
    js .Lstart_usage
    mov rsi, rax
    mov rdi, rbx
    call greet
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_usage:
    mov edi, STDERR
    lea rsi, [rip + usage_msg]
    mov edx, USAGE_LEN
    call write_all
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_USAGE
    jmp .Lstart_exit
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_argc:
    mov edi, 1
    call assert_fail
.size _start, . - _start

# Finds the NAME variable in the environment.
# In: rdi = the environment's pointers, each to a NUL-terminated "NAME=value", then 0.
# Out: rax = NAME's value, after "NAME=", or 0 when no variable is NAME.
.type find_name, @function
find_name:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Lfind_name_assert_null
    mov rbx, rdi
    xor r12d, r12d
    mov r13, MAX_VARIABLES
.Lfind_name_loop:  # bound: 256
    cmp r12, MAX_VARIABLES
    jae .Lfind_name_assert_index
    mov rdi, qword ptr [rbx + r12*8]
    test rdi, rdi
    jz .Lfind_name_none
    call value_if_name
    test rax, rax
    jnz .Lfind_name_return
    inc r12
    dec r13
    jnz .Lfind_name_loop
.Lfind_name_none:
    xor eax, eax
.Lfind_name_return:
    pop r13
    pop r12
    pop rbx
    ret
.Lfind_name_assert_null:
    mov edi, 2
    call assert_fail
.Lfind_name_assert_index:
    mov edi, 3
    call assert_fail
.size find_name, . - find_name

# The value of a variable, when it's NAME.
# In: rdi = a NUL-terminated "NAME=value".
# Out: rax = the value, after "NAME=", or 0 for any other variable.
.type value_if_name, @function
value_if_name:
    test rdi, rdi
    jz .Lvalue_if_name_assert_null
    lea rsi, [rip + prefix]
    xor ecx, ecx
    mov r8, PREFIX_LEN
.Lvalue_if_name_loop:  # bound: 5
    cmp rcx, PREFIX_LEN
    jae .Lvalue_if_name_assert_index
    movzx eax, byte ptr [rdi + rcx]
    cmp al, byte ptr [rsi + rcx]
    jne .Lvalue_if_name_other
    inc rcx
    dec r8
    jnz .Lvalue_if_name_loop
    lea rax, [rdi + PREFIX_LEN]
    jmp .Lvalue_if_name_return
.Lvalue_if_name_other:
    xor eax, eax
.Lvalue_if_name_return:
    ret
.Lvalue_if_name_assert_null:
    mov edi, 4
    call assert_fail
.Lvalue_if_name_assert_index:
    mov edi, 5
    call assert_fail
.size value_if_name, . - value_if_name

# How long a name is.
# In: rdi = a NUL-terminated name.
# Out: rax = its length, 1 to MAX_NAME, or ERROR_INVALID when it's empty or longer.
# returns: result
.type name_length, @function
name_length:
    test rdi, rdi
    jz .Lname_length_assert_null
    xor ecx, ecx
    mov r8, MAX_NAME_SCAN
.Lname_length_loop:  # bound: 65
    cmp rcx, MAX_NAME_SCAN
    jae .Lname_length_assert_index
    cmp byte ptr [rdi + rcx], 0
    je .Lname_length_end
    inc rcx
    dec r8
    jnz .Lname_length_loop
    mov rax, ERROR_INVALID
    jmp .Lname_length_return
.Lname_length_end:
    mov rax, rcx
    test rcx, rcx
    jnz .Lname_length_return
    mov rax, ERROR_INVALID
.Lname_length_return:
    ret
.Lname_length_assert_null:
    mov edi, 6
    call assert_fail
.Lname_length_assert_index:
    mov edi, 7
    call assert_fail
.size name_length, . - name_length

# Writes "Hello, ", the name, "!" and a newline to standard output.
# In: rdi = the name, rsi = its length, 1 to MAX_NAME.
# Out: rax = 0, or a negative error code.
# returns: result
.type greet, @function
greet:
    sub rsp, 8
    test rdi, rdi
    jz .Lgreet_assert_null
    test rsi, rsi
    jz .Lgreet_assert_empty
    cmp rsi, MAX_NAME
    ja .Lgreet_assert_long
    lea rdx, [rip + out_buf]
    lea r8, [rip + hello]
    xor ecx, ecx
    mov r9, HELLO_LEN
.Lgreet_hello:  # bound: 7
    cmp rcx, HELLO_LEN
    jae .Lgreet_assert_hello_index
    movzx eax, byte ptr [r8 + rcx]
    mov byte ptr [rdx + rcx], al
    inc rcx
    dec r9
    jnz .Lgreet_hello
    xor ecx, ecx
    mov r9, MAX_NAME
.Lgreet_name:  # bound: 64
    cmp rcx, rsi
    jae .Lgreet_end
    cmp rcx, MAX_NAME
    jae .Lgreet_assert_name_index
    movzx eax, byte ptr [rdi + rcx]
    mov byte ptr [rdx + rcx + HELLO_LEN], al
    inc rcx
    dec r9
    jnz .Lgreet_name
.Lgreet_end:
    cmp rcx, MAX_NAME_SCAN
    jae .Lgreet_assert_end_index
    mov byte ptr [rdx + rcx + HELLO_LEN], '!'
    mov byte ptr [rdx + rcx + HELLO_LEN + 1], NEWLINE
    lea rdx, [rcx + HELLO_LEN + 2]
    lea rsi, [rip + out_buf]
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lgreet_return
    xor eax, eax
.Lgreet_return:
    add rsp, 8
    ret
.Lgreet_assert_null:
    mov edi, 8
    call assert_fail
.Lgreet_assert_empty:
    mov edi, 9
    call assert_fail
.Lgreet_assert_long:
    mov edi, 10
    call assert_fail
.Lgreet_assert_hello_index:
    mov edi, 11
    call assert_fail
.Lgreet_assert_name_index:
    mov edi, 12
    call assert_fail
.Lgreet_assert_end_index:
    mov edi, 13
    call assert_fail
.size greet, . - greet

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
    mov edi, 14
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 15
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 16
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
