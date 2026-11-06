.arch armv8-a

// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 2
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// Eight tables of 256 counts each, eight bytes a step, the k-th byte of each step into table k,
// so the bytes of a step never wait on one counter. Each table starts 2048 + 72 bytes after the
// one before, so a value's eight counts never share their place in a 4 KiB page.
.equ VALUES, 256
.equ TABLE_STRIDE, 2120
.equ COUNTS_LEN, 16960
.equ EIGHT, 8
// Sizes and limits: a read's bytes at most, with a step's room past the last, so a step's
// check is one on its start
.equ READ_LEN, 1048576
.equ INPUT_LEN, READ_LEN + EIGHT
.equ EIGHTS, READ_LEN / EIGHT
.equ MAX_READS, 1099511627776
.equ MAX_DIGITS, 20
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ SPACE, 32
// At most 256 lines of a 3-digit value, a space, a 20-digit count and a newline.
.equ OUT_LEN, 6400

.section .bss

// Standard input as it comes.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

// The eight tables of counts, one after another.
.type counts, @object
counts:
    .zero COUNTS_LEN
.size counts, . - counts

// Every line it prints, built from the end.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
// Counts each byte value of standard input, a read at a time, then prints the values that occur.
.type main, @function
main:
    mov x19, #MAX_READS
.Lmain_read:  // bound: 1099511627776
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, #READ_LEN
    bl os_read
    tbnz x0, #63, .Lmain_failed
    cbz x0, .Lmain_print
    cmp x0, #READ_LEN
    b.hi .Lmain_assert_count
    mov x20, x0
    mov x1, x0
    adrp x0, input
    add x0, x0, :lo12:input
    adrp x2, counts
    add x2, x2, :lo12:counts
    bl count_bytes
    // The last bytes, fewer than eight, into the first table.
    sub x1, x20, x0
    adrp x9, input
    add x9, x9, :lo12:input
    add x0, x9, x0
    adrp x2, counts
    add x2, x2, :lo12:counts
    bl count_rest
    subs x19, x19, #1
    b.ne .Lmain_read
    b .Lmain_assert_reads
.Lmain_print:
    adrp x0, counts
    add x0, x0, :lo12:counts
    bl print_histogram
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
.Lmain_assert_reads:
    mov w0, #2
    bl assert_fail
.size main, . - main

// Adds each byte to the count of its value, eight at a step, the k-th byte of each step into
// table k, so neighboring bytes never wait on one counter. Four bytes at a time, their counts'
// addresses come first, then their loads, adds and stores together, each table reached from x2
// by its offset: so the core has four counts in flight, where one at a time stalled it, as the
// CPU's own counters showed (todo.md). It stops before the last bytes, fewer than eight, for
// count_rest.
// In: x0 = the bytes; x1 = how many, up to READ_LEN; x2 = the eight tables, COUNTS_LEN bytes.
// Out: x0 = how many it added, a multiple of eight.
.type count_bytes, @function
count_bytes:
    cmp x1, #READ_LEN
    b.hi .Lcount_bytes_assert_length
    // The last place an eight starts, below 0 when there's none.
    sub x17, x1, #EIGHT
    mov x9, #0
    mov x11, #EIGHTS
