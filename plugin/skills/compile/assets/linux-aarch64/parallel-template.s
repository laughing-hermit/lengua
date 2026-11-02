.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_READ, 63
.equ SYS_WRITE, 64
.equ SYS_EXIT, 93
// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
.equ STDERR, 2
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_TOO_LONG, 1
.equ EXIT_IO_FAILED, 2
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_TOO_LONG, -7
// Sizes and limits
.equ INPUT_MAX, 1048576
.equ MAX_READS, INPUT_MAX + 1
.equ PROBE_BYTES, 1
.equ VECTOR, 16
.equ QWORD, 8
.equ CHUNK, 65536
.equ CHUNK_VECTORS, CHUNK / VECTOR
.equ MAX_WRITES, 64
.equ TOO_LONG_MSG_LEN, 46
// Bytes in every lane with movi: a letter is above BELOW_A and below ABOVE_Z
.equ BELOW_A, 0x60
.equ ABOVE_Z, 0x7b
.equ CASE_BIT, 0x20
// Threads: what parallel_for needs, copied word for word from the template...
.equ SYS_CLONE, 220
.equ SYS_FUTEX, 98
.equ CLONE_FLAGS, 0x350F00
.equ FUTEX_WAIT, 0
.equ THREAD_EXIT, 0
.equ MAX_WAITS, 64
// ...and the program's choices: at most the threads the spec allows, each one's stack, and each
// one's output slot, both multiples of 64 bytes.
.equ THREADS, 16
.equ STACK_SIZE, 4096
.equ SLOT_SIZE, CHUNK
.equ SLOT_SCAN, SLOT_SIZE - VECTOR + 1
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

// Room for one byte past the input's end, to tell whether there is more.
.type probe, @object
probe:
    .zero QWORD
.size probe, . - probe

// shared: _start records the input's length, then the workers read it
.type input_size, @object
input_size:
    .zero QWORD
.size input_size, . - input_size

// Each thread's own stack (copied word for word from the template).
.type stacks, @object
.balign 64
stacks:
    .zero STACKS_LEN
.size stacks, . - stacks

// shared: start_worker records each thread's id, and join_worker waits for it to clear
.type tids, @object
.balign 8
tids:
    .zero TIDS_LEN
.size tids, . - tids

// shared: each worker's result, stored by parallel_for and start_worker, read by the program
.type results, @object
.balign 8
results:
    .zero RESULTS_LEN
.size results, . - results

// shared: each worker's slot, filled with its part of the input by read_input, capitalized in
// place by its worker through x19, and printed by _start
.type slots, @object
.balign 64
slots:
    .zero SLOTS_LEN
.size slots, . - slots

.text
.globl _start

// Reads the input, capitalizes it on up to THREADS threads, prints it and exits.
.type _start, @function
_start:
    bl read_input
    tbnz x0, #63, .Lstart_read_failed
    cmp x0, #INPUT_MAX
    b.hi .Lstart_assert_length
    mov x19, x0
    adrp x9, input_size
    add x9, x9, :lo12:input_size
    str x19, [x9]
    mov x10, #CHUNK
    add x11, x19, x10
    sub x11, x11, #1
    udiv x11, x11, x10
    cbnz x11, .Lstart_count
    mov x11, #1
.Lstart_count:
    mov x20, x11
    mov x0, x20
    bl parallel_for
    mov x0, x20
    bl total_results
    cmp x0, x19
    b.ne .Lstart_assert_total
    cbz x19, .Lstart_done
    mov x0, #STDOUT
    adrp x1, slots
    add x1, x1, :lo12:slots
    mov x2, x19
    bl write_all
    tbnz x0, #63, .Lstart_io_failed
.Lstart_done:
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_read_failed:
    cmn x0, #7
    b.ne .Lstart_io_failed
    mov x0, #STDERR
    adrp x1, too_long_msg
    add x1, x1, :lo12:too_long_msg
    mov x2, #TOO_LONG_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lstart_io_failed
    mov w0, #EXIT_TOO_LONG
    b .Lstart_exit
.Lstart_io_failed:
    mov w0, #EXIT_IO_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_length:
    mov w0, #1
    bl assert_fail
