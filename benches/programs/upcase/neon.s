.arch armv8-a

// File descriptors, and os_open's modes: reading, and a new or emptied file to write
.equ STDERR, 2
.equ OPEN_READ, 0
.equ OPEN_WRITE, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_CANNOT_OPEN, 2
.equ EXIT_TOO_LARGE, 3
.equ EXIT_IO_FAILED, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_CANNOT_OPEN, -2
.equ ERROR_TOO_LARGE, -3
.equ ERROR_IO, -5
// fill's end byte for none
.equ READ_TO_END, -1
// Sizes and limits: the input's most bytes, the buffer one more so a larger file shows, the
// letters a to z, and the longest argument the kernel passes
.equ ARGC_EXPECTED, 3
.equ MAX_INPUT, 1048576
.equ BUF_LEN, 1048577
// The bytes a letter lies between, without a sign: 'a' less one and 'z' plus one; the bit that
// makes a capital; and the 16-byte steps, the last starting at most where 16 bytes still fit
.equ BELOW_A, 0x60
.equ ABOVE_Z, 0x7b
.equ CASE_BIT, 0x20
.equ CHUNK, 16
.equ CHUNKS, MAX_INPUT / CHUNK
.equ LAST_CHUNK, BUF_LEN - CHUNK + 1
.equ MAX_WRITES, 64
.equ MAX_ARG_LEN, 131072
.equ USAGE_MSG_LEN, 31
.equ OPEN_PREFIX_LEN, 20
.equ NEWLINE_MSG_LEN, 1
.equ LARGE_PREFIX_LEN, 8
.equ LARGE_SUFFIX_LEN, 30

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: upcase <input> <output>\n"
.size usage_msg, . - usage_msg

.type open_prefix, @object
open_prefix:
    .ascii "upcase: cannot open "
.size open_prefix, . - open_prefix

.type newline_msg, @object
newline_msg:
    .ascii "\n"
.size newline_msg, . - newline_msg

.type large_prefix, @object
large_prefix:
    .ascii "upcase: "
.size large_prefix, . - large_prefix

.type large_suffix, @object
large_suffix:
    .ascii " is larger than 1048576 bytes\n"
.size large_suffix, . - large_suffix

.section .bss

// The input file's bytes: one more than it may hold, so a larger file shows.
.type input, @object
input:
    .zero BUF_LEN
.size input, . - input

