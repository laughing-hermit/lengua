.arch armv8-a

// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
.equ STDERR, 2
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_WRITE_FAILED, 2
.equ EXIT_READ_FAILED, 3
.equ EXIT_TOO_LONG, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// fill's reads at most, and what makes it read to the end
.equ READ_TO_END, -1
// The bytes between words: space, and tab to carriage return (9 to 13)
.equ NEWLINE, 10
.equ SPACE, 32
.equ TAB, 9
.equ TAB_TO_CR, 4
// The longest input, and room for one byte more so a longer input shows
.equ TEXT_MAX, 1048576
.equ INPUT_LEN, TEXT_MAX + 1
// The most distinct words a text of TEXT_MAX bytes holds
.equ MAX_WORDS, TEXT_MAX / 2 + 1
// An entry: its word's length in the low 21 bits, its start in the next 21, its count above
.equ FIELD_BITS, 21
.equ FIELD_MASK, 2097151
.equ COUNT_SHIFT, 42
.equ ONE_MORE, 4398046511104
.equ ENTRIES_LEN, MAX_WORDS * 8
.equ SCRATCH_LEN, MAX_WORDS * 8
// The index: a slot of 4 bytes for each hash, its entry's number plus one, 0 where empty
.equ SLOTS, 1048576
.equ SLOT_MASK, SLOTS - 1
.equ TABLE_LEN, SLOTS * 4
// FNV-1a's 64-bit offset basis and prime
.equ FNV_OFFSET, -3750763034362895579
.equ FNV_PRIME, 0x100000001b3
// The merge sort's passes at most: runs doubling from 1 past MAX_WORDS
.equ MAX_PASSES, 21
// The buffer the lines go out from, and the most a line takes besides its word
.equ OUT_LEN, TEXT_MAX + 64
.equ LINE_EXTRA, 22
.equ MAX_DIGITS, 20
.equ MAX_WRITES, 64
.equ TOO_LONG_MSG_LEN, 39

.section .rodata

.type too_long_msg, @object
too_long_msg:
    .ascii "error: input longer than 1048576 bytes\n"
.size too_long_msg, . - too_long_msg

.section .bss

// Standard input, one byte past the limit so a longer input shows.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

// Each distinct word's entry, in the order the words first appear.
.type entries, @object
entries:
    .zero ENTRIES_LEN
.size entries, . - entries

// The merge sort's second array of entries.
.type scratch, @object
scratch:
    .zero SCRATCH_LEN
.size scratch, . - scratch

// The index from a word's hash to its entry.
.type table, @object
table:
    .zero TABLE_LEN
.size table, . - table

// The lines it prints, a buffer-full at a time.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
// Reads standard input, counts each distinct word, and prints each with its count, most
// frequent first, words with the same count in byte order.
.type main, @function
main:
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, #INPUT_LEN & 0xffff
    movk x2, #INPUT_LEN >> 16, lsl #16
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lmain_read_failed
    cmp x0, #TEXT_MAX
    b.hi .Lmain_too_long
    mov x1, x0
    adrp x0, input
    add x0, x0, :lo12:input
    adrp x2, entries
    add x2, x2, :lo12:entries
    adrp x3, table
    add x3, x3, :lo12:table
    bl count_words
    mov x9, #MAX_WORDS & 0xffff
    movk x9, #MAX_WORDS >> 16, lsl #16
    cmp x0, x9
    b.hi .Lmain_assert_words
    mov x19, x0
    adrp x0, entries
    add x0, x0, :lo12:entries
    adrp x1, scratch
    add x1, x1, :lo12:scratch
    mov x2, x19
    adrp x3, input
    add x3, x3, :lo12:input
    bl merge_sort
    mov x1, x19
    adrp x2, input
    add x2, x2, :lo12:input
    adrp x3, out
    add x3, x3, :lo12:out
    bl emit
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_OK
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
.Lmain_assert_words:
    mov w0, #1
    bl assert_fail
.size main, . - main

