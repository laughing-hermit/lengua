.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_READ, 0
.equ SYS_WRITE, 1
.equ SYS_EXIT, 60
# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
.equ STDERR, 2
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_WRITE_FAILED, 2
.equ EXIT_READ_FAILED, 3
.equ EXIT_TOO_LONG, 4
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# fill's reads at most, and what makes it read to the end
.equ MAX_FILL_READS, 1099511627776
.equ READ_TO_END, -1
# Bytes that matter
.equ NEWLINE, 10
.equ SPACE, 32
.equ TAB, 9
# Tab through carriage return are the 5 bytes from 9 to 13.
.equ TAB_TO_CR, 4
# The longest text it counts, and one byte more, which shows a text is longer
.equ TEXT_MAX, 1048576
.equ INPUT_LEN, TEXT_MAX + 1
# The most words a text holds, each a byte and the whitespace after it
.equ MAX_WORDS, TEXT_MAX / 2 + 1
# A word's entry, one qword: its count, its start in the input and its length, 21 bits each,
# the count highest, so adding ONE_MORE counts it once more
.equ FIELD_BITS, 21
.equ FIELD_MASK, 2097151
.equ COUNT_SHIFT, 42
.equ ONE_MORE, 4398046511104
.equ ENTRIES_LEN, MAX_WORDS * 8
.equ SCRATCH_LEN, MAX_WORDS * 8
# The index: each slot an entry's number plus one, or 0 for none, found by the word's hash;
# a power of two of slots, at least twice the most words
.equ SLOTS, 1048576
.equ SLOT_MASK, SLOTS - 1
.equ TABLE_LEN, SLOTS * 4
# FNV-1a, 64 bits: its offset basis, 0xcbf29ce484222325, as a signed number, and its prime
.equ FNV_OFFSET, -3750763034362895579
.equ FNV_PRIME, 0x100000001b3
# Sorting the entries: a bottom-up merge sort doubles its runs at most this often
.equ MAX_PASSES, 21
# Printing: a line is a count of up to 20 digits, a space, the word and a newline
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

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# Standard input, one byte past the limit so a longer input shows.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

# Each distinct word's entry, in the order the words first appear.
.type entries, @object
entries:
    .zero ENTRIES_LEN
.size entries, . - entries

# The merge sort's second array of entries.
.type scratch, @object
scratch:
    .zero SCRATCH_LEN
.size scratch, . - scratch

# The index from a word's hash to its entry.
.type table, @object
table:
    .zero TABLE_LEN
.size table, . - table

# The lines it prints, a buffer-full at a time.
.type out, @object
out:
    .zero OUT_LEN
.size out, . - out

.text
.globl _start

# Reads standard input, counts each distinct word, and prints each with its count, most
# frequent first, words with the same count in byte order.
.type _start, @function
_start:
    mov edi, STDIN
    lea rsi, [rip + input]
    mov edx, INPUT_LEN
    mov ecx, READ_TO_END
    call fill
    test rax, rax
    js .Lstart_read_failed
    cmp rax, TEXT_MAX
    ja .Lstart_too_long
    lea rdi, [rip + input]
    mov rsi, rax
    lea rdx, [rip + entries]
    lea rcx, [rip + table]
    call count_words
    cmp rax, MAX_WORDS
    ja .Lstart_assert_words
    mov rbx, rax
    lea rdi, [rip + entries]
    lea rsi, [rip + scratch]
    mov rdx, rbx
    lea rcx, [rip + input]
    call merge_sort
    mov rdi, rax
    mov rsi, rbx
    lea rdx, [rip + input]
    lea rcx, [rip + out]
    call emit
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_too_long:
    mov edi, STDERR
    lea rsi, [rip + too_long_msg]
    mov edx, TOO_LONG_MSG_LEN
    call write_all
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_TOO_LONG
    jmp .Lstart_exit
.Lstart_read_failed:
    mov edi, EXIT_READ_FAILED
    jmp .Lstart_exit
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_words:
    mov edi, 1
    call assert_fail
