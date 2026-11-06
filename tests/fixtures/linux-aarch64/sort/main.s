.arch armv8-a

// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
.equ STDERR, 2
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_WRITE_FAILED, 2
.equ EXIT_READ_FAILED, 3
.equ EXIT_TOO_LONG, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
// Argument counts, the program's name included
.equ ARGC_PLAIN, 1
.equ ARGC_OPTION, 2
// Bytes that matter
.equ NEWLINE, 10
.equ READ_TO_END, -1
// Sizes and limits
.equ MAX_INPUT, 1048576
.equ TEXT_MAX, MAX_INPUT + 1
.equ WORD_BYTES, 8
.equ IN_BUF_LEN, TEXT_MAX + WORD_BYTES
.equ MAX_WORDS, MAX_INPUT / WORD_BYTES + 1
.equ OUT_BUF_LEN, MAX_INPUT + 1
.equ MAX_LINES, MAX_INPUT + 1
.equ LINES_LEN, MAX_LINES * 8
.equ MAX_PASSES, 21
.equ MAX_WRITES, 64
.equ USAGE_MSG_LEN, 17
.equ TOO_LONG_MSG_LEN, 39

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: sort [-u]\n"
.size usage_msg, . - usage_msg

.type too_long_msg, @object
too_long_msg:
    .ascii "error: input longer than 1048576 bytes\n"
.size too_long_msg, . - too_long_msg

.section .bss

// Standard input, one byte past the limit so a longer input shows, and room for a final newline,
// then 8 spare bytes so comparing lines eight bytes at a time stays inside.
.type in_buf, @object
in_buf:
    .zero IN_BUF_LEN
.size in_buf, . - in_buf

// Each line as a doubleword: its start in in_buf in the low 32 bits, its length in the high 32.
.type lines_a, @object
lines_a:
    .zero LINES_LEN
.size lines_a, . - lines_a

// The merge sort's second array of lines.
.type lines_b, @object
lines_b:
    .zero LINES_LEN
.size lines_b, . - lines_b