.Lstart_assert_total:
    mov w0, #2
    bl assert_fail
.size _start, . - _start

// Reads standard input straight into the threads' slots, which lie end to end, until it ends or
// they're full; then one byte more tells whether the input is too long.
// Out: x0 = the input's length, ERROR_TOO_LONG when it's longer than INPUT_MAX, or ERROR_IO
// when reading fails.
// returns: result
.type read_input, @function
read_input:
    mov x9, #0
    mov x10, #MAX_READS & 0xffff
    movk x10, #MAX_READS >> 16, lsl #16
.Lread_loop:  // bound: 1048577
    cmp x9, #INPUT_MAX
    b.hs .Lread_full
    mov x0, #STDIN
    adrp x1, slots
    add x1, x1, :lo12:slots
    add x1, x1, x9
    mov x2, #INPUT_MAX
    sub x2, x2, x9
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lread_failed
    cbz x0, .Lread_ended
    add x9, x9, x0
    cmp x9, #INPUT_MAX
    b.hi .Lread_assert_count
    subs x10, x10, #1
    b.ne .Lread_loop
    b .Lread_assert_reads
.Lread_full:
    mov x0, #STDIN
    adrp x1, probe
    add x1, x1, :lo12:probe
    mov x2, #PROBE_BYTES
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lread_failed
    cbnz x0, .Lread_too_long
.Lread_ended:
    mov x0, x9
    b .Lread_return
.Lread_too_long:
    mov x0, #ERROR_TOO_LONG
    b .Lread_return
.Lread_failed:
    mov x0, #ERROR_IO
.Lread_return:
    ret
.Lread_assert_count:
    mov w0, #3
    bl assert_fail
.Lread_assert_reads:
    mov w0, #4
    bl assert_fail
.size read_input, . - read_input