.size _start, . - _start

# Counts each distinct word of a text: each gets an entry the first time it appears, and its
# count grows each time again.
# In: rdi = the text, rsi = its length, 0 to TEXT_MAX, rdx = the entries, rcx = the index, all
# its slots 0.
# Out: rax = how many distinct words.
.type count_words, @function
count_words:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 24
    test rdi, rdi
    jz .Lcount_words_assert_text
    cmp rsi, TEXT_MAX
    ja .Lcount_words_assert_length
    mov r12, rdi
    mov r13, rsi
    mov r14, rdx
    mov r15, rcx
    xor ebx, ebx
    mov qword ptr [rsp + 16], 0
    mov rbp, MAX_WORDS
.Lcount_words_loop:  # bound: 524289
    mov rdi, r12
    mov rsi, rbx
    mov rdx, r13
    mov ecx, 1
    call skip
    cmp rax, r13
    jae .Lcount_words_done
    mov qword ptr [rsp + 8], rax
    mov rdi, r12
    mov rsi, rax
    mov rdx, r13
    xor ecx, ecx
    call skip
    mov rbx, rax
    mov rdi, r12
    mov rsi, qword ptr [rsp + 8]
    mov rdx, rax
    sub rdx, rsi
    mov rcx, r14
    mov r8, r15
    mov r9, qword ptr [rsp + 16]
    call insert
    mov qword ptr [rsp + 16], rax
    dec rbp
    jnz .Lcount_words_loop
    jmp .Lcount_words_assert_words
.Lcount_words_done:
    mov rax, qword ptr [rsp + 16]
    add rsp, 24
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.Lcount_words_assert_text:
    mov edi, 2
    call assert_fail
.Lcount_words_assert_length:
    mov edi, 3
    call assert_fail
.Lcount_words_assert_words:
    mov edi, 4
    call assert_fail
.size count_words, . - count_words

# Finds where a run of whitespace, or of other bytes, ends.
# In: rdi = the text, rsi = where to start, rdx = its length, at most TEXT_MAX, ecx = 1 to
# pass whitespace, 0 to pass a word.
# Out: rax = the first byte from rsi on that isn't of that kind, or the length.
.type skip, @function
skip:
    cmp rdx, TEXT_MAX
    ja .Lskip_assert_length
    mov rax, rsi
    mov r8, INPUT_LEN
.Lskip_loop:  # bound: 1048577
    cmp rax, rdx
    jae .Lskip_return
    cmp rax, TEXT_MAX
    jae .Lskip_assert_index
    movzx r9d, byte ptr [rdi + rax]
    xor r10d, r10d
    cmp r9d, SPACE
    sete r10b
    sub r9d, TAB
    cmp r9d, TAB_TO_CR
    setbe r11b
    or r10b, r11b
    cmp r10d, ecx
    jne .Lskip_return
    inc rax
    dec r8
    jnz .Lskip_loop
    jmp .Lskip_assert_bound
.Lskip_return:
    ret
.Lskip_assert_length:
    mov edi, 5
    call assert_fail
.Lskip_assert_index:
    mov edi, 6
    call assert_fail
.Lskip_assert_bound:
    mov edi, 7
    call assert_fail
.size skip, . - skip

# Counts one word: once more in its entry, or in a new entry where it's new.
# In: rdi = the text, rsi = the word's start in it, rdx = its length, 1 to TEXT_MAX, rcx = the
# entries, r8 = the index, r9 = how many entries there are, below MAX_WORDS.
# Out: rax = how many entries there are now.
.type insert, @function
insert:
    push rbx
    push r12
    push r13
    push r14
    push r15
    cmp rdx, TEXT_MAX
    ja .Linsert_assert_length
    cmp r9, MAX_WORDS
    jae .Linsert_assert_entries
    mov r12, rdi
    mov r13, rsi
    shl r13, FIELD_BITS
    or r13, rdx
    mov r14, rcx
    mov r15, r8
    mov rbx, r9
    lea rdi, [r12 + rsi]
    mov rsi, rdx
    call fnv
    and rax, SLOT_MASK
    mov rdi, r12
    mov rsi, r13
    mov rdx, r14
    mov rcx, r15
    mov r8, rax
    mov r9, rbx
    call probe
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Linsert_assert_length:
    mov edi, 8
    call assert_fail
