.arch armv8-a

// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
.equ STDERR, 2
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_NO_INPUT, 1
.equ EXIT_TOO_LONG, 2
.equ EXIT_READ_FAILED, 3
.equ EXIT_WRITE_FAILED, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_TOO_LONG, -7
// What find_newline returns when there's no newline
.equ NO_NEWLINE, -1
// Sizes and limits
.equ CHUNK, 16
.equ MAX_LINE, 64
.equ READ_MAX, MAX_LINE + 1
.equ MAX_READS, READ_MAX
.equ LINE_CHUNKS, 5
.equ LINE_BUF_LEN, LINE_CHUNKS * CHUNK
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ NO_INPUT_MSG_LEN, 16
.equ TOO_LONG_MSG_LEN, 21
// The bytes a letter lies between, without a sign: 'a' less one and 'z' plus one; and the bit
// that makes a capital
.equ BELOW_A, 0x60
.equ ABOVE_Z, 0x7b
.equ CASE_BIT, 0x20

.section .rodata

.type no_input_msg, @object
no_input_msg:
    .ascii "error: no input\n"
.size no_input_msg, . - no_input_msg

.type too_long_msg, @object
too_long_msg:
    .ascii "error: line too long\n"
.size too_long_msg, . - too_long_msg

.section .bss

// READ_MAX bytes of input, rounded up to whole 16-byte chunks so vector loads stay inside.
// shared: read_line fills it, find_newline and shout work on it, and _start prints it
.type line_buf, @object
line_buf:
    .zero LINE_BUF_LEN
.size line_buf, . - line_buf

.text
// Reads a line, prints it in capital letters and exits.
.type main, @function
main:
    bl read_line
    tbnz x0, #63, .Lmain_read_failed
    cbz x0, .Lmain_no_input
    cmp x0, #MAX_LINE
    b.hi .Lmain_assert_length
    mov x19, x0
    bl shout
    adrp x1, line_buf
    add x1, x1, :lo12:line_buf
    cmp x19, #LINE_BUF_LEN
    b.hs .Lmain_assert_index
    mov w2, #NEWLINE
    strb w2, [x1, x19]
    mov x0, #STDOUT
    add x2, x19, #1
    bl write_all
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_read_failed:
    cmp x0, #ERROR_TOO_LONG
    b.eq .Lmain_too_long
    mov w0, #EXIT_READ_FAILED
    b .Lmain_exit
.Lmain_too_long:
    mov x0, #STDERR
    adrp x1, too_long_msg
    add x1, x1, :lo12:too_long_msg
    mov x2, #TOO_LONG_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_TOO_LONG
    b .Lmain_exit
.Lmain_no_input:
    mov x0, #STDERR
    adrp x1, no_input_msg
    add x1, x1, :lo12:no_input_msg
    mov x2, #NO_INPUT_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_NO_INPUT
    b .Lmain_exit
.Lmain_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_length:
    mov w0, #1
    bl assert_fail
.Lmain_assert_index:
    mov w0, #2
    bl assert_fail
.size main, . - main

// Reads standard input into line_buf until a newline arrives, the input ends or READ_MAX bytes
// are in.
// Out: x0 = the line's length without its newline, ERROR_TOO_LONG when the line is longer than
// MAX_LINE, or ERROR_IO when reading fails.
// returns: result
.type read_line, @function
read_line:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    mov x19, #0
    mov x20, #MAX_READS
.Lread_line_loop:  // bound: 65
    cmp x19, #READ_MAX
    b.hs .Lread_line_assert_full
    mov x0, #STDIN
    adrp x1, line_buf
    add x1, x1, :lo12:line_buf
    add x1, x1, x19
    mov x2, #READ_MAX
    sub x2, x2, x19
    bl os_read
    tbnz x0, #63, .Lread_line_failed
    cbz x0, .Lread_line_ended
    add x19, x19, x0
    bl find_newline
    tbz x0, #63, .Lread_line_found
    cmp x19, #READ_MAX
    b.hs .Lread_line_too_long
    subs x20, x20, #1
    b.ne .Lread_line_loop
    b .Lread_line_assert_reads
