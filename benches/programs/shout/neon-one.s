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
// fill's end byte for none
.equ READ_TO_END, -1
// Sizes and limits: the input's most bytes, the buffer one more so a longer input shows
.equ INPUT_MAX, 1048576
.equ BUF_LEN, 1048577
.equ MAX_WRITES, 64
.equ TOO_LONG_MSG_LEN, 46
// The bytes a letter lies between, without a sign: 'a' less one and 'z' plus one; the bit that
// makes a capital; and the 16-byte steps, the last starting at most where 16 bytes still fit
.equ BELOW_A, 0x60
.equ ABOVE_Z, 0x7b
.equ CASE_BIT, 0x20
.equ CHUNK, 16
.equ CHUNKS, INPUT_MAX / CHUNK
.equ LAST_CHUNK, BUF_LEN - CHUNK + 1

.section .rodata

.type too_long_msg, @object
too_long_msg:
    .ascii "shout: the input is longer than 1048576 bytes\n"
.size too_long_msg, . - too_long_msg

.section .bss

// Standard input, all of it: one byte more than it may hold, so a longer input shows.
.type input, @object
input:
    .zero BUF_LEN
.size input, . - input

.text
// Reads standard input whole, capitalizes its ASCII letters 16 bytes a step, and prints it.
.type main, @function
main:
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, #BUF_LEN & 0xffff
    movk x2, #BUF_LEN >> 16, lsl #16
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lmain_failed
    cmp x0, #INPUT_MAX
    b.hi .Lmain_too_long
    mov x19, x0
    cbz x19, .Lmain_done
    adrp x1, input
    add x1, x1, :lo12:input
    bl shout
    mov x0, #STDOUT
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, x19
    bl write_all
    tbnz x0, #63, .Lmain_failed
.Lmain_done:
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_too_long:
    mov x0, #STDERR
    adrp x1, too_long_msg
    add x1, x1, :lo12:too_long_msg
    mov x2, #TOO_LONG_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lmain_failed
    mov w0, #EXIT_TOO_LONG
    b .Lmain_exit
.Lmain_failed:
    mov w0, #EXIT_IO_FAILED
.Lmain_exit:
    bl os_exit
.size main, . - main

// Turns every ASCII letter from a to z in the buffer's first x0 bytes into its capital, 16 bytes
// a step with NEON. The last step may change bytes past the text, which is harmless: only the
// text is written.
// In: x0 = how many bytes, at most INPUT_MAX; x1 = the buffer, of BUF_LEN bytes.
.type shout, @function
shout:
    cmp x0, #INPUT_MAX
    b.hi .Lshout_assert_length
    cbz x1, .Lshout_assert_buffer
    movi v1.16b, #BELOW_A
    movi v2.16b, #ABOVE_Z
    movi v3.16b, #CASE_BIT
    mov x9, #LAST_CHUNK & 0xffff
    movk x9, #LAST_CHUNK >> 16, lsl #16
    mov x2, #0
    mov x3, #CHUNKS
.Lshout_loop:  // bound: 65536
    cmp x2, x0
    b.hs .Lshout_done
    cmp x2, x9
    b.hs .Lshout_assert_index
    ldr q0, [x1, x2]
    cmhi v4.16b, v0.16b, v1.16b
    cmhi v5.16b, v2.16b, v0.16b
    and v4.16b, v4.16b, v5.16b
    and v4.16b, v4.16b, v3.16b
    sub v0.16b, v0.16b, v4.16b
    str q0, [x1, x2]
    add x2, x2, #CHUNK
    subs x3, x3, #1
    b.ne .Lshout_loop
.Lshout_done:
    ret
.Lshout_assert_length:
    mov w0, #20
    bl assert_fail
.Lshout_assert_index:
    mov w0, #21
    bl assert_fail
.Lshout_assert_buffer:
    mov w0, #22
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
    mov w0, #2
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #3
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #4
    bl assert_fail
.size write_all, . - write_all