.Linsert_assert_entries:
    mov edi, 9
    call assert_fail
.size insert, . - insert

# Finds a word's entry from its slot on, the next slot each time another word holds one, and
# counts it once more there, or gives it a new entry in the first empty slot.
# In: rdi = the text, rsi = the word as an entry holds it, with no count, rdx = the entries,
# rcx = the index, r8 = the slot its hash names, below SLOTS, r9 = how many entries there are.
# Out: rax = how many entries there are now.
.type probe, @function
probe:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8
    mov r12, rdi
    mov r13, rsi
    mov r14, rdx
    mov r15, rcx
    mov rbp, r8
    mov qword ptr [rsp], r9
    mov rbx, SLOTS
.Lprobe_loop:  # bound: 1048576
    cmp rbp, SLOTS
    jae .Lprobe_assert_slot
    mov ecx, dword ptr [r15 + rbp*4]
    test ecx, ecx
    jz .Lprobe_new
    dec ecx
    mov rdi, r12
    mov rsi, r13
    mov rdx, r14
    call same_word
    test rax, rax
    jnz .Lprobe_done
    inc rbp
    and rbp, SLOT_MASK
    dec rbx
    jnz .Lprobe_loop
    jmp .Lprobe_assert_full
.Lprobe_new:
    mov rcx, qword ptr [rsp]
    cmp rcx, MAX_WORDS
    jae .Lprobe_assert_room
    mov rax, ONE_MORE
    or rax, r13
    mov qword ptr [r14 + rcx*8], rax
    lea eax, [rcx + 1]
    cmp rbp, SLOTS
    jae .Lprobe_assert_new_slot
    mov dword ptr [r15 + rbp*4], eax
    inc qword ptr [rsp]
.Lprobe_done:
    mov rax, qword ptr [rsp]
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.Lprobe_assert_slot:
    mov edi, 10
    call assert_fail
.Lprobe_assert_full:
    mov edi, 11
    call assert_fail
.Lprobe_assert_room:
    mov edi, 12
    call assert_fail
.Lprobe_assert_new_slot:
    mov edi, 13
    call assert_fail
.size probe, . - probe

# Says whether an entry is a word's, and if so counts the word once more there.
# In: rdi = the text, rsi = the word as an entry holds it, with no count, rdx = the entries,
# ecx = the entry's number, below MAX_WORDS.
# Out: rax = 1 where the entry is the word's, which now counts it once more, else 0.
.type same_word, @function
same_word:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Lsame_word_assert_text
    mov ecx, ecx
    cmp rcx, MAX_WORDS
    jae .Lsame_word_assert_entry
    mov rbx, rdx
    mov r12, rcx
    mov rax, qword ptr [rbx + rcx*8]
    mov r8, rax
    and r8, FIELD_MASK
    mov r9, rsi
    and r9, FIELD_MASK
    xor r13d, r13d
    cmp r8, r9
    jne .Lsame_word_return
    shr rax, FIELD_BITS
    and rax, FIELD_MASK
    mov rdx, rsi
    shr rdx, FIELD_BITS
    and rdx, FIELD_MASK
    lea rdx, [rdi + rdx]
    lea rdi, [rdi + rax]
    mov rsi, r8
    mov rcx, r9
    call compare_bytes
    test rax, rax
    jnz .Lsame_word_return
    cmp r12, MAX_WORDS
    jae .Lsame_word_assert_count
    mov rax, ONE_MORE
    add qword ptr [rbx + r12*8], rax
    mov r13d, 1
.Lsame_word_return:
    mov rax, r13
    pop r13
    pop r12
    pop rbx
    ret
.Lsame_word_assert_text:
    mov edi, 14
    call assert_fail
