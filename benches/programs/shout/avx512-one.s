# lengua: spec sha256:5e8481da31a6756ba6b2eab7257bd845a9bc3f3f361260fe7419a830101c669d
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
.equ EXIT_TOO_LONG, 1
.equ EXIT_IO_FAILED, 2
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_TOO_LONG, -7
# Sizes and limits
.equ INPUT_MAX, 1048576
.equ MAX_READS, INPUT_MAX + 1
.equ PROBE_BYTES, 1
.equ VECTOR, 64
.equ QWORD, 8
.equ CHUNK, 1048576
.equ CHUNK_VECTORS, CHUNK / VECTOR
.equ MAX_WRITES, 64
.equ TOO_LONG_MSG_LEN, 46
# Threads: what parallel_for needs, copied word for word from the template...
.equ SYS_CLONE, 56
.equ SYS_FUTEX, 202
.equ CLONE_FLAGS, 0x350F00
.equ FUTEX_WAIT, 0
.equ THREAD_EXIT, 0
.equ MAX_WAITS, 64
# ...and the program's choices: at most the threads the spec allows, each one's stack, and each
# one's output slot, both multiples of 64 bytes.
.equ THREADS, 16
.equ STACK_SIZE, 4096
.equ SLOT_SIZE, CHUNK
.equ STACKS_LEN, THREADS * STACK_SIZE
.equ TIDS_LEN, THREADS * 4
.equ RESULTS_LEN, THREADS * QWORD
.equ SLOTS_LEN, THREADS * SLOT_SIZE

.section .rodata

.type too_long_msg, @object
too_long_msg:
    .ascii "shout: the input is longer than 1048576 bytes\n"
.size too_long_msg, . - too_long_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# Room for one byte past the input's end, to tell whether there is more.
.type probe, @object
probe:
    .zero QWORD
.size probe, . - probe

# shared: _start records the input's length, then the workers read it
.type input_size, @object
input_size:
    .zero QWORD
.size input_size, . - input_size

# Each thread's own stack (copied word for word from the template).
.type stacks, @object
.balign 64
stacks:
    .zero STACKS_LEN
.size stacks, . - stacks

# shared: start_worker records each thread's id, and join_worker waits for it to clear
.type tids, @object
.balign 8
tids:
    .zero TIDS_LEN
.size tids, . - tids

# shared: each worker's result, stored by parallel_for and start_worker, read by the program
.type results, @object
.balign 8
results:
    .zero RESULTS_LEN
.size results, . - results

# shared: each worker's slot, filled with its part of the input by read_input, capitalized in
# place by its worker through rbx, and printed by _start
.type slots, @object
.balign 64
slots:
    .zero SLOTS_LEN
.size slots, . - slots

.text
.globl _start

# Reads the input, capitalizes it on up to THREADS threads, prints it and exits.
.type _start, @function
_start:
    call read_input
    test rax, rax
    js .Lstart_read_failed
    cmp rax, INPUT_MAX
    ja .Lstart_assert_length
    mov rbx, rax
    mov qword ptr [rip + input_size], rbx
    lea rax, [rbx + CHUNK - 1]
    xor edx, edx
    mov ecx, CHUNK
    div rcx
    test rax, rax
    jnz .Lstart_count
    inc eax
.Lstart_count:
    mov r12, rax
    mov rdi, r12
    call parallel_for
    mov rdi, r12
    call total_results
    cmp rax, rbx
    jne .Lstart_assert_total
    test rbx, rbx
    jz .Lstart_done
    mov edi, STDOUT
    lea rsi, [rip + slots]
    mov rdx, rbx
    call write_all
    test rax, rax
    js .Lstart_io_failed
.Lstart_done:
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_read_failed:
    cmp rax, ERROR_TOO_LONG
    jne .Lstart_io_failed
    mov edi, STDERR
    lea rsi, [rip + too_long_msg]
    mov edx, TOO_LONG_MSG_LEN
    call write_all
    test rax, rax
    js .Lstart_io_failed
    mov edi, EXIT_TOO_LONG
    jmp .Lstart_exit
.Lstart_io_failed:
    mov edi, EXIT_IO_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_length:
    mov edi, 1
    call assert_fail
.Lstart_assert_total:
    mov edi, 2
    call assert_fail
.size _start, . - _start

# Reads standard input straight into the threads' slots, which lie end to end, until it ends or
# they're full; then one byte more tells whether the input is too long.
# Out: rax = the input's length, ERROR_TOO_LONG when it's longer than INPUT_MAX, or ERROR_IO
# when reading fails.
# returns: result
.type read_input, @function
read_input:
    push rbx
    push r12
    sub rsp, 8
    xor ebx, ebx
    mov r12, MAX_READS