.text
// Writes the file named first to the file named second with its ASCII letters in capitals.
.type main, @function
main:
    cmp x0, #ARGC_EXPECTED
    b.ne .Lmain_usage
    ldr x19, [x1, #8]
    ldr x20, [x1, #16]
    cbz x19, .Lmain_assert_input
    cbz x20, .Lmain_assert_output
    adrp x22, input
    add x22, x22, :lo12:input
    mov x0, x19
    mov x1, x22
    bl load_input
    tbnz x0, #63, .Lmain_load_failed
    mov x21, x0
    mov x1, x22
    bl upcase_input
    mov x0, x20
    mov x1, x22
    mov x2, x21
    bl write_output
    tbnz x0, #63, .Lmain_io_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_load_failed:
    cmp x0, #ERROR_CANNOT_OPEN
    b.eq .Lmain_cannot_open
    cmp x0, #ERROR_TOO_LARGE
    b.ne .Lmain_io_failed
    mov x0, x19
    bl report_too_large
    tbnz x0, #63, .Lmain_io_failed
    mov w0, #EXIT_TOO_LARGE
    b .Lmain_exit
.Lmain_cannot_open:
    mov x0, x19
    bl report_cannot_open
    tbnz x0, #63, .Lmain_io_failed
    mov w0, #EXIT_CANNOT_OPEN
    b .Lmain_exit
.Lmain_usage:
    mov x0, #STDERR
    adrp x1, usage_msg
    add x1, x1, :lo12:usage_msg
    mov x2, #USAGE_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lmain_io_failed
    mov w0, #EXIT_USAGE
    b .Lmain_exit
.Lmain_io_failed:
    mov w0, #EXIT_IO_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_input:
    mov w0, #1
    bl assert_fail
.Lmain_assert_output:
    mov w0, #2
    bl assert_fail
.size main, . - main

// Reads the file named x0, all of it, into the buffer at x1, of BUF_LEN bytes.
// In: x0 = the file's name; x1 = the buffer.
// Out: x0 = how many bytes it holds, at most MAX_INPUT; ERROR_CANNOT_OPEN where it can't be
// opened, ERROR_TOO_LARGE where it holds more, or ERROR_IO where reading it fails.
// returns: result
.type load_input, @function
load_input:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    cbz x0, .Lload_input_assert_name
    cbz x1, .Lload_input_assert_buffer
    mov x20, x1
    mov x1, #OPEN_READ
    bl os_open
    tbnz x0, #63, .Lload_input_cannot_open
    mov x19, x0
    mov x1, x20
    mov x2, #BUF_LEN & 0xffff
    movk x2, #BUF_LEN >> 16, lsl #16
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lload_input_failed
    mov x20, x0
    mov x0, x19
    bl os_close
    tbnz x0, #63, .Lload_input_failed
    cmp x20, #MAX_INPUT
    b.hi .Lload_input_too_large
    mov x0, x20
    b .Lload_input_return
.Lload_input_cannot_open:
    mov x0, #ERROR_CANNOT_OPEN
    b .Lload_input_return
.Lload_input_too_large:
    mov x0, #ERROR_TOO_LARGE
    b .Lload_input_return
.Lload_input_failed:
    mov x0, #ERROR_IO
.Lload_input_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lload_input_assert_name:
    mov w0, #20
    bl assert_fail
.Lload_input_assert_buffer:
    mov w0, #23
    bl assert_fail
.size load_input, . - load_input

// Turns every ASCII letter from a to z in the buffer's first x0 bytes into its capital, 16 bytes
// a step with NEON: a byte above 'a' less one and below 'z' plus one, without a sign, is a
// letter, and its capital is 32 less. The last step may change bytes past the text, which is
// harmless: only the text is written.
// In: x0 = how many bytes, at most MAX_INPUT; x1 = the buffer, of BUF_LEN bytes.
.type upcase_input, @function
upcase_input:
    cmp x0, #MAX_INPUT
    b.hi .Lupcase_input_assert_length
    cbz x1, .Lupcase_input_assert_buffer
    movi v1.16b, #BELOW_A
    movi v2.16b, #ABOVE_Z
    movi v3.16b, #CASE_BIT
    mov x9, #LAST_CHUNK & 0xffff
    movk x9, #LAST_CHUNK >> 16, lsl #16
    mov x2, #0
    mov x3, #CHUNKS
.Lupcase_input_loop:  // bound: 65536
    cmp x2, x0
    b.hs .Lupcase_input_done
    cmp x2, x9
    b.hs .Lupcase_input_assert_index
    ldr q0, [x1, x2]
    cmhi v4.16b, v0.16b, v1.16b
    cmhi v5.16b, v2.16b, v0.16b
    and v4.16b, v4.16b, v5.16b
    and v4.16b, v4.16b, v3.16b
    sub v0.16b, v0.16b, v4.16b
    str q0, [x1, x2]
    add x2, x2, #CHUNK
    subs x3, x3, #1
    b.ne .Lupcase_input_loop
.Lupcase_input_done:
    ret
.Lupcase_input_assert_length:
    mov w0, #21
    bl assert_fail
.Lupcase_input_assert_index:
    mov w0, #22
    bl assert_fail
.Lupcase_input_assert_buffer:
    mov w0, #24
    bl assert_fail
.size upcase_input, . - upcase_input

// Writes the first x2 bytes at x1 to the file named x0, made or emptied.
// In: x0 = the file's name; x1 = the bytes; x2 = how many, at most MAX_INPUT.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_output, @function
write_output:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Lwrite_output_assert_name
    cbz x1, .Lwrite_output_assert_bytes
    cmp x2, #MAX_INPUT
    b.hi .Lwrite_output_assert_length
    mov x21, x1
    mov x20, x2
    mov x1, #OPEN_WRITE
    bl os_open
    tbnz x0, #63, .Lwrite_output_failed
    mov x19, x0
    cbz x20, .Lwrite_output_close
    mov x0, x19
    mov x1, x21
    mov x2, x20
    bl write_all
    tbnz x0, #63, .Lwrite_output_failed
.Lwrite_output_close:
    mov x0, x19
    bl os_close
    tbnz x0, #63, .Lwrite_output_failed
    mov x0, #0
    b .Lwrite_output_return
.Lwrite_output_failed:
    mov x0, #ERROR_IO
.Lwrite_output_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lwrite_output_assert_name:
    mov w0, #30
    bl assert_fail
.Lwrite_output_assert_bytes:
    mov w0, #31
    bl assert_fail
.Lwrite_output_assert_length:
    mov w0, #32
    bl assert_fail
.size write_output, . - write_output

// Says on standard error that the file named x0 can't be opened.
// In: x0 = its name.
// Out: x0 = 0, or a negative error code.
// returns: result
.type report_cannot_open, @function
report_cannot_open:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lreport_cannot_open_assert_name
    mov x2, x0
    adrp x0, open_prefix
    add x0, x0, :lo12:open_prefix
    mov x1, #OPEN_PREFIX_LEN
    adrp x3, newline_msg
    add x3, x3, :lo12:newline_msg
    mov x4, #NEWLINE_MSG_LEN
    bl report_name
    tbnz x0, #63, .Lreport_cannot_open_return
    mov x0, #0
.Lreport_cannot_open_return:
    ldp x29, x30, [sp], #16
    ret
.Lreport_cannot_open_assert_name:
    mov w0, #42
    bl assert_fail
.size report_cannot_open, . - report_cannot_open

// Says on standard error that the file named x0 is larger than MAX_INPUT bytes.
// In: x0 = its name.
// Out: x0 = 0, or a negative error code.
// returns: result
.type report_too_large, @function
report_too_large:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lreport_too_large_assert_name
    mov x2, x0
    adrp x0, large_prefix
    add x0, x0, :lo12:large_prefix
    mov x1, #LARGE_PREFIX_LEN
    adrp x3, large_suffix
    add x3, x3, :lo12:large_suffix
    mov x4, #LARGE_SUFFIX_LEN
    bl report_name
    tbnz x0, #63, .Lreport_too_large_return
    mov x0, #0
.Lreport_too_large_return:
    ldp x29, x30, [sp], #16
    ret
.Lreport_too_large_assert_name:
    mov w0, #43
    bl assert_fail
.size report_too_large, . - report_too_large

// Writes x1 bytes from x0, then the name x2, then x4 bytes from x3, to standard error.
// In: x0, x1 = what goes before the name; x2 = the name; x3, x4 = what goes after it.
// Out: x0 = 0, or a negative error code.
// returns: result
.type report_name, @function
report_name:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x2, .Lreport_name_assert_name
    cbz x3, .Lreport_name_assert_after
    mov x19, x2
    mov x20, x3
    mov x21, x4
    mov x2, x1
    mov x1, x0
    mov x0, #STDERR
    bl write_all
    tbnz x0, #63, .Lreport_name_return
    mov x0, x19
    bl str_len
    cbz x0, .Lreport_name_after
    mov x2, x0
    mov x1, x19
    mov x0, #STDERR
    bl write_all
    tbnz x0, #63, .Lreport_name_return
.Lreport_name_after:
    mov x0, #STDERR
    mov x1, x20
    mov x2, x21
    bl write_all
    tbnz x0, #63, .Lreport_name_return
    mov x0, #0
.Lreport_name_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lreport_name_assert_name:
    mov w0, #40
    bl assert_fail
.Lreport_name_assert_after:
    mov w0, #41
    bl assert_fail
.size report_name, . - report_name

// The length of the text at x0, up to the byte 0 that ends it.
// In: x0 = the text.
// Out: x0 = how many bytes it has before its 0.
.type str_len, @function
str_len:
    cbz x0, .Lstr_len_assert_null
    mov x1, #0
    mov x2, #MAX_ARG_LEN
.Lstr_len_loop:  // bound: 131072
    cmp x1, #MAX_ARG_LEN
    b.hs .Lstr_len_assert_index
    ldrb w3, [x0, x1]
    cbz w3, .Lstr_len_done
    add x1, x1, #1
    subs x2, x2, #1
    b.ne .Lstr_len_loop
    b .Lstr_len_assert_long
.Lstr_len_done:
    mov x0, x1
    ret
.Lstr_len_assert_null:
    mov w0, #50
    bl assert_fail
.Lstr_len_assert_index:
    mov w0, #51
    bl assert_fail
.Lstr_len_assert_long:
    mov w0, #52
    bl assert_fail
.size str_len, . - str_len

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