.Lsame_word_assert_entry:
    mov edi, 15
    call assert_fail
.Lsame_word_assert_count:
    mov edi, 16
    call assert_fail
.size same_word, . - same_word

# FNV-1a, 64 bits, of some bytes.
# In: rdi = the bytes, rsi = how many, at most TEXT_MAX.
# Out: rax = the hash.
.type fnv, @function
fnv:
    test rdi, rdi
    jz .Lfnv_assert_bytes
    cmp rsi, TEXT_MAX
    ja .Lfnv_assert_length
    mov rax, FNV_OFFSET
    mov r8, FNV_PRIME
    xor ecx, ecx
    mov r9, TEXT_MAX
.Lfnv_loop:  # bound: 1048576
    cmp rcx, rsi
    jae .Lfnv_return
    movzx edx, byte ptr [rdi + rcx]
    xor rax, rdx
    imul rax, r8
    inc rcx
    dec r9
    jnz .Lfnv_loop
.Lfnv_return:
    ret
.Lfnv_assert_bytes:
    mov edi, 17
    call assert_fail
.Lfnv_assert_length:
    mov edi, 18
    call assert_fail
.size fnv, . - fnv

# Compares two runs of bytes in byte order, as a shorter run that starts the other comes first.
# In: rdi = the first, rsi = its length, rdx = the second, rcx = its length, each at most
# TEXT_MAX.
# Out: rax = -1 when the first comes first, 0 when they're the same, 1 when the second does.
.type compare_bytes, @function
compare_bytes:
    test rdi, rdi
    jz .Lcompare_bytes_assert_first
    test rdx, rdx
    jz .Lcompare_bytes_assert_second
    cmp rsi, TEXT_MAX
    ja .Lcompare_bytes_assert_first_length
    cmp rcx, TEXT_MAX
    ja .Lcompare_bytes_assert_second_length
    mov r8, rsi
    cmp r8, rcx
    cmova r8, rcx
    xor r9d, r9d
    mov r10, TEXT_MAX
.Lcompare_bytes_loop:  # bound: 1048576
    cmp r9, r8
    jae .Lcompare_bytes_lengths
    movzx eax, byte ptr [rdi + r9]
    movzx r11d, byte ptr [rdx + r9]
    cmp eax, r11d
    jne .Lcompare_bytes_differ
    inc r9
    dec r10
    jnz .Lcompare_bytes_loop
.Lcompare_bytes_lengths:
    mov eax, esi
    mov r11d, ecx
.Lcompare_bytes_differ:
    xor r8d, r8d
    cmp eax, r11d
    mov eax, -1
    mov ecx, 1
    cmova eax, ecx
    cmove eax, r8d
    movsxd rax, eax
    ret
.Lcompare_bytes_assert_first:
    mov edi, 19
    call assert_fail
.Lcompare_bytes_assert_second:
    mov edi, 20
    call assert_fail
.Lcompare_bytes_assert_first_length:
    mov edi, 21
    call assert_fail
.Lcompare_bytes_assert_second_length:
    mov edi, 22
    call assert_fail
.size compare_bytes, . - compare_bytes

# Says which of two entries comes first: the more frequent, or with the same count, the word
# first in byte order.
# In: rdi = the first entry, rsi = the second, rdx = the text their words are in.
# Out: rax = negative when the first comes first, 0 when they're the same, positive when the
# second does.
.type compare_entries, @function
compare_entries:
    sub rsp, 8
    test rdx, rdx
    jz .Lcompare_entries_assert_text
    mov rax, rdi
    shr rax, COUNT_SHIFT
    jz .Lcompare_entries_assert_count
    mov rcx, rsi
    shr rcx, COUNT_SHIFT
    cmp rax, rcx
    mov rax, -1
    ja .Lcompare_entries_return
    mov rax, 1
    jb .Lcompare_entries_return
    mov r8, rdi
    shr r8, FIELD_BITS
    and r8, FIELD_MASK
    mov r9, rsi
    shr r9, FIELD_BITS
    and r9, FIELD_MASK
    mov rcx, rsi
    and rcx, FIELD_MASK
    mov rsi, rdi
    and rsi, FIELD_MASK
    lea rdi, [rdx + r8]
    lea rdx, [rdx + r9]
    call compare_bytes