.Lread_loop:  # bound: 1048577
    cmp rbx, INPUT_MAX
    jae .Lread_full
    mov edi, STDIN
    lea rsi, [rip + slots]
    add rsi, rbx
    mov edx, INPUT_MAX
    sub rdx, rbx
    mov eax, SYS_READ
    syscall
    test rax, rax
    js .Lread_failed
    jz .Lread_ended
    add rbx, rax
    cmp rbx, INPUT_MAX
    ja .Lread_assert_count
    dec r12
    jnz .Lread_loop
    jmp .Lread_assert_reads
.Lread_full:
    mov edi, STDIN
    lea rsi, [rip + probe]
    mov edx, PROBE_BYTES
    mov eax, SYS_READ
    syscall
    test rax, rax
    js .Lread_failed
    jnz .Lread_too_long
.Lread_ended:
    mov rax, rbx
    jmp .Lread_return
.Lread_too_long:
    mov rax, ERROR_TOO_LONG
    jmp .Lread_return
.Lread_failed:
    mov rax, ERROR_IO
.Lread_return:
    add rsp, 8
    pop r12
    pop rbx
    ret
.Lread_assert_count:
    mov edi, 3
    call assert_fail
.Lread_assert_reads:
    mov edi, 4
    call assert_fail
.size read_input, . - read_input

# The work of one thread: capitalizes its CHUNK of the input, all of it, into its slot, 64
# bytes at a time with AVX-512:
# a to z become A to Z, other bytes stay. A last, partial chunk may fill bytes of the slot past
# its end, which is harmless: only the input's length is printed.
# In: rdi = the worker's index, below THREADS; rsi = its slot, SLOT_SIZE bytes.
# Out: rax = how many bytes of the input it capitalized.
.type worker, @function
worker:
    push rbx
    mov rbx, rsi
    cmp rdi, THREADS
    jae .Lworker_assert_index
    imul rcx, rdi, CHUNK
    mov rdx, qword ptr [rip + input_size]
    cmp rdx, INPUT_MAX
    ja .Lworker_assert_size
    sub rdx, rcx
    jb .Lworker_assert_start
    cmp rdx, CHUNK
    jbe .Lworker_sized
    mov edx, CHUNK
.Lworker_sized:
    mov eax, 'a'
    vpbroadcastb zmm1, eax
    mov eax, 26
    vpbroadcastb zmm2, eax
    mov eax, 0x20
    vpbroadcastb zmm3, eax
    xor r9d, r9d
    mov r8, CHUNK_VECTORS
.Lworker_loop:  # bound: 16384
    cmp r9, rdx
    jae .Lworker_done
    cmp r9, SLOT_SIZE - VECTOR + 1
    jae .Lworker_assert_slot
    vmovdqu8 zmm0, zmmword ptr [rbx + r9]
    vpsubb zmm4, zmm0, zmm1
    vpcmpub k1, zmm4, zmm2, 1
    vpsubb zmm0 {k1}, zmm0, zmm3
    vmovdqu8 zmmword ptr [rbx + r9], zmm0
    add r9, VECTOR
    dec r8
    jnz .Lworker_loop
.Lworker_done:
    mov rax, rdx
    pop rbx
    ret
.Lworker_assert_index:
    mov edi, 5
    call assert_fail
.Lworker_assert_size:
    mov edi, 6
    call assert_fail
.Lworker_assert_start:
    mov edi, 7
    call assert_fail
.Lworker_assert_slot:
    mov edi, 9
    call assert_fail
.size worker, . - worker

# Adds up the results of the workers that ran.
# In: rdi = how many ran, 1 to THREADS.
# Out: rax = the sum.
.type total_results, @function
total_results:
    test rdi, rdi
    jz .Ltotal_assert_none
    cmp rdi, THREADS
    ja .Ltotal_assert_many
    lea rsi, [rip + results]
    xor eax, eax
    xor ecx, ecx
    mov r8, THREADS
.Ltotal_loop:  # bound: 16
    cmp rcx, rdi
    jae .Ltotal_return
    cmp rcx, THREADS
    jae .Ltotal_assert_index
    add rax, qword ptr [rsi + rcx*8]
    inc rcx
    dec r8
    jnz .Ltotal_loop
.Ltotal_return:
    ret
.Ltotal_assert_none:
    mov edi, 10
    call assert_fail
.Ltotal_assert_many:
    mov edi, 11
    call assert_fail
