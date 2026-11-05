# The x86-64 library: functions a program may call without defining them, written once for every
# operating system because they reach the OS only through the operations (os_write, os_exit, ...).
# src/edge/mod.rs says how this file is read: a line `# == name: needs` starts a snippet, which is
# written when main.s calls `name` or another snippet needs it.

# == assert_fail: os_write os_exit
.section .rodata
.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.text
.type assert_fail, @function
# lengua's assertion handler.
# In: edi = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
# standard error and ends the program, every thread of it, with code 134.
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
    mov edi, 2
    mov rsi, rsp
    mov edx, 29
    call os_write
    mov edi, 134
    call os_exit
.size assert_fail, . - assert_fail

# == fill: assert_fail os_read
.equ LENGUA_FILL_READS, 1099511627776
.equ LENGUA_FILL_IO, -5

.text
.type fill, @function
# Reads from file descriptor edi into the rdx bytes at rsi until they're full, the input ends,
# or, when ecx is a byte (0 to 255), the last byte read is ecx; with ecx = -1 it reads to the
# end. However the input arrives, it reads the same.
# In: edi = the file descriptor, rsi = the buffer, rdx = its size, at least 1, ecx = the end
# byte or -1.
# Out: rax = how many bytes it read, or a negative error code.
fill:
    sub rsp, 8
    test rsi, rsi
    jz .Llengua_fill_assert_null
    test rdx, rdx
    jz .Llengua_fill_assert_empty
    mov r8, rdx
    mov r10d, ecx
    mov r9, LENGUA_FILL_READS
.Llengua_fill_loop:  # bound: 1099511627776
    call os_read
    test rax, rax
    js .Llengua_fill_return
    jz .Llengua_fill_done
    cmp rax, rdx
    ja .Llengua_fill_assert_count
    add rsi, rax
    sub rdx, rax
    jz .Llengua_fill_done
    movzx eax, byte ptr [rsi - 1]
    cmp eax, r10d
    je .Llengua_fill_done
    dec r9
    jnz .Llengua_fill_loop
    mov rax, LENGUA_FILL_IO
    jmp .Llengua_fill_return
.Llengua_fill_done:
    mov rax, r8
    sub rax, rdx
.Llengua_fill_return:
    add rsp, 8
    ret
.Llengua_fill_assert_null:
    mov edi, 990
    call assert_fail
.Llengua_fill_assert_empty:
    mov edi, 991
    call assert_fail
.Llengua_fill_assert_count:
    mov edi, 992
    call assert_fail
.size fill, . - fill

# == parallel_for: assert_fail os_thread_start os_thread_join
.text
.type parallel_for, @function
# Runs worker(index, slot) for every index from 0 to count - 1, each on its own thread (worker 0
# on this one), and returns when all have finished. worker is main.s's: it takes rdi = its index
# and rsi = its slot, writes only its own slot and its own stack, and returns a result in rax,
# which lands in results[index].
# In: rdi = count, from 1 to 64; rsi = the slots, slot_size bytes each; rdx = slot_size;
# rcx = the results, 8 bytes each.
parallel_for:
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov r12, rdi
    mov r13, rsi
    mov r14, rdx
    mov r15, rcx
    cmp r12, 1
    jb .Llengua_parallel_assert_none
    cmp r12, 64
    ja .Llengua_parallel_assert_many
    mov ebx, 1
.Llengua_parallel_start:  # bound: 64
    cmp rbx, r12
    jae .Llengua_parallel_started
    mov rdi, rbx
    mov rax, rbx
    imul rax, r14
    lea rsi, [r13 + rax]
    call os_thread_start
    test rax, rax
    js .Llengua_parallel_assert_start
    inc rbx
    jmp .Llengua_parallel_start
.Llengua_parallel_started:
    xor edi, edi
    mov rsi, r13
    call worker
    mov qword ptr [r15], rax
    mov ebx, 1
.Llengua_parallel_join:  # bound: 64
    cmp rbx, r12
    jae .Llengua_parallel_joined
    mov rdi, rbx
    call os_thread_join
    mov qword ptr [r15 + 8*rbx], rax
    inc rbx
    jmp .Llengua_parallel_join
.Llengua_parallel_joined:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Llengua_parallel_assert_none:
    mov edi, 901
    call assert_fail
.Llengua_parallel_assert_many:
    mov edi, 902
    call assert_fail
.Llengua_parallel_assert_start:
    mov edi, 903
    call assert_fail
.size parallel_for, . - parallel_for