.Lcompare_entries_return:
    add rsp, 8
    ret
.Lcompare_entries_assert_text:
    mov edi, 23
    call assert_fail
.Lcompare_entries_assert_count:
    mov edi, 24
    call assert_fail
.size compare_entries, . - compare_entries

# Merges the sorted runs src[lo, mid) and src[mid, hi) into dst[lo, hi).
# In: rdi = src, rsi = dst, rdx = lo, rcx = mid, r8 = hi, r9 = the text the entries' words are
# in; lo <= mid <= hi.
.type merge, @function
merge:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 40
    cmp rdx, rcx
    ja .Lmerge_assert_order
    lea rbx, [rdi + rcx*8]
    mov qword ptr [rsp], rbx
    lea rbx, [rdi + r8*8]
    mov qword ptr [rsp + 8], rbx
    mov rax, r8
    sub rax, rdx
    mov qword ptr [rsp + 16], rax
    mov qword ptr [rsp + 24], r9
    lea r13, [rdi + rdx*8]
    lea r14, [rdi + rcx*8]
    lea r15, [rsi + rdx*8]
    test rax, rax
    jz .Lmerge_done
    mov rbp, MAX_WORDS
.Lmerge_loop:  # bound: 524289
    cmp r13, qword ptr [rsp]
    jae .Lmerge_take_right
    cmp r14, qword ptr [rsp + 8]
    jae .Lmerge_take_left
    mov rdi, qword ptr [r13]
    mov rsi, qword ptr [r14]
    mov rdx, qword ptr [rsp + 24]
    call compare_entries
    test rax, rax
    jg .Lmerge_take_right
.Lmerge_take_left:
    mov rax, qword ptr [r13]
    add r13, 8
    jmp .Lmerge_store
.Lmerge_take_right:
    mov rax, qword ptr [r14]
    add r14, 8
.Lmerge_store:
    mov qword ptr [r15], rax
    add r15, 8
    dec qword ptr [rsp + 16]
    jz .Lmerge_done
    dec rbp
    jnz .Lmerge_loop
    jmp .Lmerge_assert_bound
.Lmerge_done:
    add rsp, 40
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.Lmerge_assert_order:
    mov edi, 25
    call assert_fail
.Lmerge_assert_bound:
    mov edi, 26
    call assert_fail
.size merge, . - merge

# Merges each pair of neighboring sorted runs of a given width from src into dst.
# In: rdi = src, rsi = dst, rdx = how many entries, rcx = the run width, at least 1, r8 = the text the entries' words are in.
.type merge_pass, @function
merge_pass:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8
    test rcx, rcx
    jz .Lmerge_pass_assert_width
    mov rbx, rdi
    mov r12, rsi
    mov r13, rdx
    mov r14, rcx
    mov r15, r8
    mov qword ptr [rsp], 0
    mov rbp, MAX_WORDS
.Lmerge_pass_loop:  # bound: 524289
    mov rdx, qword ptr [rsp]
    cmp rdx, r13
    jae .Lmerge_pass_done
    lea rcx, [rdx + r14]
    cmp rcx, r13
    cmova rcx, r13
    lea r8, [rcx + r14]
    cmp r8, r13
    cmova r8, r13
    mov qword ptr [rsp], r8
    mov rdi, rbx
    mov rsi, r12
    mov r9, r15
    call merge
    dec rbp
    jnz .Lmerge_pass_loop
    jmp .Lmerge_pass_assert_bound
.Lmerge_pass_done:
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.Lmerge_pass_assert_width:
    mov edi, 27
    call assert_fail
.Lmerge_pass_assert_bound:
    mov edi, 28
    call assert_fail
.size merge_pass, . - merge_pass