// The work of one thread: capitalizes its CHUNK of the input in place, in its own slot, 16 bytes
// at a time: a to z become A to Z, other bytes stay. A last, partial chunk may change bytes of
// the slot past its end, which is harmless: only the input's length is printed.
// In: x0 = the worker's index, below THREADS; x1 = its slot, SLOT_SIZE bytes.
// Out: x0 = how many bytes of the input it capitalized.
.type worker, @function
worker:
    stp x19, x20, [sp, #-16]!
    mov x19, x1
    cmp x0, #THREADS
    b.hs .Lworker_assert_index
    mov x9, #CHUNK
    mul x10, x0, x9
    adrp x11, input_size
    add x11, x11, :lo12:input_size
    ldr x12, [x11]
    cmp x12, #INPUT_MAX
    b.hi .Lworker_assert_size
    subs x12, x12, x10
    b.lo .Lworker_assert_start
    cmp x12, x9
    csel x12, x9, x12, hi
    movi v1.16b, #BELOW_A
    movi v2.16b, #ABOVE_Z
    movi v3.16b, #CASE_BIT
    mov x14, #0
    mov x16, #CHUNK_VECTORS
.Lworker_loop:  // bound: 4096
    cmp x14, x12
    b.hs .Lworker_done
    // The last place a vector fits in the slot, too wide for cmp's number, held in a register.
    mov x15, #SLOT_SCAN
    cmp x14, x15
    b.hs .Lworker_assert_slot
    ldr q0, [x19, x14]
    // A letter is above 'a' less one and below 'z' plus one, without a sign.
    cmhi v4.16b, v0.16b, v1.16b
    cmhi v5.16b, v2.16b, v0.16b
    and v4.16b, v4.16b, v5.16b
    and v4.16b, v4.16b, v3.16b
    sub v0.16b, v0.16b, v4.16b
    str q0, [x19, x14]
    add x14, x14, #VECTOR
    subs x16, x16, #1
    b.ne .Lworker_loop
.Lworker_done:
    mov x0, x12
    ldp x19, x20, [sp], #16
    ret
.Lworker_assert_index:
    mov w0, #5
    bl assert_fail
.Lworker_assert_size:
    mov w0, #6
    bl assert_fail
.Lworker_assert_start:
    mov w0, #7
    bl assert_fail
.Lworker_assert_slot:
    mov w0, #9
    bl assert_fail
.size worker, . - worker

// Adds up the results of the workers that ran.
// In: x0 = how many ran, 1 to THREADS.
// Out: x0 = the sum.
.type total_results, @function
total_results:
    cbz x0, .Ltotal_assert_none
    cmp x0, #THREADS
    b.hi .Ltotal_assert_many
    adrp x9, results
    add x9, x9, :lo12:results
    mov x10, #0
    mov x11, #0
    mov x12, #THREADS
.Ltotal_loop:  // bound: 16
    cmp x11, x0
    b.hs .Ltotal_return
    cmp x11, #THREADS
    b.hs .Ltotal_assert_index
    ldr x13, [x9, x11, lsl #3]
    add x10, x10, x13
    add x11, x11, #1
    subs x12, x12, #1
    b.ne .Ltotal_loop
.Ltotal_return:
    mov x0, x10
    ret
.Ltotal_assert_none:
    mov w0, #10
    bl assert_fail
.Ltotal_assert_many:
    mov w0, #11
    bl assert_fail
.Ltotal_assert_index:
    mov w0, #12
    bl assert_fail
.size total_results, . - total_results

// Writes x2 bytes from x1 to file descriptor x0, retrying partial writes.
// In: x0 = the file descriptor, x1 = the bytes, x2 = how many, at least 1.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_all, @function
write_all:
    cbz x1, .Lwrite_all_assert_null
    cbz x2, .Lwrite_all_assert_empty
    mov x9, x0
    mov x10, #MAX_WRITES
.Lwrite_all_loop:  // bound: 64
    mov x0, x9
    mov x8, #SYS_WRITE
    svc #0
    tbnz x0, #63, .Lwrite_all_return
    cmp x0, x2
    b.hi .Lwrite_all_assert_count
    add x1, x1, x0
    subs x2, x2, x0
    b.eq .Lwrite_all_done
    subs x10, x10, #1
    b.ne .Lwrite_all_loop
    mov x0, #ERROR_IO
    b .Lwrite_all_return
.Lwrite_all_done:
    mov x0, #0
.Lwrite_all_return:
    ret
.Lwrite_all_assert_null:
    mov w0, #13
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #14
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #15
    bl assert_fail
.size write_all, . - write_all

// lengua's fork-join threads. check compares parallel_for, start_worker and join_worker word for
// word, with stacks, tids, results and slots: copy them exactly.
// parallel_for runs worker(index, slot) for every index from 0 to count - 1, each on its own
// thread (worker 0 on this one), and returns when all have finished. A worker writes only its
// own stack and its slot, through x19, and returns a result in x0, which lands in
// results[index].
// In: x0 = count, from 1 to THREADS.
.type parallel_for, @function
parallel_for:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    mov x20, x0
    cmp x20, #1
    b.lo .Lparallel_for_assert_none
    cmp x20, #THREADS
    b.hi .Lparallel_for_assert_many
    mov x19, #1
    mov x21, #THREADS
.Lparallel_for_start:  // bound: 64
    cmp x19, x20
    b.hs .Lparallel_for_started
    mov x0, x19
    bl start_worker
    add x19, x19, #1
    subs x21, x21, #1
    b.ne .Lparallel_for_start
.Lparallel_for_started:
    mov x0, #0
    adrp x1, slots
    add x1, x1, :lo12:slots
    bl worker
    adrp x9, results
    add x9, x9, :lo12:results
    str x0, [x9]
    mov x19, #1
    mov x21, #THREADS
.Lparallel_for_join:  // bound: 64
    cmp x19, x20
    b.hs .Lparallel_for_joined
    mov x0, x19
    bl join_worker
    add x19, x19, #1
    subs x21, x21, #1
    b.ne .Lparallel_for_join
.Lparallel_for_joined:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lparallel_for_assert_none:
    mov w0, #901
    bl assert_fail
.Lparallel_for_assert_many:
    mov w0, #902
    bl assert_fail
.size parallel_for, . - parallel_for

// Starts a thread on its own stack that runs worker(index, slot), stores the result in
// results[index] and ends. AArch64's clone takes the thread's id in x2 and where to clear it in
// x4.
// In: x0 = index, from 1 to THREADS - 1.
.type start_worker, @function
start_worker:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    mov x19, x0
    cmp x19, #THREADS
    b.hs .Lstart_worker_assert_index
    adrp x1, stacks
    add x1, x1, :lo12:stacks
    add x9, x19, #1
    mov x10, #STACK_SIZE
    madd x1, x9, x10, x1
    adrp x2, tids
    add x2, x2, :lo12:tids
    add x2, x2, x19, lsl #2
    mov x3, #0
    mov x4, x2
    mov x0, #CLONE_FLAGS & 0xffff
    movk x0, #CLONE_FLAGS >> 16, lsl #16
    mov x8, #SYS_CLONE
    svc #0
    tbnz x0, #63, .Lstart_worker_assert_clone
    cbz x0, .Lstart_worker_child
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lstart_worker_child:
    mov x0, x19
    adrp x1, slots
    add x1, x1, :lo12:slots
    mov x9, #SLOT_SIZE
    madd x1, x19, x9, x1
    bl worker
    adrp x9, results
    add x9, x9, :lo12:results
    cmp x19, #THREADS
    b.hs .Lstart_worker_assert_result
    str x0, [x9, x19, lsl #3]
    mov w0, #THREAD_EXIT
    mov x8, #SYS_EXIT
    svc #0
.Lstart_worker_assert_index:
    mov w0, #903
    bl assert_fail
.Lstart_worker_assert_clone:
    mov w0, #904
    bl assert_fail
.Lstart_worker_assert_result:
    mov w0, #905
    bl assert_fail
.size start_worker, . - start_worker

// Waits until the thread for index has ended: the kernel clears its id in tids and wakes us. The
// id is read with ldar, so the worker's result, read after, can't be read before it.
// In: x0 = index, from 1 to THREADS - 1.
.type join_worker, @function
join_worker:
    mov x9, x0
    cmp x9, #THREADS
    b.hs .Ljoin_worker_assert_index
    adrp x11, tids
    add x11, x11, :lo12:tids
    add x11, x11, x9, lsl #2
    mov x10, #MAX_WAITS
.Ljoin_worker_wait:  // bound: 64
    ldar w2, [x11]
    cbz w2, .Ljoin_worker_done
    mov x0, x11
    mov x1, #FUTEX_WAIT
    mov x3, #0
    mov x8, #SYS_FUTEX
    svc #0
    tbnz x0, #63, .Ljoin_worker_next
.Ljoin_worker_next:
    subs x10, x10, #1
    b.ne .Ljoin_worker_wait
.Ljoin_worker_assert_waits:
    mov w0, #906
    bl assert_fail
.Ljoin_worker_done:
    ret
.Ljoin_worker_assert_index:
    mov w0, #907
    bl assert_fail
.size join_worker, . - join_worker

// lengua's assertion handler. check compares it word for word: copy it exactly.
// In: w0 = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
// standard error and ends the program, every thread of it, with code 134.
.type assert_fail, @function
assert_fail:
    sub sp, sp, #32
    adrp x1, assert_message
    add x1, x1, :lo12:assert_message
    ldp x2, x3, [x1]
    stp x2, x3, [sp]
    ldr x2, [x1, #16]
    str x2, [sp, #16]
    ldr w2, [x1, #24]
    str w2, [sp, #24]
    ldrb w2, [x1, #28]
    strb w2, [sp, #28]
    mov w2, #100
    udiv w3, w0, w2
    msub w0, w3, w2, w0
    add w3, w3, #48
    strb w3, [sp, #18]
    mov w2, #10
    udiv w3, w0, w2
    msub w0, w3, w2, w0
    add w3, w3, #48
    strb w3, [sp, #19]
    add w0, w0, #48
    strb w0, [sp, #20]
    mov x0, #2
    mov x1, sp
    mov x2, #29
    mov x8, #64
    svc #0
    mov x0, #134
    mov x8, #94
    svc #0
.size assert_fail, . - assert_fail