.Ltotal_assert_index:
    mov edi, 12
    call assert_fail
.size total_results, . - total_results

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
    mov edi, 13
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 14
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 15
    call assert_fail
.size write_all, . - write_all

# lengua's fork-join threads. check_code compares parallel_for, start_worker and join_worker
# word for word, with stacks, tids, results and slots: copy them exactly.
# parallel_for runs worker(index, slot) for every index from 0 to count - 1, each on its own
# thread (worker 0 on this one), and returns when all have finished. A worker writes only its
# own stack and its slot, through rbx, and returns a result in rax, which lands in
# results[index].
# In: rdi = count, from 1 to THREADS.
.type parallel_for, @function
parallel_for:
    push rbx
    push r12
    push r13
    mov r12, rdi
    cmp r12, 1
    jb .Lparallel_for_assert_none
    cmp r12, THREADS
    ja .Lparallel_for_assert_many
    mov ebx, 1
    mov r13, THREADS
.Lparallel_for_start:  # bound: 64
    cmp rbx, r12
    jae .Lparallel_for_started
    mov rdi, rbx
    call start_worker
    inc rbx
    dec r13
    jnz .Lparallel_for_start
.Lparallel_for_started:
    xor edi, edi
    lea rsi, [rip + slots]
    call worker
    mov qword ptr [rip + results], rax
    mov ebx, 1
    mov r13, THREADS
.Lparallel_for_join:  # bound: 64
    cmp rbx, r12
    jae .Lparallel_for_joined
    mov rdi, rbx
    call join_worker
    inc rbx
    dec r13
    jnz .Lparallel_for_join
.Lparallel_for_joined:
    pop r13
    pop r12
    pop rbx
    ret
.Lparallel_for_assert_none:
    mov edi, 901
    call assert_fail
.Lparallel_for_assert_many:
    mov edi, 902
    call assert_fail
.size parallel_for, . - parallel_for

# Starts a thread on its own stack that runs worker(index, slot), stores the result in
# results[index] and ends.
# In: rdi = index, from 1 to THREADS - 1.
.type start_worker, @function
start_worker:
    push rbx
    mov rbx, rdi
    cmp rbx, THREADS
    jae .Lstart_worker_assert_index
    lea rsi, [rip + stacks]
    mov rax, rbx
    inc rax
    imul rax, rax, STACK_SIZE
    add rsi, rax
    lea rdx, [rip + tids]
    lea rdx, [rdx + rbx*4]
    mov r10, rdx
    mov edi, CLONE_FLAGS
    mov eax, SYS_CLONE
    syscall
    test rax, rax
    js .Lstart_worker_assert_clone
    jz .Lstart_worker_child
    pop rbx
    ret
.Lstart_worker_child:
    mov rdi, rbx
    lea rsi, [rip + slots]
    imul rax, rbx, SLOT_SIZE
    add rsi, rax
    call worker
    lea rcx, [rip + results]
    cmp rbx, THREADS
    jae .Lstart_worker_assert_result
    mov qword ptr [rcx + rbx*8], rax
    mov edi, THREAD_EXIT
    mov eax, SYS_EXIT
    syscall
.Lstart_worker_assert_index:
    mov edi, 903
    call assert_fail
.Lstart_worker_assert_clone:
    mov edi, 904
    call assert_fail
.Lstart_worker_assert_result:
    mov edi, 905
    call assert_fail
.size start_worker, . - start_worker

# Waits until the thread for index has ended: the kernel clears its id in tids and wakes us.
# In: rdi = index, from 1 to THREADS - 1.
.type join_worker, @function
join_worker:
    push rbx
    push r12
    sub rsp, 8
    mov rbx, rdi
    cmp rbx, THREADS
    jae .Ljoin_worker_assert_index
    lea rdi, [rip + tids]
    lea rdi, [rdi + rbx*4]
    mov r12, MAX_WAITS
.Ljoin_worker_wait:  # bound: 64
    mov edx, dword ptr [rdi]
    test edx, edx
    jz .Ljoin_worker_done
    mov esi, FUTEX_WAIT
    xor r10d, r10d
    mov eax, SYS_FUTEX
    syscall
    test rax, rax
    jns .Ljoin_worker_next
.Ljoin_worker_next:
    dec r12
    jnz .Ljoin_worker_wait
.Ljoin_worker_assert_waits:
    mov edi, 906
    call assert_fail
.Ljoin_worker_done:
    add rsp, 8
    pop r12
    pop rbx
    ret
.Ljoin_worker_assert_index:
    mov edi, 907
    call assert_fail
.size join_worker, . - join_worker

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