# Sorts entries with a bottom-up merge sort, back and forth between two arrays.
# In: rdi = the entries, rsi = an array as large, rdx = how many, rcx = the text their words are in.
# Out: rax = whichever of the two arrays holds the sorted entries.
.type merge_sort, @function
merge_sort:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8
    test rdi, rdi
    jz .Lmerge_sort_assert_null
    cmp rdx, MAX_WORDS
    ja .Lmerge_sort_assert_count
    mov rbx, rdi
    mov r12, rsi
    mov r13, rdx
    mov r14d, 1
    mov r15, rcx
    mov rbp, MAX_PASSES
.Lmerge_sort_loop:  # bound: 21
    cmp r14, r13
    jae .Lmerge_sort_done
    mov rdi, rbx
    mov rsi, r12
    mov rdx, r13
    mov rcx, r14
    mov r8, r15
    call merge_pass
    mov rax, rbx
    mov rbx, r12
    mov r12, rax
    shl r14, 1
    dec rbp
    jnz .Lmerge_sort_loop
    jmp .Lmerge_sort_assert_bound
.Lmerge_sort_done:
    mov rax, rbx
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.Lmerge_sort_assert_null:
    mov edi, 29
    call assert_fail
.Lmerge_sort_assert_count:
    mov edi, 30
    call assert_fail
.Lmerge_sort_assert_bound:
    mov edi, 31
    call assert_fail
.size merge_sort, . - merge_sort

# Prints each entry's line: its count, a space, its word and a newline, a buffer-full at a time.
# In: rdi = the entries, rsi = how many, at most MAX_WORDS, rdx = the text their words are in,
# rcx = the buffer to print from, OUT_LEN bytes.
# Out: rax = 0, or a negative error code.
# returns: result
.type emit, @function
emit:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8
    test rdi, rdi
    jz .Lemit_assert_entries
    cmp rsi, MAX_WORDS
    ja .Lemit_assert_count
    mov r12, rdi
    mov r13, rsi
    mov r14, rdx
    mov r15, rcx
    mov qword ptr [rsp], 0
    xor ebx, ebx
    mov rbp, MAX_WORDS
.Lemit_loop:  # bound: 524289
    cmp rbx, r13
    jae .Lemit_rest
    cmp rbx, MAX_WORDS
    jae .Lemit_assert_index
    mov rdi, qword ptr [r12 + rbx*8]
    mov rsi, r14
    mov rdx, r15
    mov rcx, qword ptr [rsp]
    call emit_line
    test rax, rax
    js .Lemit_return
    mov qword ptr [rsp], rax
    inc rbx
    dec rbp
    jnz .Lemit_loop
    jmp .Lemit_assert_bound
.Lemit_rest:
    xor eax, eax
    mov rdx, qword ptr [rsp]
    test rdx, rdx
    jz .Lemit_return
    mov edi, STDOUT
    mov rsi, r15
    call write_all
    test rax, rax
    js .Lemit_return
    xor eax, eax
.Lemit_return:
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.Lemit_assert_entries:
    mov edi, 32
    call assert_fail
.Lemit_assert_count:
    mov edi, 33
    call assert_fail
.Lemit_assert_index:
    mov edi, 34
    call assert_fail
.Lemit_assert_bound:
    mov edi, 35
    call assert_fail
.size emit, . - emit

# Adds an entry's line to the buffer, printing what the buffer holds first where the line
# doesn't fit after it.
# In: rdi = the entry, rsi = the text its word is in, rdx = the buffer, OUT_LEN bytes, rcx = how
# many bytes of it are taken.
# Out: rax = how many are taken now, or a negative error code.
# returns: result
.type emit_line, @function
emit_line:
    push rbx
    push r12
    push r13
    push r14
    sub rsp, 8
    test rsi, rsi
    jz .Lemit_line_assert_text
    mov rbx, rdi
    mov r12, rsi
    mov r13, rdx
    mov r14, rcx
    mov rax, rbx
    and rax, FIELD_MASK
    add rax, LINE_EXTRA
    add rax, r14
    cmp rax, OUT_LEN
    jbe .Lemit_line_put
    test r14, r14
    jz .Lemit_line_assert_room
    mov edi, STDOUT
    mov rsi, r13
    mov rdx, r14
    call write_all
    test rax, rax
    js .Lemit_line_return
    xor r14d, r14d
