.arch armv8-a
.arch_extension sve

// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 2
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// fill's reads at most, and what makes it read to the end
.equ READ_TO_END, -1
// Bytes that matter
.equ NEWLINE, 10
.equ SPACE, 32
.equ TAB, 9
// Tab through carriage return are the 5 bytes from 9 to 13.
.equ TAB_TO_CR, 4
// A buffer-full of input, the most the main loop takes at a time, and its steps of a vector
// each: as many as the shortest vectors, 16 bytes, take
.equ BUF_LEN, 65536
.equ SHORTEST, 16
.equ STEPS, BUF_LEN / SHORTEST
// Room past a buffer-full for the longest vector the rules hold an access to
.equ VECTOR_ROOM, 64
.equ BUFFER_LEN, BUF_LEN + VECTOR_ROOM
// The buffer-fulls the main loop takes at most
.equ MAX_FULLS, 1099511627776
.equ MAX_DIGITS, 20
.equ MAX_WRITES, 64
.equ OUT_LEN, 42

.section .bss

// One buffer-full of standard input, with a vector's room after it.
.type buffer, @object
buffer:
    .zero BUFFER_LEN
.size buffer, . - buffer

// The line it prints: two numbers of up to 20 digits, a space and a newline.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
// Counts the lines and words of standard input of any length, a buffer-full at a time through
// fill, and prints both. What crosses from one buffer-full to the next is x23: 0xff when the
// last byte read is whitespace, or nothing has been read, else 0.
.type main, @function
main:
    mov x21, #0
    mov x22, #0
    mov x23, #0xff
    mov x19, #MAX_FULLS
.Lmain_full:  // bound: 1099511627776
    mov x0, #STDIN
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, #BUF_LEN
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lmain_failed
    cbz x0, .Lmain_print
    cmp x0, #BUF_LEN
    b.hi .Lmain_assert_count
    mov x20, x0
    mov x1, x0
    adrp x0, buffer
    add x0, x0, :lo12:buffer
    mov x2, x23
    bl count_piece
    add x21, x21, x0
    add x22, x22, x1
    mov x23, x2
    cmp x20, #BUF_LEN
    b.lo .Lmain_print
    subs x19, x19, #1
    b.ne .Lmain_full
    b .Lmain_assert_fulls
.Lmain_print:
    mov x0, x21
    mov x1, x22
    bl print_counts
    tbnz x0, #63, .Lmain_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_failed:
    mov w0, #EXIT_IO_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_count:
    mov w0, #1
    bl assert_fail
.Lmain_assert_fulls:
    mov w0, #2
    bl assert_fail
.size main, . - main

// Counts the newlines and word starts in a piece of the buffer, a vector a step with SVE: as
// many bytes a step as the CPU's vectors hold, 16 to 64. The loop leaves where its index
// reaches the bytes in the piece, and whilelo makes each step's lanes the bytes left, so the
// last partial step needs no code of its own, and no step's work depends on where the piece
// ends but what it counts; cntp counts a comparison's lanes.
// In: x0 = the bytes; x1 = how many, 1 to BUF_LEN; x2 = 0xff when the byte before them is
// whitespace (or there is none), else 0.
// Out: x0 = newlines; x1 = word starts; x2 = 0xff when the last byte is whitespace, else 0.
.type count_piece, @function
count_piece:
    cbz x0, .Lcount_piece_assert_bytes
    mov z18.b, #SPACE
    mov w12, w2
    mov x8, #0
    mov x9, #0
    mov x10, #0
    mov x11, #STEPS
.Lcount_piece_loop:  // bound: 4096
    cmp x10, x1
    b.hs .Lcount_piece_done
    cmp x10, #BUF_LEN
    b.hs .Lcount_piece_assert_index
    whilelo p0.b, x10, x1
    ld1b { z0.b }, p0/z, [x0, x10]
    cmpeq p1.b, p0/z, z0.b, #NEWLINE
    cntp x5, p0, p1.b
    add x8, x8, x5
    // Whitespace: a space, or a byte from tab to carriage return.
    cmpeq p2.b, p0/z, z0.b, z18.b
    cmphs p3.b, p0/z, z0.b, #TAB
    cmpls p4.b, p0/z, z0.b, #TAB + TAB_TO_CR
    and p3.b, p0/z, p3.b, p4.b
    orr p2.b, p0/z, p2.b, p3.b
    // A word starts at a byte that isn't whitespace where the one before is: each lane's
    // whitespace as 0xff, moved up a lane with the byte before the step's in lane 0.
    mov z1.b, p2/z, #-1
    lastb w13, p0, z1.b
    insr z1.b, w12
    cmpne p5.b, p0/z, z1.b, #0
    bic p6.b, p0/z, p5.b, p2.b
    cntp x6, p0, p6.b
    add x9, x9, x6
    mov w12, w13
    incb x10
    subs x11, x11, #1
    b.ne .Lcount_piece_loop
.Lcount_piece_done:
    mov x0, x8
    mov x1, x9
    mov w2, w12
    ret
.Lcount_piece_assert_bytes:
    mov w0, #3
    bl assert_fail
.Lcount_piece_assert_index:
    mov w0, #4
    bl assert_fail
.size count_piece, . - count_piece

// Prints two numbers, a space between them and a newline after, to standard output.
// In: x0 = the first; x1 = the second.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_counts, @function
print_counts:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    mov x19, x0
    adrp x20, out
    add x20, x20, :lo12:out
    mov w2, #NEWLINE
    strb w2, [x20, #OUT_LEN - 1]
    mov x0, x1
    add x1, x20, #OUT_LEN - 1
    bl digits
    cmp x0, x20
    b.ls .Lprint_counts_assert_second
    mov w2, #SPACE
    sub x0, x0, #1
    strb w2, [x0]
    mov x1, x0
    mov x0, x19
    bl digits
    cmp x0, x20
    b.lo .Lprint_counts_assert_first
    mov x1, x0
    add x2, x20, #OUT_LEN
    sub x2, x2, x0
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lprint_counts_return
    mov x0, #0
.Lprint_counts_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lprint_counts_assert_second:
    mov w0, #11
    bl assert_fail
.Lprint_counts_assert_first:
    mov w0, #12
    bl assert_fail
.size print_counts, . - print_counts

// Writes a whole number's decimal digits just before a place in memory.
// In: x0 = the number; x1 = where the digits end.
// Out: x0 = where they start.
.type digits, @function
digits:
    cbz x1, .Ldigits_assert_end
    mov x2, #10
    mov x3, #MAX_DIGITS
.Ldigits_loop:  // bound: 20
    udiv x4, x0, x2
    msub x5, x4, x2, x0
    mov x0, x4
    add w5, w5, #'0'
    sub x1, x1, #1
    strb w5, [x1]
    cbz x0, .Ldigits_done
    subs x3, x3, #1
    b.ne .Ldigits_loop
    b .Ldigits_assert_digits
.Ldigits_done:
    mov x0, x1
    ret
.Ldigits_assert_end:
    mov w0, #8
    bl assert_fail
.Ldigits_assert_digits:
    mov w0, #9
    bl assert_fail
.size digits, . - digits

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
    mov w0, #5
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #6
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #7
    bl assert_fail
.size write_all, . - write_all

