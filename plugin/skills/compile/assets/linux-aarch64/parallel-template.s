.arch armv8-a

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
// Threads, the program's choices: at most the threads the idea allows, and each one's output
// slot, a multiple of 64 bytes. lengua gives each thread its stack.
.equ THREADS, 16
.equ SLOT_SIZE, CHUNK
.equ SLOT_SCAN, SLOT_SIZE - VECTOR + 1
.equ RESULTS_LEN, THREADS * QWORD
.equ SLOTS_LEN, THREADS * SLOT_SIZE

.section .rodata

.type too_long_msg, @object
too_long_msg:
    .ascii "shout: the input is longer than 1048576 bytes\n"
.size too_long_msg, . - too_long_msg

.section .bss

// Room for one byte past the input's end, to tell whether there is more.
.type probe, @object
probe:
    .zero QWORD
.size probe, . - probe

// shared: main records the input's length, then the workers read it
.type input_size, @object
input_size:
    .zero QWORD
.size input_size, . - input_size

// shared: each worker's result, stored by parallel_for, read by the program
.type results, @object
.balign 8
results:
    .zero RESULTS_LEN
.size results, . - results

// shared: each worker's slot, filled with its part of the input by read_input, capitalized in
// place by its worker through x19, and printed by main
.type slots, @object
.balign 64
slots:
    .zero SLOTS_LEN
.size slots, . - slots

.text
// Reads the input, capitalizes it on up to THREADS threads, prints it and exits. lengua's `_start`
// calls main, which never returns, and lengua's parallel_for runs worker on each thread.
.type main, @function
main:
    bl read_input
    tbnz x0, #63, .Lmain_read_failed
    cmp x0, #INPUT_MAX
    b.hi .Lmain_assert_length
    mov x19, x0
    adrp x9, input_size
    add x9, x9, :lo12:input_size
    str x19, [x9]
    mov x10, #CHUNK
    add x11, x19, x10
    sub x11, x11, #1
    udiv x11, x11, x10
    cbnz x11, .Lmain_count
    mov x11, #1
.Lmain_count:
    mov x20, x11
    cmp x20, #THREADS
    b.hi .Lmain_assert_threads
    mov x0, x20
    adrp x1, slots
    add x1, x1, :lo12:slots
    mov x2, #SLOT_SIZE
    adrp x3, results
    add x3, x3, :lo12:results
    bl parallel_for
    mov x0, x20
    bl total_results
    cmp x0, x19
    b.ne .Lmain_assert_total
    cbz x19, .Lmain_done
    mov x0, #STDOUT
    adrp x1, slots
    add x1, x1, :lo12:slots
    mov x2, x19
    bl write_all
    tbnz x0, #63, .Lmain_io_failed
.Lmain_done:
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_read_failed:
    cmn x0, #7
    b.ne .Lmain_io_failed
    mov x0, #STDERR
    adrp x1, too_long_msg
    add x1, x1, :lo12:too_long_msg
    mov x2, #TOO_LONG_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lmain_io_failed
    mov w0, #EXIT_TOO_LONG
    b .Lmain_exit
.Lmain_io_failed:
    mov w0, #EXIT_IO_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_length:
    mov w0, #1
    bl assert_fail
.Lmain_assert_total:
    mov w0, #2
    bl assert_fail
.Lmain_assert_threads:
    mov w0, #8
    bl assert_fail
.size main, . - main

// Reads standard input straight into the threads' slots, which lie end to end, until it ends or
// they're full; then one byte more tells whether the input is too long.
// Out: x0 = the input's length, ERROR_TOO_LONG when it's longer than INPUT_MAX, or ERROR_IO
// when reading fails.
// returns: result
.type read_input, @function
read_input:
    stp x29, x30, [sp, #-16]!
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
    bl os_read
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
    bl os_read
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
    ldp x29, x30, [sp], #16
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
    stp x29, x30, [sp, #-16]!
    cbz x1, .Lwrite_all_assert_null
    cbz x2, .Lwrite_all_assert_empty
    mov x9, x0
    mov x10, #MAX_WRITES
.Lwrite_all_loop:  // bound: 64
    mov x0, x9
    bl os_write
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
    ldp x29, x30, [sp], #16
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