.Lemit_line_put:
    mov rdi, rbx
    mov rsi, r12
    mov rdx, r13
    mov rcx, r14
    call put_line
.Lemit_line_return:
    add rsp, 8
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Lemit_line_assert_text:
    mov edi, 36
    call assert_fail
.Lemit_line_assert_room:
    mov edi, 37
    call assert_fail
.size emit_line, . - emit_line

# Writes an entry's line into the buffer: its count, a space, its word and a newline.
# In: rdi = the entry, rsi = the text its word is in, rdx = the buffer, OUT_LEN bytes, rcx = how
# many bytes of it are taken, with room for the line after them.
# Out: rax = how many are taken now.
.type put_line, @function
put_line:
    push rbx
    push r12
    push r13
    push r14
    sub rsp, 8
    mov rbx, rdi
    mov r12, rsi
    mov r13, rdx
    mov rdi, rbx
    shr rdi, COUNT_SHIFT
    mov rsi, r13
    mov rdx, rcx
    call put_number
    mov r14, rax
    cmp r14, OUT_LEN
    jae .Lput_line_assert_space
    mov byte ptr [r13 + r14], SPACE
    inc r14
    mov rax, rbx
    shr rax, FIELD_BITS
    and rax, FIELD_MASK
    mov rcx, rbx
    and rcx, FIELD_MASK
    xor edx, edx
    mov r8, TEXT_MAX
.Lput_line_copy:  # bound: 1048576
    cmp rdx, rcx
    jae .Lput_line_end
    lea r9, [rax + rdx]
    cmp r9, TEXT_MAX
    jae .Lput_line_assert_word
    movzx r10d, byte ptr [r12 + r9]
    cmp r14, OUT_LEN
    jae .Lput_line_assert_byte
    mov byte ptr [r13 + r14], r10b
    inc r14
    inc rdx
    dec r8
    jnz .Lput_line_copy
.Lput_line_end:
    cmp r14, OUT_LEN
    jae .Lput_line_assert_newline
    mov byte ptr [r13 + r14], NEWLINE
    lea rax, [r14 + 1]
    add rsp, 8
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Lput_line_assert_space:
    mov edi, 38
    call assert_fail
.Lput_line_assert_word:
    mov edi, 39
    call assert_fail
.Lput_line_assert_byte:
    mov edi, 40
    call assert_fail
.Lput_line_assert_newline:
    mov edi, 41
    call assert_fail
.size put_line, . - put_line

# Writes a whole number's decimal digits into a buffer.
# In: rdi = the number, rsi = the buffer, OUT_LEN bytes, rdx = where in it the digits start.
# Out: rax = where they end.
.type put_number, @function
put_number:
    mov r10, rdx
    mov rax, rdi
    mov r8d, 10
    xor ecx, ecx
    mov r9, MAX_DIGITS
.Lput_number_count:  # bound: 20
    xor edx, edx
    div r8
    inc rcx
    test rax, rax
    jz .Lput_number_write
    dec r9
    jnz .Lput_number_count
    jmp .Lput_number_assert_counted
.Lput_number_write:
    lea r11, [r10 + rcx]
    mov rax, rdi
    mov r9, MAX_DIGITS
.Lput_number_digit:  # bound: 20
    xor edx, edx
    div r8
    add edx, '0'
    dec r11
    cmp r11, OUT_LEN
    jae .Lput_number_assert_index
    mov byte ptr [rsi + r11], dl
    test rax, rax
    jz .Lput_number_done
    dec r9
    jnz .Lput_number_digit
    jmp .Lput_number_assert_written
.Lput_number_done:
    lea rax, [r10 + rcx]
    ret
.Lput_number_assert_counted:
    mov edi, 42
    call assert_fail
.Lput_number_assert_index:
    mov edi, 43
    call assert_fail