// The sorted lines, each with its newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
// Checks the arguments, reads standard input, sorts its lines and prints them.
.type main, @function
main:
    mov x21, #0
    cmp x0, #ARGC_PLAIN
    b.eq .Lmain_read
    cmp x0, #ARGC_OPTION
    b.ne .Lmain_usage
    ldr x0, [x1, #8]
    cbz x0, .Lmain_assert_argv
    bl is_unique_option
    tbnz x0, #63, .Lmain_usage
    mov x21, #1
.Lmain_read:
    mov x0, #STDIN
    adrp x1, in_buf
    add x1, x1, :lo12:in_buf
    mov x2, #TEXT_MAX & 0xffff
    movk x2, #TEXT_MAX >> 16, lsl #16
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lmain_read_failed
    cmp x0, #MAX_INPUT
    b.hi .Lmain_too_long
    cbz x0, .Lmain_ok
    mov x1, x0
    adrp x19, in_buf
    add x19, x19, :lo12:in_buf
    mov x0, x19
    bl end_last_line
    mov x1, x0
    mov x0, x19
    mov x2, x21
    bl sort_text
    tbnz x0, #63, .Lmain_write_failed
.Lmain_ok:
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_usage:
    mov x0, #STDERR
    adrp x1, usage_msg
    add x1, x1, :lo12:usage_msg
    mov x2, #USAGE_MSG_LEN
    bl write_all
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_USAGE
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
.Lmain_read_failed:
    mov w0, #EXIT_READ_FAILED
    b .Lmain_exit
.Lmain_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_argv:
    mov w0, #1
    bl assert_fail
.size main, . - main

// Gives a text's last line a newline when it lacks one.
// In: x0 = the text, with room for TEXT_MAX bytes, x1 = its length, 1 to MAX_INPUT.
// Out: x0 = its new length.
.type end_last_line, @function
end_last_line:
    cbz x1, .Lend_assert_empty
    mov x9, x0
    mov x0, x1
    sub x10, x1, #1
    mov x11, #TEXT_MAX & 0xffff
    movk x11, #TEXT_MAX >> 16, lsl #16
    cmp x10, x11
    b.hs .Lend_assert_last
    ldrb w12, [x9, x10]
    cmp w12, #NEWLINE
    b.eq .Lend_return
    cmp x1, x11
    b.hs .Lend_assert_append
    mov w12, #NEWLINE
    strb w12, [x9, x1]
    add x0, x0, #1
.Lend_return:
    ret
.Lend_assert_empty:
    mov w0, #2
    bl assert_fail
.Lend_assert_last:
    mov w0, #3
    bl assert_fail
.Lend_assert_append:
    mov w0, #12
    bl assert_fail
.size end_last_line, . - end_last_line

// Tells whether an argument is exactly "-u".
// In: x0 = a NUL-terminated string.
// Out: x0 = 0 when it is, or ERROR_INVALID.
// returns: result
.type is_unique_option, @function
is_unique_option:
    cbz x0, .Lis_unique_assert_null
    mov x9, x0
    mov x0, #ERROR_INVALID
    ldrb w10, [x9]
    cmp w10, #'-'
    b.ne .Lis_unique_return
    ldrb w10, [x9, #1]
    cmp w10, #'u'
    b.ne .Lis_unique_return
    ldrb w10, [x9, #2]
    cbnz w10, .Lis_unique_return
    mov x0, #0
.Lis_unique_return:
    ret
.Lis_unique_assert_null:
    mov w0, #28
    bl assert_fail
.size is_unique_option, . - is_unique_option

// Sorts the lines of a text, keeps one of each when asked, and writes them to standard output.
// In: x0 = the text, every line ending with a newline, x1 = its length, 1 to TEXT_MAX,
// w2 = 1 to drop duplicate lines, else 0.
// Out: x0 = 0, or a negative error code.
// returns: result
.type sort_text, @function
sort_text:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x0, .Lsort_text_assert_null
    mov x9, #TEXT_MAX & 0xffff
    movk x9, #TEXT_MAX >> 16, lsl #16
    cmp x1, x9
    b.hi .Lsort_text_assert_length
    mov x19, x0
    mov w20, w2
    adrp x2, lines_a
    add x2, x2, :lo12:lines_a
    bl split_lines
    mov x21, x0
    adrp x0, lines_a
    add x0, x0, :lo12:lines_a
    adrp x1, lines_b
    add x1, x1, :lo12:lines_b
    mov x2, x21
    mov x3, x19
    bl merge_sort
    mov x22, x0
    cbz w20, .Lsort_text_emit
    mov x0, x22
    mov x1, x21
    mov x2, x19
    bl dedupe
    mov x21, x0
.Lsort_text_emit:
    mov x0, x22
    mov x1, x21
    mov x2, x19
    adrp x3, out_buf
    add x3, x3, :lo12:out_buf
    bl emit_lines
    mov x2, x0
    mov x0, #0
    cbz x2, .Lsort_text_return
    mov x0, #STDOUT
    adrp x1, out_buf
    add x1, x1, :lo12:out_buf
    bl write_all
    tbnz x0, #63, .Lsort_text_return
    mov x0, #0
.Lsort_text_return:
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lsort_text_assert_null:
    mov w0, #29
    bl assert_fail
.Lsort_text_assert_length:
    mov w0, #30
    bl assert_fail
.size sort_text, . - sort_text

// Splits text whose every line ends with a newline into line entries.
// In: x0 = the text, x1 = its length, 1 to TEXT_MAX, x2 = room for MAX_LINES entries.
// Out: x0 = how many lines.
.type split_lines, @function
split_lines:
    cbz x0, .Lsplit_assert_null
    mov x9, x0
    mov x0, #0
    mov x10, #0
    mov x11, #0
    mov x12, #TEXT_MAX & 0xffff
    movk x12, #TEXT_MAX >> 16, lsl #16
    mov x13, #MAX_LINES & 0xffff
    movk x13, #MAX_LINES >> 16, lsl #16
    mov x14, #TEXT_MAX & 0xffff
    movk x14, #TEXT_MAX >> 16, lsl #16
.Lsplit_loop:  // bound: 1048577
    cmp x10, x12
    b.hs .Lsplit_assert_index
    ldrb w15, [x9, x10]
    cmp w15, #NEWLINE
    b.ne .Lsplit_next
    cmp x0, x13
    b.hs .Lsplit_assert_lines
    sub x16, x10, x11
    orr x16, x11, x16, lsl #32
    str x16, [x2, x0, lsl #3]
    add x0, x0, #1
    add x11, x10, #1
.Lsplit_next:
    add x10, x10, #1
    cmp x10, x1
    b.hs .Lsplit_return
    subs x14, x14, #1
    b.ne .Lsplit_loop
    b .Lsplit_assert_bound
.Lsplit_return:
    ret
.Lsplit_assert_null:
    mov w0, #4
    bl assert_fail
.Lsplit_assert_index:
    mov w0, #5
    bl assert_fail
.Lsplit_assert_lines:
    mov w0, #6
    bl assert_fail
.Lsplit_assert_bound:
    mov w0, #7
    bl assert_fail
.size split_lines, . - split_lines

// Compares two lines in byte order, a shorter line first when it starts the other, eight
// bytes at a time; the text has 8 spare bytes past its end, so the last load stays inside.
// In: x0 = the first line's entry, x1 = the second's, x2 = the text they're in.
// Out: x0 negative, zero or positive as the first line sorts before, equal to, or after.
.type compare_entries, @function
compare_entries:
    lsr x9, x0, #32
    lsr x10, x1, #32
    mov w11, w0
    add x11, x2, x11
    mov w12, w1
    add x12, x2, x12
    cmp x9, x10
    csel x13, x9, x10, ls
    mov x14, #0
    mov x15, #MAX_INPUT
    mov x16, #MAX_WORDS & 0xffff
    movk x16, #MAX_WORDS >> 16, lsl #16
.Lcompare_loop:  // bound: 131073
    add x17, x14, #WORD_BYTES
    cmp x17, x13
    b.hi .Lcompare_tail
    cmp x14, x15
    b.hs .Lcompare_assert_index
    ldr x3, [x11, x14]
    ldr x4, [x12, x14]
    cmp x3, x4
    b.ne .Lcompare_words
    add x14, x14, #WORD_BYTES
    subs x16, x16, #1
    b.ne .Lcompare_loop
    b .Lcompare_assert_bound
.Lcompare_words:
    rev x3, x3
    rev x4, x4
    b .Lcompare_ordered
.Lcompare_tail:
    subs x5, x13, x14
    b.eq .Lcompare_lengths
    cmp x14, x15
    b.hs .Lcompare_assert_tail
    ldr x3, [x11, x14]
    ldr x4, [x12, x14]
    lsl x5, x5, #3
    mov x6, #64
    sub x6, x6, x5
    rev x3, x3
    rev x4, x4
    lsr x3, x3, x6
    lsr x4, x4, x6
    cmp x3, x4
    b.eq .Lcompare_lengths
.Lcompare_ordered:
    cmp x3, x4
    mov x0, #1
    mov x7, #-1
    csel x0, x7, x0, lo
    b .Lcompare_return
.Lcompare_lengths:
    sub x0, x9, x10
.Lcompare_return:
    ret
.Lcompare_assert_index:
    mov w0, #9
    bl assert_fail
.Lcompare_assert_bound:
    mov w0, #10
    bl assert_fail
.Lcompare_assert_tail:
    mov w0, #31
    bl assert_fail
.size compare_entries, . - compare_entries

// Merges the sorted runs src[lo, mid) and src[mid, hi) into dst[lo, hi).
// In: x0 = src, x1 = dst, x2 = lo, x3 = mid, x4 = hi, x5 = the text; lo <= mid <= hi.
.type merge, @function
merge:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    stp x25, x26, [sp, #-16]!
    cmp x2, x3
    b.hi .Lmerge_assert_order
    add x19, x0, x3, lsl #3
    add x20, x0, x4, lsl #3
    sub x21, x4, x2
    mov x22, x5
    add x23, x0, x2, lsl #3
    add x24, x0, x3, lsl #3
    add x25, x1, x2, lsl #3
    cbz x21, .Lmerge_done
    mov x26, #MAX_LINES & 0xffff
    movk x26, #MAX_LINES >> 16, lsl #16
.Lmerge_loop:  // bound: 1048577
    cmp x23, x19
    b.hs .Lmerge_take_right
    cmp x24, x20
    b.hs .Lmerge_take_left
    ldr x0, [x23]
    ldr x1, [x24]
    mov x2, x22
    bl compare_entries
    cmp x0, #0
    b.gt .Lmerge_take_right
.Lmerge_take_left:
    ldr x9, [x23]
    add x23, x23, #8
    b .Lmerge_store
.Lmerge_take_right:
    ldr x9, [x24]
    add x24, x24, #8
.Lmerge_store:
    str x9, [x25]
    add x25, x25, #8
    subs x21, x21, #1
    b.eq .Lmerge_done
    subs x26, x26, #1
    b.ne .Lmerge_loop
    b .Lmerge_assert_bound
.Lmerge_done:
    ldp x25, x26, [sp], #16
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lmerge_assert_order:
    mov w0, #11
    bl assert_fail
.Lmerge_assert_bound:
    mov w0, #13
    bl assert_fail
.size merge, . - merge

// Merges each pair of neighboring sorted runs of a given width from src into dst.
// In: x0 = src, x1 = dst, x2 = how many entries, x3 = the run width, at least 1, x4 = the text.
.type merge_pass, @function
merge_pass:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    stp x25, x26, [sp, #-16]!
    cbz x3, .Lmerge_pass_assert_width
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, x3
    mov x23, x4
    mov x24, #0
    mov x25, #MAX_LINES & 0xffff
    movk x25, #MAX_LINES >> 16, lsl #16
.Lmerge_pass_loop:  // bound: 1048577
    cmp x24, x21
    b.hs .Lmerge_pass_done
    add x3, x24, x22
    cmp x3, x21
    csel x3, x21, x3, hi
    add x4, x3, x22
    cmp x4, x21
    csel x4, x21, x4, hi
    mov x2, x24
    mov x24, x4
    mov x0, x19
    mov x1, x20
    mov x5, x23
    bl merge
    subs x25, x25, #1
    b.ne .Lmerge_pass_loop
    b .Lmerge_pass_assert_bound
.Lmerge_pass_done:
    ldp x25, x26, [sp], #16
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lmerge_pass_assert_width:
    mov w0, #14
    bl assert_fail
.Lmerge_pass_assert_bound:
    mov w0, #15
    bl assert_fail
.size merge_pass, . - merge_pass

// Sorts line entries with a bottom-up merge sort, back and forth between two arrays.
// In: x0 = the entries, x1 = an array as large, x2 = how many, x3 = the text.
// Out: x0 = whichever of the two arrays holds the sorted entries.
.type merge_sort, @function
merge_sort:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x0, .Lmerge_sort_assert_null
    mov x9, #MAX_LINES & 0xffff
    movk x9, #MAX_LINES >> 16, lsl #16
    cmp x2, x9
    b.hi .Lmerge_sort_assert_count
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, #1
    mov x23, x3
    mov x24, #MAX_PASSES
.Lmerge_sort_loop:  // bound: 21
    cmp x22, x21
    b.hs .Lmerge_sort_done
    mov x0, x19
    mov x1, x20
    mov x2, x21
    mov x3, x22
    mov x4, x23
    bl merge_pass
    mov x9, x19
    mov x19, x20
    mov x20, x9
    lsl x22, x22, #1
    subs x24, x24, #1
    b.ne .Lmerge_sort_loop
    b .Lmerge_sort_assert_bound
.Lmerge_sort_done:
    mov x0, x19
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lmerge_sort_assert_null:
    mov w0, #16
    bl assert_fail
.Lmerge_sort_assert_count:
    mov w0, #17
    bl assert_fail
.Lmerge_sort_assert_bound:
    mov w0, #18
    bl assert_fail
.size merge_sort, . - merge_sort

// Keeps the first of each run of equal sorted lines, in place.
// In: x0 = the sorted entries, x1 = how many, x2 = the text.
// Out: x0 = how many are kept.
.type dedupe, @function
dedupe:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    mov x23, #0
    cbz x1, .Ldedupe_done
    mov x9, #MAX_LINES & 0xffff
    movk x9, #MAX_LINES >> 16, lsl #16
    cmp x1, x9
    b.hi .Ldedupe_assert_count
    mov x19, x2
    mov x20, x0
    add x21, x0, #8
    sub x22, x1, #1
    mov x23, #1
    mov x24, #MAX_LINES & 0xffff
    movk x24, #MAX_LINES >> 16, lsl #16
.Ldedupe_loop:  // bound: 1048577
    cbz x22, .Ldedupe_done
    ldr x0, [x20]
    ldr x1, [x21]
    mov x2, x19
    bl compare_entries
    cbz x0, .Ldedupe_skip
    ldr x9, [x21]
    add x20, x20, #8
    str x9, [x20]
    add x23, x23, #1
.Ldedupe_skip:
    add x21, x21, #8
    sub x22, x22, #1
    subs x24, x24, #1
    b.ne .Ldedupe_loop
    b .Ldedupe_assert_bound
.Ldedupe_done:
    mov x0, x23
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Ldedupe_assert_count:
    mov w0, #19
    bl assert_fail
.Ldedupe_assert_bound:
    mov w0, #20
    bl assert_fail
.size dedupe, . - dedupe

// Copies the lines in order into the output, each followed by a newline.
// In: x0 = the entries, x1 = how many, x2 = the text, x3 = OUT_BUF_LEN bytes of output.
// Out: x0 = the output's length.
.type emit_lines, @function
emit_lines:
    mov x9, x0
    mov x0, #0
    cbz x1, .Lemit_return
    mov x15, #OUT_BUF_LEN & 0xffff
    movk x15, #OUT_BUF_LEN >> 16, lsl #16
    mov x10, #MAX_LINES & 0xffff
    movk x10, #MAX_LINES >> 16, lsl #16
.Lemit_line:  // bound: 1048577
    ldr x11, [x9]
    mov w12, w11
    lsr x11, x11, #32
    add x12, x2, x12
    cbz x11, .Lemit_newline
    mov x13, #MAX_INPUT
.Lemit_byte:  // bound: 1048576
    cmp x0, x15
    b.hs .Lemit_assert_byte
    ldrb w14, [x12]
    strb w14, [x3, x0]
    add x12, x12, #1
    add x0, x0, #1
    subs x11, x11, #1
    b.eq .Lemit_newline
    subs x13, x13, #1
    b.ne .Lemit_byte
    b .Lemit_assert_line_bound
.Lemit_newline:
    cmp x0, x15
    b.hs .Lemit_assert_newline
    mov w14, #NEWLINE
    strb w14, [x3, x0]
    add x0, x0, #1
    add x9, x9, #8
    subs x1, x1, #1
    b.eq .Lemit_return
    subs x10, x10, #1
    b.ne .Lemit_line
    b .Lemit_assert_lines_bound
.Lemit_return:
    ret
.Lemit_assert_byte:
    mov w0, #21
    bl assert_fail
.Lemit_assert_line_bound:
    mov w0, #22
    bl assert_fail
.Lemit_assert_newline:
    mov w0, #23
    bl assert_fail
.Lemit_assert_lines_bound:
    mov w0, #24
    bl assert_fail
.size emit_lines, . - emit_lines

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
    mov w0, #25
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #26
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #27
    bl assert_fail
.size write_all, . - write_all