.Lread_line_ended:
    cmp x19, #MAX_LINE
    b.hi .Lread_line_too_long
    mov x0, x19
    b .Lread_line_return
.Lread_line_found:
    cmp x0, #MAX_LINE
    b.hi .Lread_line_too_long
    b .Lread_line_return
.Lread_line_too_long:
    mov x0, #ERROR_TOO_LONG
    b .Lread_line_return
.Lread_line_failed:
    mov x0, #ERROR_IO
.Lread_line_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lread_line_assert_full:
    mov w0, #3
    bl assert_fail
.Lread_line_assert_reads:
    mov w0, #4
    bl assert_fail
.size read_line, . - read_line

// Finds the first newline in line_buf, 16 bytes at a time. Bytes not read yet are zero. NEON
// has no byte mask in one instruction: shrn by 4 packs each byte's comparison into 4 bits of a
// doubleword, and the lowest set bit, by rbit and clz, is 4 times the byte's place.
// Out: x0 = its index, or NO_NEWLINE when there's none.
// returns: result
.type find_newline, @function
find_newline:
    adrp x1, line_buf
    add x1, x1, :lo12:line_buf
    movi v1.16b, #NEWLINE
    mov x2, #0
    mov x3, #LINE_CHUNKS
.Lfind_newline_loop:  // bound: 5
    cmp x2, #LINE_BUF_LEN - CHUNK + 1
    b.hs .Lfind_newline_assert_index
    ldr q0, [x1, x2]
    cmeq v0.16b, v0.16b, v1.16b
    shrn v0.8b, v0.8h, #4
    fmov x0, d0
    cbnz x0, .Lfind_newline_found
    add x2, x2, #CHUNK
    subs x3, x3, #1
    b.ne .Lfind_newline_loop
    mov x0, #NO_NEWLINE
    b .Lfind_newline_return
.Lfind_newline_found:
    rbit x0, x0
    clz x0, x0
    add x0, x2, x0, lsr #2
    cmp x0, #LINE_BUF_LEN
    b.hs .Lfind_newline_assert_found
.Lfind_newline_return:
    ret
.Lfind_newline_assert_index:
    mov w0, #5
    bl assert_fail
.Lfind_newline_assert_found:
    mov w0, #6
    bl assert_fail
.size find_newline, . - find_newline

// Capitalizes the line in line_buf, 16 bytes at a time: a to z become A to Z, other bytes stay.
// The last chunk may change bytes past the line, which is harmless: only the line is printed.
// In: x0 = the line's length, at most MAX_LINE.
.type shout, @function
shout:
    cmp x0, #MAX_LINE
    b.hi .Lshout_assert_length
    adrp x1, line_buf
    add x1, x1, :lo12:line_buf
    movi v1.16b, #BELOW_A
    movi v2.16b, #ABOVE_Z
    movi v3.16b, #CASE_BIT
    mov x2, #0
    mov x3, #LINE_CHUNKS
.Lshout_loop:  // bound: 5
    cmp x2, x0
    b.hs .Lshout_return
    cmp x2, #LINE_BUF_LEN - CHUNK + 1
    b.hs .Lshout_assert_index
    ldr q0, [x1, x2]
    // A letter is above 'a' less one and below 'z' plus one, without a sign.
    cmhi v4.16b, v0.16b, v1.16b
    cmhi v5.16b, v2.16b, v0.16b
    and v4.16b, v4.16b, v5.16b
    and v4.16b, v4.16b, v3.16b
    sub v0.16b, v0.16b, v4.16b
    str q0, [x1, x2]
    add x2, x2, #CHUNK
    subs x3, x3, #1
    b.ne .Lshout_loop
.Lshout_return:
    ret
.Lshout_assert_length:
    mov w0, #7
    bl assert_fail
.Lshout_assert_index:
    mov w0, #8
    bl assert_fail
.size shout, . - shout

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
    mov w0, #9
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #10
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #11
    bl assert_fail
.size write_all, . - write_all

