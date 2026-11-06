.intel_syntax noprefix

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
.equ VECTOR, 16
.equ QWORD, 8
.equ CHUNK, 65536
.equ CHUNK_VECTORS, CHUNK / VECTOR
.equ MAX_WRITES, 64
.equ TOO_LONG_MSG_LEN, 46
# Bytes repeated four times, spread over a vector register with pshufd
.equ BELOW_A_X4, 0x60606060
.equ ABOVE_Z_X4, 0x7b7b7b7b
.equ CASE_BIT_X4, 0x20202020
# Threads, the program's choices: at most the threads the idea allows, and each one's output
# slot, a multiple of 64 bytes. lengua gives each thread its stack.
.equ THREADS, 16
.equ SLOT_SIZE, CHUNK
.equ RESULTS_LEN, THREADS * QWORD
.equ SLOTS_LEN, THREADS * SLOT_SIZE

.section .rodata

.type too_long_msg, @object
too_long_msg:
    .ascii "shout: the input is longer than 1048576 bytes\n"
.size too_long_msg, . - too_long_msg

.section .bss

# Room for one byte past the input's end, to tell whether there is more.
.type probe, @object
probe:
    .zero QWORD
.size probe, . - probe

# shared: main records the input's length, then the workers read it
.type input_size, @object
input_size:
    .zero QWORD
.size input_size, . - input_size

# shared: each worker's result, stored by parallel_for, read by the program
.type results, @object
.balign 8
results:
    .zero RESULTS_LEN
.size results, . - results

# shared: each worker's slot, filled with its part of the input by read_input, capitalized in
# place by its worker through rbx, and printed by main
.type slots, @object
.balign 64
slots:
    .zero SLOTS_LEN
.size slots, . - slots

.text
# Reads the input, capitalizes it on up to THREADS threads, prints it and exits. lengua's `_start`
# calls main, which never returns, and lengua's parallel_for runs worker on each thread.
.type main, @function
main:
    sub rsp, 8
    call read_input
    test rax, rax
    js .Lmain_read_failed
    cmp rax, INPUT_MAX
    ja .Lmain_assert_length
    mov rbx, rax
    mov qword ptr [rip + input_size], rbx
    lea rax, [rbx + CHUNK - 1]
    xor edx, edx
    mov ecx, CHUNK
    div rcx
    test rax, rax
    jnz .Lmain_count
    inc eax
.Lmain_count:
    mov r12, rax
    cmp r12, THREADS
    ja .Lmain_assert_threads
    mov rdi, r12
    lea rsi, [rip + slots]
    mov edx, SLOT_SIZE
    lea rcx, [rip + results]
    call parallel_for
    mov rdi, r12
    call total_results
    cmp rax, rbx
    jne .Lmain_assert_total
    test rbx, rbx
    jz .Lmain_done
    mov edi, STDOUT
    lea rsi, [rip + slots]
    mov rdx, rbx
    call write_all
    test rax, rax
    js .Lmain_io_failed
.Lmain_done:
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_read_failed:
    cmp rax, ERROR_TOO_LONG
    jne .Lmain_io_failed
    mov edi, STDERR
    lea rsi, [rip + too_long_msg]
    mov edx, TOO_LONG_MSG_LEN
    call write_all
    test rax, rax
    js .Lmain_io_failed
    mov edi, EXIT_TOO_LONG
    jmp .Lmain_exit
.Lmain_io_failed:
    mov edi, EXIT_IO_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_length:
    mov edi, 1
    call assert_fail
.Lmain_assert_total:
    mov edi, 2
    call assert_fail
.Lmain_assert_threads:
    mov edi, 8
    call assert_fail
.size main, . - main

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
    call os_read
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
    call os_read
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

# The work of one thread: capitalizes its CHUNK of the input in place, in its own slot, 16 bytes
# at a time: a to z become A to Z, other bytes stay. A last, partial chunk may change bytes of
# the slot past its end, which is harmless: only the input's length is printed.
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
    mov eax, BELOW_A_X4
    movd xmm1, eax
    pshufd xmm1, xmm1, 0
    mov eax, ABOVE_Z_X4
    movd xmm2, eax
    pshufd xmm2, xmm2, 0
    mov eax, CASE_BIT_X4
    movd xmm3, eax
    pshufd xmm3, xmm3, 0
    xor r9d, r9d
    mov r8, CHUNK_VECTORS
.Lworker_loop:  # bound: 4096
    cmp r9, rdx
    jae .Lworker_done
    cmp r9, SLOT_SIZE - VECTOR + 1
    jae .Lworker_assert_slot
    movdqu xmm0, xmmword ptr [rbx + r9]
    movdqu xmm4, xmm0
    pcmpgtb xmm4, xmm1
    movdqu xmm5, xmm2
    pcmpgtb xmm5, xmm0
    pand xmm4, xmm5
    pand xmm4, xmm3
    psubb xmm0, xmm4
    movdqu xmmword ptr [rbx + r9], xmm0
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
    mov edi, 13
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 14
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 15
    call assert_fail
.size write_all, . - write_all

