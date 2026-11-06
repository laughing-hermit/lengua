.arch armv8-a

// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_READ_FAILED, 3
.equ EXIT_WRITE_FAILED, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// fill's reads at most, and what makes it read to the end
.equ READ_TO_END, -1
// A buffer-full of input, the most the main loop takes at a time, and its 16-byte chunks
.equ BUF_LEN, 4096
.equ CHUNK, 16
.equ BUF_CHUNKS, BUF_LEN / CHUNK
// The buffer-fulls the main loop takes at most
.equ MAX_FULLS, 1099511627776
.equ MAX_WRITES, 64
// Bytes in every lane with movi: a letter is above BELOW_A and below ABOVE_Z
.equ BELOW_A, 0x60
.equ ABOVE_Z, 0x7b
.equ CASE_BIT, 0x20

.section .bss

// One buffer-full of standard input, capitalized in place, then written.
// shared: _start fills and writes it, and shout capitalizes it
.type buffer, @object
buffer:
    .zero BUF_LEN
.size buffer, . - buffer

.text
// Copies standard input of any length to standard output with every ASCII letter in capitals, a
// buffer-full at a time, until the input ends.
.type main, @function
main:
    mov x19, #MAX_FULLS
.Lmain_full:  // bound: 1099511627776
    mov x0, #STDIN
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, #BUF_LEN
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lmain_read_failed
    cbz x0, .Lmain_done
    cmp x0, #BUF_LEN
    b.hi .Lmain_assert_count
    mov x20, x0
    bl shout
    mov x0, #STDOUT
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, x20
    bl write_all
    tbnz x0, #63, .Lmain_write_failed
    cmp x20, #BUF_LEN
    b.lo .Lmain_done
    subs x19, x19, #1
    b.ne .Lmain_full
.Lmain_done:
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_read_failed:
    mov w0, #EXIT_READ_FAILED
    b .Lmain_exit
.Lmain_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_count:
    mov w0, #1
    bl assert_fail
.size main, . - main

// Capitalizes the first x0 bytes of buffer, 16 at a time: a to z become A to Z, other bytes stay.
// The last chunk may change bytes past them, which is harmless: only those bytes are written.
// In: x0 = how many, 1 to BUF_LEN.
.type shout, @function
shout:
    cmp x0, #BUF_LEN
    b.hi .Lshout_assert_length
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    movi v1.16b, #BELOW_A
    movi v2.16b, #ABOVE_Z
    movi v3.16b, #CASE_BIT
    mov x2, #0
    mov x3, #BUF_CHUNKS
.Lshout_loop:  // bound: 256
    cmp x2, x0
    b.hs .Lshout_return
    cmp x2, #BUF_LEN - CHUNK + 1
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
    mov w0, #2
    bl assert_fail
.Lshout_assert_index:
    mov w0, #3
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

