.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_WRITE, 64
.equ SYS_EXIT, 93
// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_READ_FAILED, 3
.equ EXIT_WRITE_FAILED, 4
// fill's reads at most, and what makes it read to the end, as assets/linux-aarch64/fill.s has
// them
.equ SYS_READ, 63
.equ ERROR_IO, -5
.equ MAX_FILL_READS, 1099511627776
.equ READ_TO_END, -1
// A buffer-full of input, and the buffer-fulls the main loop takes at most
.equ BUF_LEN, 256
.equ MAX_FULLS, 1099511627776
.equ MAX_WRITES, 64
// The letter whose pairs it counts
.equ LETTER, 'a'
.equ MAX_DIGITS, 19
.equ NEWLINE, 10
.equ OUT_BUF_LEN, 21

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// One buffer-full of standard input.
// shared: _start fills it, and count_pairs reads it
.type buffer, @object
buffer:
    .zero BUF_LEN
.size buffer, . - buffer

// Sign, 19 digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

// Counts the pairs of LETTER in standard input of any length, a buffer-full at a time through
// fill. What crosses from one buffer-full to the next is x22: 1 when the last letter read is an
// unpaired LETTER, else 0.
.type _start, @function
_start:
    mov x21, #0
    mov x22, #0
    mov x19, #MAX_FULLS
.Lstart_full:  // bound: 1099511627776
    mov x0, #STDIN
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, #BUF_LEN
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lstart_read_failed
    cbz x0, .Lstart_print
    cmp x0, #BUF_LEN
    b.hi .Lstart_assert_count
    mov x20, x0
    mov x1, x22
    bl count_pairs
    add x21, x21, x0
    mov x22, x1
    cmp x20, #BUF_LEN
    b.lo .Lstart_print
    subs x19, x19, #1
    b.ne .Lstart_full
.Lstart_print:
    mov x0, x21
    bl write_i64
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_read_failed:
    mov w0, #EXIT_READ_FAILED
    b .Lstart_exit
.Lstart_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_count:
    mov w0, #1
    bl assert_fail
.size _start, . - _start

// Counts the pairs of LETTER in the buffer's first bytes, each LETTER in at most one pair, taken
// from the left, with no branch on a byte: a LETTER after an unpaired one makes a pair.
// In: x0 = how many bytes, 1 to BUF_LEN; x1 = 1 when the byte before them is an unpaired
// LETTER, else 0.
// Out: x0 = the pairs; x1 = 1 when the last byte is an unpaired LETTER, else 0.
.type count_pairs, @function
count_pairs:
    cmp x0, #BUF_LEN
    b.hi .Lcount_pairs_assert_length
    cmp x1, #1
    b.hi .Lcount_pairs_assert_pending
    adrp x8, buffer
    add x8, x8, :lo12:buffer
    mov x7, x0
    mov x0, #0
    mov x2, #0
    mov x9, #BUF_LEN
.Lcount_pairs_loop:  // bound: 256
    cmp x2, x7
    b.hs .Lcount_pairs_return
    cmp x2, #BUF_LEN
    b.hs .Lcount_pairs_assert_index
    ldrb w11, [x8, x2]
    cmp w11, #LETTER
    cset x10, eq
    and x11, x10, x1
    add x0, x0, x11
    eor x11, x11, #1
    and x1, x10, x11
    add x2, x2, #1
    subs x9, x9, #1
    b.ne .Lcount_pairs_loop
.Lcount_pairs_return:
    ret
.Lcount_pairs_assert_length:
    mov w0, #2
    bl assert_fail
.Lcount_pairs_assert_pending:
    mov w0, #3
    bl assert_fail
.Lcount_pairs_assert_index:
    mov w0, #13
    bl assert_fail
.size count_pairs, . - count_pairs

// Writes a whole number in decimal, then a newline, to standard output.
// In: x0 = the number, never -9223372036854775808.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_i64, @function
write_i64:
    stp x29, x30, [sp, #-16]!
    mov x9, x0
    cmp x0, #0
    b.ge .Lwrite_i64_digits
    negs x0, x0
    b.vs .Lwrite_i64_assert_min
.Lwrite_i64_digits:
    adrp x1, out_buf
    add x1, x1, :lo12:out_buf
    mov w10, #NEWLINE
    strb w10, [x1, #OUT_BUF_LEN - 1]
    mov x2, #OUT_BUF_LEN - 1
    mov x11, #10
    mov x3, #MAX_DIGITS
.Lwrite_i64_digit:  // bound: 19
    udiv x12, x0, x11
    msub x13, x12, x11, x0
    add w13, w13, #'0'
    sub x2, x2, #1
    cmp x2, #OUT_BUF_LEN
    b.hs .Lwrite_i64_assert_digit_index
    strb w13, [x1, x2]
    mov x0, x12
    cbz x0, .Lwrite_i64_sign
    subs x3, x3, #1
    b.ne .Lwrite_i64_digit
    b .Lwrite_i64_assert_digits
.Lwrite_i64_sign:
    tbz x9, #63, .Lwrite_i64_output
    sub x2, x2, #1
    cmp x2, #OUT_BUF_LEN
    b.hs .Lwrite_i64_assert_sign_index
    mov w10, #'-'
    strb w10, [x1, x2]
.Lwrite_i64_output:
    add x1, x1, x2
    mov x10, #OUT_BUF_LEN
    sub x2, x10, x2
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lwrite_i64_return
    mov x0, #0
.Lwrite_i64_return:
    ldp x29, x30, [sp], #16
    ret
.Lwrite_i64_assert_min:
    mov w0, #6
    bl assert_fail
.Lwrite_i64_assert_digit_index:
    mov w0, #7
    bl assert_fail
.Lwrite_i64_assert_digits:
    mov w0, #8
    bl assert_fail
.Lwrite_i64_assert_sign_index:
    mov w0, #9
    bl assert_fail
.size write_i64, . - write_i64

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
    mov w0, #10
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #11
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #12
    bl assert_fail
.size write_all, . - write_all

// Reads from file descriptor x0 into the x2 bytes at x1 until they're full, the input ends,
// or, when w3 is a byte (0 to 255), the last byte read is w3; with w3 = -1 it reads to the
// end. However the input arrives, it reads the same.
// In: x0 = the file descriptor, x1 = the buffer, x2 = its size, at least 1, w3 = the end
// byte or -1.
// Out: x0 = how many bytes it read, or a negative error code.
// returns: result
.type fill, @function
fill:
    cbz x1, .Lfill_assert_null
    cbz x2, .Lfill_assert_empty
    mov x9, x0
    mov x10, x2
    mov w11, w3
    mov x12, #MAX_FILL_READS
.Lfill_loop:  // bound: 1099511627776
    mov x0, x9
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lfill_return
    cbz x0, .Lfill_done
    cmp x0, x2
    b.hi .Lfill_assert_count
    add x1, x1, x0
    subs x2, x2, x0
    b.eq .Lfill_done
    ldurb w13, [x1, #-1]
    cmp w13, w11
    b.eq .Lfill_done
    subs x12, x12, #1
    b.ne .Lfill_loop
    mov x0, #ERROR_IO
    b .Lfill_return
.Lfill_done:
    sub x0, x10, x2
.Lfill_return:
    ret
.Lfill_assert_null:
    mov w0, #990
    bl assert_fail
.Lfill_assert_empty:
    mov w0, #991
    bl assert_fail
.Lfill_assert_count:
    mov w0, #992
    bl assert_fail
.size fill, . - fill

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

