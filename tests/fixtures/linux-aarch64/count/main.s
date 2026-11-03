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
.equ EXIT_USAGE, 1
.equ EXIT_READ_FAILED, 2
.equ EXIT_WRITE_FAILED, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// Bytes that matter
.equ NEWLINE, 10
.equ SPACE, 32
.equ TAB, 9
// Tab through carriage return are the 5 bytes from 9 to 13.
.equ TAB_TO_CR, 4
.equ ARGC_EXPECTED, 1
// A read's bytes at most, and its 16-byte steps
.equ BUF_LEN, 65536
.equ VECTOR, 16
.equ STEPS, BUF_LEN / VECTOR
// Where the last step may start, and one past it
.equ STEP_LIMIT, BUF_LEN - VECTOR + 1
// The reads the main loop takes at most
.equ MAX_READS, 1099511627776
.equ MAX_DIGITS, 20
.equ MAX_WRITES, 64
.equ OUT_LEN, 42
.equ USAGE_MSG_LEN, 21

.section .rodata

// Each lane's place in a vector, 0 to 15: a lane is a byte of the input where its place is
// below the bytes left.
.type places, @object
places:
    .byte 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15
.size places, . - places

.type usage_msg, @object
usage_msg:
    .ascii "usage: count < input\n"
.size usage_msg, . - usage_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// One read's worth of standard input.
.type input, @object
input:
    .zero BUF_LEN
.size input, . - input

// The line it prints: two numbers of up to 20 digits, a space and a newline.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
.globl _start

// Counts the lines and words of standard input of any length, a read at a time, and prints both.
// What crosses from one read to the next is x23: 0xff when the last byte read is whitespace, or
// nothing has been read, else 0.
.type _start, @function
_start:
    ldr x0, [sp]
    cmp x0, #ARGC_EXPECTED
    b.ne .Lstart_usage
    mov x21, #0
    mov x22, #0
    mov x23, #0xff
    mov x19, #MAX_READS
.Lstart_read:  // bound: 1099511627776
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, #BUF_LEN
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lstart_read_failed
    cbz x0, .Lstart_print
    cmp x0, #BUF_LEN
    b.hi .Lstart_assert_count
    mov x1, x0
    adrp x0, input
    add x0, x0, :lo12:input
    mov x2, x23
    bl count_piece
    add x21, x21, x0
    add x22, x22, x1
    mov x23, x2
    subs x19, x19, #1
    b.ne .Lstart_read
    b .Lstart_assert_reads
.Lstart_print:
    mov x0, x21
    mov x1, x22
    bl print_counts
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_usage:
    mov x0, #STDERR
    adrp x1, usage_msg
    add x1, x1, :lo12:usage_msg
    mov x2, #USAGE_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_USAGE
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
.Lstart_assert_reads:
    mov w0, #2
    bl assert_fail
.size _start, . - _start

// Counts the newlines and word starts in a piece of the buffer, 16 bytes a step with NEON.
// Each step's bytes past the end are masked off, lanes whose place isn't below the bytes left,
// so the last partial step needs no code of its own, and no step's work depends on where the
// piece ends but what it counts. A comparison's lanes, all ones or zero, become 1 or 0 with
// ushr by 7, and addv adds them.
// In: x0 = the bytes; x1 = how many, 1 to BUF_LEN; x2 = 0xff when the byte before them is
// whitespace (or there is none), else 0.
// Out: x0 = newlines; x1 = word starts; x2 = 0xff when the last byte is whitespace, else 0.
.type count_piece, @function
count_piece:
    cbz x0, .Lcount_piece_assert_bytes
    adrp x3, places
    add x3, x3, :lo12:places
    ldr q16, [x3]
    movi v17.16b, #NEWLINE
    movi v18.16b, #SPACE
    movi v19.16b, #TAB
    movi v20.16b, #TAB_TO_CR
    // The byte before the piece's whitespace, in every lane: only the last is read.
    dup v21.16b, w2
    mov x8, #0
    mov x9, #0
    mov x10, #0
    mov x12, #STEP_LIMIT
    mov x11, #STEPS
.Lcount_piece_loop:  // bound: 4096
    cmp x10, x1
    b.hs .Lcount_piece_done
    cmp x10, x12
    b.hs .Lcount_piece_assert_index
    // This step's bytes before the end, min(x1 - x10, 16), and the lanes that hold them.
    sub x4, x1, x10
    mov x5, #VECTOR
    cmp x4, x5
    csel x4, x4, x5, lo
    dup v22.16b, w4
    cmhi v22.16b, v22.16b, v16.16b
    ldr q0, [x0, x10]
    cmeq v1.16b, v0.16b, v17.16b
    and v1.16b, v1.16b, v22.16b
    ushr v1.16b, v1.16b, #7
    addv b1, v1.16b
    umov w6, v1.b[0]
    add x8, x8, x6
    // Whitespace: a space, or a byte from tab to carriage return.
    cmeq v2.16b, v0.16b, v18.16b
    sub v3.16b, v0.16b, v19.16b
    cmhs v3.16b, v20.16b, v3.16b
    orr v2.16b, v2.16b, v3.16b
    // A word starts at a byte that isn't whitespace where the one before is.
    ext v3.16b, v21.16b, v2.16b, #15
    bic v3.16b, v3.16b, v2.16b
    and v3.16b, v3.16b, v22.16b
    ushr v3.16b, v3.16b, #7
    addv b3, v3.16b
    umov w6, v3.b[0]
    add x9, x9, x6
    mov v21.16b, v2.16b
    add x10, x10, #VECTOR
    subs x11, x11, #1
    b.ne .Lcount_piece_loop
.Lcount_piece_done:
    // Whether the piece's last byte is whitespace, for the next piece: the last step's lane
    // for its last byte.
    sub w5, w4, #1
    dup v4.16b, w5
    tbl v4.16b, { v21.16b }, v4.16b
    umov w2, v4.b[0]
    mov x0, x8
    mov x1, x9
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
    mov w0, #5
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #6
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #7
    bl assert_fail
.size write_all, . - write_all

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