.Lcount_bytes_eights:  // bound: 131072
    cmp x9, x17
    b.gt .Lcount_bytes_done
    cmp x9, #READ_LEN
    b.hs .Lcount_bytes_assert_index
    ldr x14, [x0, x9]
    // unsafe: each index is one byte, 0 to 255, and each table holds 256 counts
    and x3, x14, #0xff
    ubfx x4, x14, #8, #8
    ubfx x5, x14, #16, #8
    ubfx x6, x14, #24, #8
    add x3, x2, x3, lsl #3
    add x4, x2, x4, lsl #3
    add x5, x2, x5, lsl #3
    add x6, x2, x6, lsl #3
    ldr x7, [x3]
    ldr x8, [x4, #TABLE_STRIDE]
    ldr x10, [x5, #2 * TABLE_STRIDE]
    ldr x12, [x6, #3 * TABLE_STRIDE]
    add x7, x7, #1
    add x8, x8, #1
    add x10, x10, #1
    add x12, x12, #1
    str x7, [x3]
    str x8, [x4, #TABLE_STRIDE]
    str x10, [x5, #2 * TABLE_STRIDE]
    str x12, [x6, #3 * TABLE_STRIDE]
    ubfx x3, x14, #32, #8
    ubfx x4, x14, #40, #8
    ubfx x5, x14, #48, #8
    lsr x6, x14, #56
    add x3, x2, x3, lsl #3
    add x4, x2, x4, lsl #3
    add x5, x2, x5, lsl #3
    add x6, x2, x6, lsl #3
    ldr x7, [x3, #4 * TABLE_STRIDE]
    ldr x8, [x4, #5 * TABLE_STRIDE]
    ldr x10, [x5, #6 * TABLE_STRIDE]
    ldr x12, [x6, #7 * TABLE_STRIDE]
    add x7, x7, #1
    add x8, x8, #1
    add x10, x10, #1
    add x12, x12, #1
    str x7, [x3, #4 * TABLE_STRIDE]
    str x8, [x4, #5 * TABLE_STRIDE]
    str x10, [x5, #6 * TABLE_STRIDE]
    str x12, [x6, #7 * TABLE_STRIDE]
    // end unsafe
    add x9, x9, #EIGHT
    subs x11, x11, #1
    b.ne .Lcount_bytes_eights
.Lcount_bytes_done:
    mov x0, x9
    ret
.Lcount_bytes_assert_length:
    mov w0, #8
    bl assert_fail
.Lcount_bytes_assert_index:
    mov w0, #9
    bl assert_fail
.size count_bytes, . - count_bytes

// Adds the last bytes, fewer than eight, to the counts of their values in the first table.
// In: x0 = the bytes; x1 = how many, 0 to 7; x2 = the eight tables, COUNTS_LEN bytes.
.type count_rest, @function
count_rest:
    cbz x2, .Lcount_rest_assert_counts
    cmp x1, #EIGHT
    b.hs .Lcount_rest_assert_length
    mov x9, #0
    mov x11, #EIGHT
.Lcount_rest_loop:  // bound: 8
    cmp x9, x1
    b.hs .Lcount_rest_done
    cmp x9, #EIGHT
    b.hs .Lcount_rest_assert_index
    ldrb w15, [x0, x9]
    // unsafe: the index is one byte, 0 to 255, and the table holds 256 counts
    ldr x16, [x2, x15, lsl #3]
    add x16, x16, #1
    str x16, [x2, x15, lsl #3]
    // end unsafe
    add x9, x9, #1
    subs x11, x11, #1
    b.ne .Lcount_rest_loop
.Lcount_rest_done:
    ret
.Lcount_rest_assert_counts:
    mov w0, #11
    bl assert_fail
.Lcount_rest_assert_length:
    mov w0, #14
    bl assert_fail
.Lcount_rest_assert_index:
    mov w0, #15
    bl assert_fail
.size count_rest, . - count_rest

// Prints a line for each byte value that occurs, smallest first: the value, a space, the count.
// In: x0 = the eight tables of counts.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_histogram, @function
print_histogram:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x0, .Lprint_histogram_assert_counts
    mov x19, x0
    adrp x20, out
    add x20, x20, :lo12:out
    mov x9, #OUT_LEN
    add x20, x20, x9
    mov x21, x20
    mov x22, #VALUES - 1
    mov x23, #VALUES
.Lprint_histogram_loop:  // bound: 256
    cmp x22, #VALUES
    b.hs .Lprint_histogram_assert_value
    mov x0, x19
    mov x1, x22
    bl count_of
    cbz x0, .Lprint_histogram_next
    mov w11, #NEWLINE
    strb w11, [x21, #-1]!
    mov x1, x21
    bl digits
    mov w11, #SPACE
    strb w11, [x0, #-1]!
    mov x1, x0
    mov x0, x22
    bl digits
    mov x21, x0
.Lprint_histogram_next:
    sub x22, x22, #1
    subs x23, x23, #1
    b.ne .Lprint_histogram_loop
    subs x2, x20, x21
    b.eq .Lprint_histogram_empty
    mov x1, x21
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lprint_histogram_return
.Lprint_histogram_empty:
    mov x0, #0
.Lprint_histogram_return:
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lprint_histogram_assert_counts:
    mov w0, #12
    bl assert_fail
.Lprint_histogram_assert_value:
    mov w0, #13
    bl assert_fail
.size print_histogram, . - print_histogram

// The count of one byte value: its counts in the eight tables, added.
// In: x0 = the eight tables, COUNTS_LEN bytes; x1 = the value, 0 to 255.
// Out: x0 = its count.
.type count_of, @function
count_of:
    cbz x0, .Lcount_of_assert_counts
    cmp x1, #VALUES
    b.hs .Lcount_of_assert_value
    add x9, x0, x1, lsl #3
    ldr x0, [x9]
    ldr x10, [x9, #TABLE_STRIDE]
    add x0, x0, x10
    ldr x10, [x9, #2 * TABLE_STRIDE]
    add x0, x0, x10
    ldr x10, [x9, #3 * TABLE_STRIDE]
    add x0, x0, x10
    ldr x10, [x9, #4 * TABLE_STRIDE]
    add x0, x0, x10
    ldr x10, [x9, #5 * TABLE_STRIDE]
    add x0, x0, x10
    ldr x10, [x9, #6 * TABLE_STRIDE]
    add x0, x0, x10
    ldr x10, [x9, #7 * TABLE_STRIDE]
    add x0, x0, x10
    ret
.Lcount_of_assert_counts:
    mov w0, #16
    bl assert_fail
.Lcount_of_assert_value:
    mov w0, #17
    bl assert_fail
.size count_of, . - count_of

// Writes a whole number's decimal digits just before a place in memory.
// In: x0 = the number; x1 = where the digits end.
// Out: x0 = where they start.
.type digits, @function
digits:
    cbz x1, .Ldigits_assert_end
    mov x9, x0
    mov x10, #10
    mov x11, #MAX_DIGITS
.Ldigits_loop:  // bound: 20
    udiv x12, x9, x10
    msub x13, x12, x10, x9
    add w13, w13, #48
    strb w13, [x1, #-1]!
    mov x9, x12
    cbz x9, .Ldigits_done
    subs x11, x11, #1
    b.ne .Ldigits_loop
    b .Ldigits_assert_digits
.Ldigits_done:
    mov x0, x1
    ret
.Ldigits_assert_end:
    mov w0, #20
    bl assert_fail
.Ldigits_assert_digits:
    mov w0, #21
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
    mov w0, #30
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #31
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #32
    bl assert_fail
.size write_all, . - write_all