.Lput_number_assert_written:
    mov edi, 44
    call assert_fail
.size put_number, . - put_number

# Writes rdx bytes from rsi to file descriptor edi, retrying partial writes.
# In: edi = the file descriptor, rsi = the bytes, rdx = how many, at least 1.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_all, @function
write_all:
    test rsi, rsi
    jz .Lwrite_all_assert_null
    test rdx, rdx
    jz .Lwrite_all_assert_empty
    mov r8, MAX_WRITES
.Lwrite_all_loop:  # bound: 64
    mov eax, SYS_WRITE
    syscall
    test rax, rax
    js .Lwrite_all_return
    cmp rax, rdx
    ja .Lwrite_all_assert_count
    add rsi, rax
    sub rdx, rax
    jz .Lwrite_all_done
    dec r8
    jnz .Lwrite_all_loop
    mov rax, ERROR_IO
    jmp .Lwrite_all_return
.Lwrite_all_done:
    xor eax, eax
.Lwrite_all_return:
    ret
.Lwrite_all_assert_null:
    mov edi, 45
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 46
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 47
    call assert_fail
.size write_all, . - write_all

# Reads from file descriptor edi into the rdx bytes at rsi until they're full, the input ends,
# or, when ecx is a byte (0 to 255), the last byte read is ecx; with ecx = -1 it reads to the
# end. However the input arrives, it reads the same.
# In: edi = the file descriptor, rsi = the buffer, rdx = its size, at least 1, ecx = the end
# byte or -1.
# Out: rax = how many bytes it read, or a negative error code.
# returns: result
.type fill, @function
fill:
    test rsi, rsi
    jz .Lfill_assert_null
    test rdx, rdx
    jz .Lfill_assert_empty
    mov r8, rdx
    mov r10d, ecx
    mov r9, MAX_FILL_READS
.Lfill_loop:  # bound: 1099511627776
    mov eax, SYS_READ
    syscall
    test rax, rax
    js .Lfill_return
    jz .Lfill_done
    cmp rax, rdx
    ja .Lfill_assert_count
    add rsi, rax
    sub rdx, rax
    jz .Lfill_done
    movzx eax, byte ptr [rsi - 1]
    cmp eax, r10d
    je .Lfill_done
    dec r9
    jnz .Lfill_loop
    mov rax, ERROR_IO
    jmp .Lfill_return
.Lfill_done:
    mov rax, r8
    sub rax, rdx
.Lfill_return:
    ret
.Lfill_assert_null:
    mov edi, 990
    call assert_fail
.Lfill_assert_empty:
    mov edi, 991
    call assert_fail
.Lfill_assert_count:
    mov edi, 992
    call assert_fail
.size fill, . - fill

# lengua's assertion handler. check_code compares it word for word: copy it exactly.
# In: edi = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
# standard error and ends the program, every thread of it, with code 134.
.type assert_fail, @function
assert_fail:
    sub rsp, 40
    mov rax, qword ptr [rip + assert_message]
    mov qword ptr [rsp], rax
    mov rax, qword ptr [rip + assert_message + 8]
    mov qword ptr [rsp + 8], rax
    mov rax, qword ptr [rip + assert_message + 16]
    mov qword ptr [rsp + 16], rax
    mov eax, dword ptr [rip + assert_message + 24]
    mov dword ptr [rsp + 24], eax
    mov al, byte ptr [rip + assert_message + 28]
    mov byte ptr [rsp + 28], al
    mov eax, edi
    xor edx, edx
    mov ecx, 100
    div ecx
    add al, 48
    mov byte ptr [rsp + 18], al
    mov eax, edx
    xor edx, edx
    mov ecx, 10
    div ecx
    add al, 48
    mov byte ptr [rsp + 19], al
    add dl, 48
    mov byte ptr [rsp + 20], dl
    mov eax, 1
    mov edi, 2
    mov rsi, rsp
    mov edx, 29
    syscall
    mov eax, 231
    mov edi, 134
    syscall
.size assert_fail, . - assert_fail