// Counts each distinct word of a text: each gets an entry the first time it appears, and its
// count grows each time again.
// In: x0 = the text, x1 = its length, 0 to TEXT_MAX, x2 = the entries, x3 = the index, all its
// slots 0.
// Out: x0 = how many distinct words.
.type count_words, @function
count_words:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    stp x25, x26, [sp, #-16]!
    cbz x0, .Lcount_words_assert_text
    cmp x1, #TEXT_MAX
    b.hi .Lcount_words_assert_length
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, x3
    mov x23, #0
    mov x24, #0
    mov x25, #MAX_WORDS & 0xffff
    movk x25, #MAX_WORDS >> 16, lsl #16
.Lcount_words_loop:  // bound: 524289
    mov x0, x19
    mov x1, x23
    mov x2, x20
    mov w3, #1
    bl skip
    cmp x0, x20
    b.hs .Lcount_words_done
    mov x26, x0
    mov x0, x19
    mov x1, x26
    mov x2, x20
    mov w3, #0
    bl skip
    mov x23, x0
    sub x2, x0, x26
    mov x0, x19
    mov x1, x26
    mov x3, x21
    mov x4, x22
    mov x5, x24
    bl insert
    mov x24, x0
    subs x25, x25, #1
    b.ne .Lcount_words_loop
    b .Lcount_words_assert_words
.Lcount_words_done:
    mov x0, x24
    ldp x25, x26, [sp], #16
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lcount_words_assert_text:
    mov w0, #2
    bl assert_fail
.Lcount_words_assert_length:
    mov w0, #3
    bl assert_fail
.Lcount_words_assert_words:
    mov w0, #4
    bl assert_fail
.size count_words, . - count_words

// Finds where a run of whitespace, or of other bytes, ends.
// In: x0 = the text, x1 = where to start, x2 = its length, at most TEXT_MAX, w3 = 1 to pass
// whitespace, 0 to pass a word.
// Out: x0 = the first byte from x1 on that isn't of that kind, or the length.
.type skip, @function
skip:
    cmp x2, #TEXT_MAX
    b.hi .Lskip_assert_length
    mov x9, x0
    mov x0, x1
    mov x10, #INPUT_LEN & 0xffff
    movk x10, #INPUT_LEN >> 16, lsl #16
.Lskip_loop:  // bound: 1048577
    cmp x0, x2
    b.hs .Lskip_return
    cmp x0, #TEXT_MAX
    b.hs .Lskip_assert_index
    ldrb w11, [x9, x0]
    cmp w11, #SPACE
    cset w12, eq
    sub w11, w11, #TAB
    cmp w11, #TAB_TO_CR
    cset w13, ls
    orr w12, w12, w13
    cmp w12, w3
    b.ne .Lskip_return
    add x0, x0, #1
    subs x10, x10, #1
    b.ne .Lskip_loop
    b .Lskip_assert_bound
.Lskip_return:
    ret
.Lskip_assert_length:
    mov w0, #5
    bl assert_fail
.Lskip_assert_index:
    mov w0, #6
    bl assert_fail
.Lskip_assert_bound:
    mov w0, #7
    bl assert_fail
.size skip, . - skip

// Counts one word: once more in its entry, or in a new entry where it's new.
// In: x0 = the text, x1 = the word's start in it, x2 = its length, 1 to TEXT_MAX, x3 = the
// entries, x4 = the index, x5 = how many entries there are, below MAX_WORDS.
// Out: x0 = how many entries there are now.
.type insert, @function
insert:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cmp x2, #TEXT_MAX
    b.hi .Linsert_assert_length
    mov x9, #MAX_WORDS & 0xffff
    movk x9, #MAX_WORDS >> 16, lsl #16
    cmp x5, x9
    b.hs .Linsert_assert_entries
    mov x19, x0
    lsl x20, x1, #FIELD_BITS
    orr x20, x20, x2
    mov x21, x3
    mov x22, x4
    mov x23, x5
    add x0, x19, x1
    mov x1, x2
    bl fnv
    and x4, x0, #SLOT_MASK
    mov x0, x19
    mov x1, x20
    mov x2, x21
    mov x3, x22
    mov x5, x23
    bl probe
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Linsert_assert_length:
    mov w0, #8
    bl assert_fail
.Linsert_assert_entries:
    mov w0, #9
    bl assert_fail
.size insert, . - insert

// Finds a word's entry from its slot on, the next slot each time another word holds one, and
// counts it once more there, or gives it a new entry in the first empty slot.
// In: x0 = the text, x1 = the word as an entry holds it, with no count, x2 = the entries,
// x3 = the index, x4 = the slot its hash names, below SLOTS, x5 = how many entries there are.
// Out: x0 = how many entries there are now.
.type probe, @function
probe:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    stp x25, x26, [sp, #-16]!
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, x3
    mov x23, x4
    mov x24, x5
    mov x26, #MAX_WORDS & 0xffff
    movk x26, #MAX_WORDS >> 16, lsl #16
    mov x25, #SLOTS
.Lprobe_loop:  // bound: 1048576
    cmp x23, #SLOTS
    b.hs .Lprobe_assert_slot
    ldr w3, [x22, x23, lsl #2]
    cbz w3, .Lprobe_new
    sub w3, w3, #1
    mov x0, x19
    mov x1, x20
    mov x2, x21
    bl same_word
    cbnz x0, .Lprobe_done
    add x23, x23, #1
    and x23, x23, #SLOT_MASK
    subs x25, x25, #1
    b.ne .Lprobe_loop
    b .Lprobe_assert_full
.Lprobe_new:
    cmp x24, x26
    b.hs .Lprobe_assert_room
    mov x9, #ONE_MORE
    orr x9, x9, x20
    str x9, [x21, x24, lsl #3]
    add w10, w24, #1
    cmp x23, #SLOTS
    b.hs .Lprobe_assert_new_slot
    str w10, [x22, x23, lsl #2]
    add x24, x24, #1
.Lprobe_done:
    mov x0, x24
    ldp x25, x26, [sp], #16
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lprobe_assert_slot:
    mov w0, #10
    bl assert_fail
.Lprobe_assert_full:
    mov w0, #11
    bl assert_fail
.Lprobe_assert_room:
    mov w0, #12
    bl assert_fail
.Lprobe_assert_new_slot:
    mov w0, #13
    bl assert_fail
.size probe, . - probe

// Says whether an entry is a word's, and if so counts the word once more there.
// In: x0 = the text, x1 = the word as an entry holds it, with no count, x2 = the entries,
// w3 = the entry's number, below MAX_WORDS.
// Out: x0 = 1 where the entry is the word's, which now counts it once more, else 0.
.type same_word, @function
same_word:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Lsame_word_assert_text
    mov w3, w3
    mov x22, #MAX_WORDS & 0xffff
    movk x22, #MAX_WORDS >> 16, lsl #16
    cmp x3, x22
    b.hs .Lsame_word_assert_entry
    mov x19, x2
    mov x20, x3
    mov x21, #0
    ldr x9, [x2, x3, lsl #3]
    and x10, x9, #FIELD_MASK
    and x11, x1, #FIELD_MASK
    cmp x10, x11
    b.ne .Lsame_word_return
    lsr x9, x9, #FIELD_BITS
    and x9, x9, #FIELD_MASK
    lsr x12, x1, #FIELD_BITS
    and x12, x12, #FIELD_MASK
    add x2, x0, x12
    add x0, x0, x9
    mov x1, x10
    mov x3, x11
    bl compare_bytes
    cbnz x0, .Lsame_word_return
    cmp x20, x22
    b.hs .Lsame_word_assert_count
    ldr x9, [x19, x20, lsl #3]
    mov x10, #ONE_MORE
    add x9, x9, x10
    str x9, [x19, x20, lsl #3]
    mov x21, #1
.Lsame_word_return:
    mov x0, x21
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lsame_word_assert_text:
    mov w0, #14
    bl assert_fail
.Lsame_word_assert_entry:
    mov w0, #15
    bl assert_fail
.Lsame_word_assert_count:
    mov w0, #16
    bl assert_fail
.size same_word, . - same_word

// FNV-1a, 64 bits, of some bytes.
// In: x0 = the bytes, x1 = how many, at most TEXT_MAX.
// Out: x0 = the hash.
.type fnv, @function
fnv:
    cbz x0, .Lfnv_assert_bytes
    cmp x1, #TEXT_MAX
    b.hi .Lfnv_assert_length
    mov x9, x0
    mov x0, #FNV_OFFSET & 0xffff
    movk x0, #(FNV_OFFSET >> 16) & 0xffff, lsl #16
    movk x0, #(FNV_OFFSET >> 32) & 0xffff, lsl #32
    movk x0, #(FNV_OFFSET >> 48) & 0xffff, lsl #48
    mov x10, #FNV_PRIME & 0xffff
    movk x10, #FNV_PRIME >> 32, lsl #32
    mov x11, #0
    mov x12, #TEXT_MAX
.Lfnv_loop:  // bound: 1048576
    cmp x11, x1
    b.hs .Lfnv_return
    ldrb w13, [x9, x11]
    eor x0, x0, x13
    mul x0, x0, x10
    add x11, x11, #1
    subs x12, x12, #1
    b.ne .Lfnv_loop
.Lfnv_return:
    ret
.Lfnv_assert_bytes:
    mov w0, #17
    bl assert_fail
.Lfnv_assert_length:
    mov w0, #18
    bl assert_fail
.size fnv, . - fnv

// Compares two runs of bytes in byte order, as a shorter run that starts the other comes first.
// In: x0 = the first, x1 = its length, x2 = the second, x3 = its length, each at most TEXT_MAX.
// Out: x0 = -1 when the first comes first, 0 when they're the same, 1 when the second does.
.type compare_bytes, @function
compare_bytes:
    cbz x0, .Lcompare_bytes_assert_first
    cbz x2, .Lcompare_bytes_assert_second
    cmp x1, #TEXT_MAX
    b.hi .Lcompare_bytes_assert_first_length
    cmp x3, #TEXT_MAX
    b.hi .Lcompare_bytes_assert_second_length
    cmp x1, x3
    csel x9, x3, x1, hi
    mov x10, #0
    mov x11, #TEXT_MAX
.Lcompare_bytes_loop:  // bound: 1048576
    cmp x10, x9
    b.hs .Lcompare_bytes_lengths
    ldrb w12, [x0, x10]
    ldrb w13, [x2, x10]
    cmp w12, w13
    b.ne .Lcompare_bytes_differ
    add x10, x10, #1
    subs x11, x11, #1
    b.ne .Lcompare_bytes_loop
.Lcompare_bytes_lengths:
    mov x12, x1
    mov x13, x3
.Lcompare_bytes_differ:
    cmp x12, x13
    mov x0, #-1
    mov x14, #1
    csel x0, x14, x0, hi
    csel x0, xzr, x0, eq
    ret
.Lcompare_bytes_assert_first:
    mov w0, #19
    bl assert_fail
.Lcompare_bytes_assert_second:
    mov w0, #20
    bl assert_fail
.Lcompare_bytes_assert_first_length:
    mov w0, #21
    bl assert_fail
.Lcompare_bytes_assert_second_length:
    mov w0, #22
    bl assert_fail
.size compare_bytes, . - compare_bytes

// Says which of two entries comes first: the more frequent, or with the same count, the word
// first in byte order.
// In: x0 = the first entry, x1 = the second, x2 = the text their words are in.
// Out: x0 = negative when the first comes first, 0 when they're the same, positive when the
// second does.
.type compare_entries, @function
compare_entries:
    stp x29, x30, [sp, #-16]!
    cbz x2, .Lcompare_entries_assert_text
    lsr x9, x0, #COUNT_SHIFT
    cbz x9, .Lcompare_entries_assert_count
    lsr x10, x1, #COUNT_SHIFT
    cmp x9, x10
    mov x11, #-1
    b.hi .Lcompare_entries_count
    mov x11, #1
    b.lo .Lcompare_entries_count
    lsr x12, x0, #FIELD_BITS
    and x12, x12, #FIELD_MASK
    lsr x13, x1, #FIELD_BITS
    and x13, x13, #FIELD_MASK
    and x3, x1, #FIELD_MASK
    and x1, x0, #FIELD_MASK
    add x0, x2, x12
    add x2, x2, x13
    bl compare_bytes
    b .Lcompare_entries_return
.Lcompare_entries_count:
    mov x0, x11
.Lcompare_entries_return:
    ldp x29, x30, [sp], #16
    ret
.Lcompare_entries_assert_text:
    mov w0, #23
    bl assert_fail
.Lcompare_entries_assert_count:
    mov w0, #24
    bl assert_fail
.size compare_entries, . - compare_entries

// Merges the sorted runs src[lo, mid) and src[mid, hi) into dst[lo, hi).
// In: x0 = src, x1 = dst, x2 = lo, x3 = mid, x4 = hi, x5 = the text the entries' words are in;
// lo <= mid <= hi.
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
    mov x26, #MAX_WORDS & 0xffff
    movk x26, #MAX_WORDS >> 16, lsl #16
.Lmerge_loop:  // bound: 524289
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
    ldr x9, [x23], #8
    b .Lmerge_store
.Lmerge_take_right:
    ldr x9, [x24], #8
.Lmerge_store:
    str x9, [x25], #8
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
    mov w0, #25
    bl assert_fail
.Lmerge_assert_bound:
    mov w0, #26
    bl assert_fail
.size merge, . - merge

// Merges each pair of neighboring sorted runs of a given width from src into dst.
// In: x0 = src, x1 = dst, x2 = how many entries, x3 = the run width, at least 1, x4 = the text
// the entries' words are in.
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
    mov x25, #MAX_WORDS & 0xffff
    movk x25, #MAX_WORDS >> 16, lsl #16
.Lmerge_pass_loop:  // bound: 524289
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
    mov w0, #27
    bl assert_fail
.Lmerge_pass_assert_bound:
    mov w0, #28
    bl assert_fail
.size merge_pass, . - merge_pass

// Sorts entries with a bottom-up merge sort, back and forth between two arrays.
// In: x0 = the entries, x1 = an array as large, x2 = how many, x3 = the text their words are in.
// Out: x0 = whichever of the two arrays holds the sorted entries.
.type merge_sort, @function
merge_sort:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x0, .Lmerge_sort_assert_null
    mov x9, #MAX_WORDS & 0xffff
    movk x9, #MAX_WORDS >> 16, lsl #16
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
    mov w0, #29
    bl assert_fail
.Lmerge_sort_assert_count:
    mov w0, #30
    bl assert_fail
.Lmerge_sort_assert_bound:
    mov w0, #31
    bl assert_fail
.size merge_sort, . - merge_sort

// Prints each entry's line: its count, a space, its word and a newline, a buffer-full at a time.
// In: x0 = the entries, x1 = how many, at most MAX_WORDS, x2 = the text their words are in,
// x3 = the buffer to print from, OUT_LEN bytes.
// Out: x0 = 0, or a negative error code.
// returns: result
.type emit, @function
emit:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    stp x25, x26, [sp, #-16]!
    cbz x0, .Lemit_assert_entries
    mov x26, #MAX_WORDS & 0xffff
    movk x26, #MAX_WORDS >> 16, lsl #16
    cmp x1, x26
    b.hi .Lemit_assert_count
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, x3
    mov x23, #0
    mov x24, #0
    mov x25, #MAX_WORDS & 0xffff
    movk x25, #MAX_WORDS >> 16, lsl #16
.Lemit_loop:  // bound: 524289
    cmp x24, x20
    b.hs .Lemit_rest
    cmp x24, x26
    b.hs .Lemit_assert_index
    ldr x0, [x19, x24, lsl #3]
    mov x1, x21
    mov x2, x22
    mov x3, x23
    bl emit_line
    tbnz x0, #63, .Lemit_return
    mov x23, x0
    add x24, x24, #1
    subs x25, x25, #1
    b.ne .Lemit_loop
    b .Lemit_assert_bound
.Lemit_rest:
    mov x0, #0
    cbz x23, .Lemit_return
    mov x0, #STDOUT
    mov x1, x22
    mov x2, x23
    bl write_all
    tbnz x0, #63, .Lemit_return
    mov x0, #0
.Lemit_return:
    ldp x25, x26, [sp], #16
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lemit_assert_entries:
    mov w0, #32
    bl assert_fail
.Lemit_assert_count:
    mov w0, #33
    bl assert_fail
.Lemit_assert_index:
    mov w0, #34
    bl assert_fail
.Lemit_assert_bound:
    mov w0, #35
    bl assert_fail
.size emit, . - emit

// Adds an entry's line to the buffer, printing what the buffer holds first where the line
// doesn't fit after it.
// In: x0 = the entry, x1 = the text its word is in, x2 = the buffer, OUT_LEN bytes, x3 = how many
// bytes of it are taken.
// Out: x0 = how many are taken now, or a negative error code.
// returns: result
.type emit_line, @function
emit_line:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x1, .Lemit_line_assert_text
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, x3
    and x9, x19, #FIELD_MASK
    add x9, x9, #LINE_EXTRA
    add x9, x9, x22
    mov x10, #OUT_LEN & 0xffff
    movk x10, #OUT_LEN >> 16, lsl #16
    cmp x9, x10
    b.ls .Lemit_line_put
    cbz x22, .Lemit_line_assert_room
    mov x0, #STDOUT
    mov x1, x21
    mov x2, x22
    bl write_all
    tbnz x0, #63, .Lemit_line_return
    mov x22, #0
.Lemit_line_put:
    mov x0, x19
    mov x1, x20
    mov x2, x21
    mov x3, x22
    bl put_line
.Lemit_line_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lemit_line_assert_text:
    mov w0, #36
    bl assert_fail
.Lemit_line_assert_room:
    mov w0, #37
    bl assert_fail
.size emit_line, . - emit_line

// Writes an entry's line into the buffer: its count, a space, its word and a newline.
// In: x0 = the entry, x1 = the text its word is in, x2 = the buffer, OUT_LEN bytes, x3 = how
// many bytes of it are taken, with room for the line after them.
// Out: x0 = how many are taken now.
.type put_line, @function
put_line:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    mov x19, x0
    mov x20, x1
    mov x21, x2
    lsr x0, x19, #COUNT_SHIFT
    mov x1, x21
    mov x2, x3
    bl put_number
    mov x22, x0
    mov x9, #OUT_LEN & 0xffff
    movk x9, #OUT_LEN >> 16, lsl #16
    cmp x22, x9
    b.hs .Lput_line_assert_space
    mov w10, #SPACE
    strb w10, [x21, x22]
    add x22, x22, #1
    lsr x11, x19, #FIELD_BITS
    and x11, x11, #FIELD_MASK
    and x12, x19, #FIELD_MASK
    mov x13, #0
    mov x14, #TEXT_MAX
.Lput_line_copy:  // bound: 1048576
    cmp x13, x12
    b.hs .Lput_line_end
    add x15, x11, x13
    cmp x15, #TEXT_MAX
    b.hs .Lput_line_assert_word
    ldrb w10, [x20, x15]
    cmp x22, x9
    b.hs .Lput_line_assert_byte
    strb w10, [x21, x22]
    add x22, x22, #1
    add x13, x13, #1
    subs x14, x14, #1
    b.ne .Lput_line_copy
.Lput_line_end:
    cmp x22, x9
    b.hs .Lput_line_assert_newline
    mov w10, #NEWLINE
    strb w10, [x21, x22]
    add x0, x22, #1
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lput_line_assert_space:
    mov w0, #38
    bl assert_fail
.Lput_line_assert_word:
    mov w0, #39
    bl assert_fail
.Lput_line_assert_byte:
    mov w0, #40
    bl assert_fail
.Lput_line_assert_newline:
    mov w0, #41
    bl assert_fail
.size put_line, . - put_line

// Writes a whole number's decimal digits into a buffer.
// In: x0 = the number, x1 = the buffer, OUT_LEN bytes, x2 = where in it the digits start.
// Out: x0 = where they end.
.type put_number, @function
put_number:
    mov x9, x2
    mov x10, x0
    mov x11, #10
    mov x12, #0
    mov x13, #MAX_DIGITS
.Lput_number_count:  // bound: 20
    udiv x10, x10, x11
    add x12, x12, #1
    cbz x10, .Lput_number_write
    subs x13, x13, #1
    b.ne .Lput_number_count
    b .Lput_number_assert_counted
.Lput_number_write:
    add x14, x9, x12
    mov x10, x0
    mov x15, #OUT_LEN & 0xffff
    movk x15, #OUT_LEN >> 16, lsl #16
    mov x13, #MAX_DIGITS
.Lput_number_digit:  // bound: 20
    udiv x16, x10, x11
    msub x17, x16, x11, x10
    add w17, w17, #48
    sub x14, x14, #1
    cmp x14, x15
    b.hs .Lput_number_assert_index
    strb w17, [x1, x14]
    mov x10, x16
    cbz x10, .Lput_number_done
    subs x13, x13, #1
    b.ne .Lput_number_digit
    b .Lput_number_assert_written
.Lput_number_done:
    add x0, x9, x12
    ret
.Lput_number_assert_counted:
    mov w0, #42
    bl assert_fail
.Lput_number_assert_index:
    mov w0, #43
    bl assert_fail
.Lput_number_assert_written:
    mov w0, #44
    bl assert_fail
.size put_number, . - put_number

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
    mov w0, #45
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #46
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #47
    bl assert_fail
.size write_all, . - write_all

