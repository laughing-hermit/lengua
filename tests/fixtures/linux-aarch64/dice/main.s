.arch armv8-a

// File descriptors
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_WRITE_FAILED, 2
.equ EXIT_NO_RANDOM, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// The random bytes drawn at once, and the bytes set aside so every face is equally likely
.equ DRAW_BYTES, 64
.equ SET_ASIDE_FROM, 252
.equ FACES, 6
.equ MAX_WRITES, 64
// The line it prints, "rolled ?\n": its length and where the roll goes
.equ LINE_LEN, 9
.equ ROLL_INDEX, 7

.section .data

// The line it prints, the roll put in at ROLL_INDEX.
.type line, @object
line:
    .ascii "rolled ?\n"
.size line, . - line

.section .bss

// The random bytes drawn.
.type random, @object
random:
    .zero DRAW_BYTES
.size random, . - random

.text
// Rolls a die with the first random byte below SET_ASIDE_FROM, its remainder after dividing by
// 6 plus 1, then prints the roll.
.type main, @function
main:
    bl draw
    tbnz x0, #63, .Lmain_no_random
    cmp w1, #SET_ASIDE_FROM
    b.hs .Lmain_assert_byte
    mov w2, #FACES
    udiv w3, w1, w2
    msub w4, w3, w2, w1
    cmp w4, #FACES
    b.hs .Lmain_assert_face
    add w4, w4, #'1'
    adrp x5, line
    add x5, x5, :lo12:line
    strb w4, [x5, #ROLL_INDEX]
    mov x0, #STDOUT
    mov x1, x5
    mov x2, #LINE_LEN
    bl write_all
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_no_random:
    mov w0, #EXIT_NO_RANDOM
    b .Lmain_exit
.Lmain_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_byte:
    mov w0, #5
    bl assert_fail
.Lmain_assert_face:
    mov w0, #6
    bl assert_fail
.size main, . - main

// Draws DRAW_BYTES random bytes and finds the first below SET_ASIDE_FROM.
// Out: x0 = 0, or a negative error code when the computer gives fewer bytes or sets every one
// aside; x1 = the byte found.
// returns: result
.type draw, @function
draw:
    stp x29, x30, [sp, #-16]!
    adrp x0, random
    add x0, x0, :lo12:random
    mov x1, #DRAW_BYTES
    bl os_random
    tbnz x0, #63, .Ldraw_failed
    cmp x0, #DRAW_BYTES
    b.ne .Ldraw_failed
    adrp x3, random
    add x3, x3, :lo12:random
    mov x4, #0
    mov x5, #DRAW_BYTES
.Ldraw_loop:  // bound: 64
    cmp x4, #DRAW_BYTES
    b.hs .Ldraw_assert_index
    ldrb w1, [x3, x4]
    cmp w1, #SET_ASIDE_FROM
    b.lo .Ldraw_found
    add x4, x4, #1
    subs x5, x5, #1
    b.ne .Ldraw_loop
.Ldraw_failed:
    mov x0, #ERROR_IO
    b .Ldraw_return
.Ldraw_found:
    mov x0, #0
.Ldraw_return:
    ldp x29, x30, [sp], #16
    ret
.Ldraw_assert_index:
    mov w0, #1
    bl assert_fail
.size draw, . - draw

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
    mov w0, #10
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #11
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #12
    bl assert_fail
.size write_all, . - write_all

