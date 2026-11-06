.intel_syntax noprefix
# lengua: compiled from idea.md sha256:18c418b79b3b41601fc38372960981cbc647e824ae92ac032f90981b09bc8eb8 for linux-x86_64

# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
.equ STDERR, 2
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_WRITE_FAILED, 2
.equ EXIT_READ_FAILED, 3
.equ EXIT_TOO_LONG, 4
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
# Argument counts, the program's name included
.equ ARGC_PLAIN, 1
.equ ARGC_OPTION, 2
# Bytes that matter
.equ NEWLINE, 10
.equ READ_TO_END, -1
# Sizes and limits
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

# Standard input, one byte past the limit so a longer input shows, and room for a final newline,
# then 8 spare bytes so comparing lines eight bytes at a time stays inside.
.type in_buf, @object
in_buf:
    .zero IN_BUF_LEN
.size in_buf, . - in_buf

# Each line as a qword: its start in in_buf in the low 32 bits, its length in the high 32.
.type lines_a, @object
lines_a:
    .zero LINES_LEN
.size lines_a, . - lines_a

# The merge sort's second array of lines.
.type lines_b, @object
lines_b:
    .zero LINES_LEN
.size lines_b, . - lines_b

# The sorted lines, each with its newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
# Checks the arguments, reads standard input, sorts its lines and prints them.
.type main, @function
main:
    sub rsp, 8
    xor r13d, r13d
    cmp rdi, ARGC_PLAIN
    je .Lmain_read
    cmp rdi, ARGC_OPTION
    jne .Lmain_usage
    mov rdi, qword ptr [rsi + 8]
    test rdi, rdi
    jz .Lmain_assert_argv
    call is_unique_option
    test rax, rax
    js .Lmain_usage
    mov r13d, 1
.Lmain_read:
    mov edi, STDIN
    lea rsi, [rip + in_buf]
    mov edx, TEXT_MAX
    mov ecx, READ_TO_END
    call fill
    test rax, rax
    js .Lmain_read_failed
    cmp rax, MAX_INPUT
    ja .Lmain_too_long
    mov edi, EXIT_OK
    test rax, rax
    jz .Lmain_exit
    lea rbx, [rip + in_buf]
    mov rdi, rbx
    mov rsi, rax
    call end_last_line
    mov rdi, rbx
    mov rsi, rax
    mov edx, r13d
    call sort_text
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_usage:
    mov edi, STDERR
    lea rsi, [rip + usage_msg]
    mov edx, USAGE_MSG_LEN
    call write_all
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_USAGE
    jmp .Lmain_exit
.Lmain_too_long:
    mov edi, STDERR
    lea rsi, [rip + too_long_msg]
    mov edx, TOO_LONG_MSG_LEN
    call write_all
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_TOO_LONG
    jmp .Lmain_exit
.Lmain_read_failed:
    mov edi, EXIT_READ_FAILED
    jmp .Lmain_exit
.Lmain_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_argv:
    mov edi, 1
    call assert_fail
.size main, . - main

# Gives a text's last line a newline when it lacks one.
# In: rdi = the text, with room for TEXT_MAX bytes, rsi = its length, 1 to MAX_INPUT.
# Out: rax = its new length.
.type end_last_line, @function
end_last_line:
    test rsi, rsi
    jz .Lend_assert_empty
    mov rax, rsi
    lea rcx, [rsi - 1]
    cmp rcx, TEXT_MAX
    jae .Lend_assert_last
    cmp byte ptr [rdi + rcx], NEWLINE
    je .Lend_return
    cmp rsi, TEXT_MAX
    jae .Lend_assert_append
    mov byte ptr [rdi + rsi], NEWLINE
    inc rax
.Lend_return:
    ret
.Lend_assert_empty:
    mov edi, 2
    call assert_fail
.Lend_assert_last:
    mov edi, 3
    call assert_fail
.Lend_assert_append:
    mov edi, 12
    call assert_fail
.size end_last_line, . - end_last_line

# Tells whether an argument is exactly "-u".
# In: rdi = a NUL-terminated string.
# Out: rax = 0 when it is, or ERROR_INVALID.
# returns: result
.type is_unique_option, @function
is_unique_option:
    test rdi, rdi
    jz .Lis_unique_assert_null
    mov rax, ERROR_INVALID
    cmp byte ptr [rdi], '-'
    jne .Lis_unique_return
    cmp byte ptr [rdi + 1], 'u'
    jne .Lis_unique_return
    cmp byte ptr [rdi + 2], 0
    jne .Lis_unique_return
    xor eax, eax
.Lis_unique_return:
    ret
.Lis_unique_assert_null:
    mov edi, 28
    call assert_fail
.size is_unique_option, . - is_unique_option

# Sorts the lines of a text, keeps one of each when asked, and writes them to standard output.
# In: rdi = the text, every line ending with a newline, rsi = its length, 1 to TEXT_MAX,
# edx = 1 to drop duplicate lines, else 0.
# Out: rax = 0, or a negative error code.
# returns: result
.type sort_text, @function
sort_text:
    push rbx
    push r12
    push r13
    push r14
    push r15
    test rdi, rdi
    jz .Lsort_text_assert_null
    cmp rsi, TEXT_MAX
    ja .Lsort_text_assert_length
    mov rbx, rdi
    mov r13d, edx
    lea rdx, [rip + lines_a]
    call split_lines
    mov r14, rax
    lea rdi, [rip + lines_a]
    lea rsi, [rip + lines_b]
    mov rdx, r14
    mov rcx, rbx
    call merge_sort
    mov r15, rax
    test r13d, r13d
    jz .Lsort_text_emit
    mov rdi, r15
    mov rsi, r14
    mov rdx, rbx
    call dedupe
    mov r14, rax
.Lsort_text_emit:
    mov rdi, r15
    mov rsi, r14
    mov rdx, rbx
    lea rcx, [rip + out_buf]
    call emit_lines
    mov rdx, rax
    xor eax, eax
    test rdx, rdx
    jz .Lsort_text_return
    mov edi, STDOUT
    lea rsi, [rip + out_buf]
    call write_all
    test rax, rax
    js .Lsort_text_return
    xor eax, eax
.Lsort_text_return:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Lsort_text_assert_null:
    mov edi, 29
    call assert_fail
.Lsort_text_assert_length:
    mov edi, 30
    call assert_fail
.size sort_text, . - sort_text

# Splits text whose every line ends with a newline into line entries.
# In: rdi = the text, rsi = its length, 1 to TEXT_MAX, rdx = room for MAX_LINES entries.
# Out: rax = how many lines.
.type split_lines, @function
split_lines:
    test rdi, rdi
    jz .Lsplit_assert_null
    xor eax, eax
    xor ecx, ecx
    xor r9d, r9d
    mov r8, TEXT_MAX
.Lsplit_loop:  # bound: 1048577
    cmp rcx, TEXT_MAX
    jae .Lsplit_assert_index
    cmp byte ptr [rdi + rcx], NEWLINE
    jne .Lsplit_next
    cmp rax, MAX_LINES
    jae .Lsplit_assert_lines
    mov r10, rcx
    sub r10, r9
    shl r10, 32
    or r10, r9
    mov qword ptr [rdx + rax*8], r10
    inc rax
    lea r9, [rcx + 1]
.Lsplit_next:
    inc rcx
    cmp rcx, rsi
    jae .Lsplit_return
    dec r8
    jnz .Lsplit_loop
    jmp .Lsplit_assert_bound
.Lsplit_return:
    ret
.Lsplit_assert_null:
    mov edi, 4
    call assert_fail
.Lsplit_assert_index:
    mov edi, 5
    call assert_fail
.Lsplit_assert_lines:
    mov edi, 6
    call assert_fail
.Lsplit_assert_bound:
    mov edi, 7
    call assert_fail
.size split_lines, . - split_lines

# Compares two lines in byte order, a shorter line first when it starts the other, eight
# bytes at a time; the text has 8 spare bytes past its end, so the last load stays inside.
# In: rdi = the first line's entry, rsi = the second's, rdx = the text they're in.
# Out: rax negative, zero or positive as the first line sorts before, equal to, or after.
.type compare_entries, @function
compare_entries:
    mov r8, rdi
    shr r8, 32
    mov r9, rsi
    shr r9, 32
    mov eax, edi
    lea rdi, [rdx + rax]
    mov eax, esi
    lea rsi, [rdx + rax]
    mov r10, r8
    cmp r10, r9
    cmova r10, r9
    xor ecx, ecx
    mov r11, MAX_WORDS
.Lcompare_loop:  # bound: 131073
    lea rax, [rcx + WORD_BYTES]
    cmp rax, r10
    ja .Lcompare_tail
    cmp rcx, MAX_INPUT
    jae .Lcompare_assert_index
    mov rax, qword ptr [rdi + rcx]
    mov rdx, qword ptr [rsi + rcx]
    cmp rax, rdx
    jne .Lcompare_words
    add rcx, WORD_BYTES
    dec r11
    jnz .Lcompare_loop
    jmp .Lcompare_assert_bound
.Lcompare_words:
    bswap rax
    bswap rdx
    jmp .Lcompare_ordered
.Lcompare_tail:
    mov rax, r10
    sub rax, rcx
    jz .Lcompare_lengths
    cmp rcx, MAX_INPUT
    jae .Lcompare_assert_tail
    mov r11, qword ptr [rdi + rcx]
    mov rdx, qword ptr [rsi + rcx]
    shl eax, 3
    mov ecx, 64
    sub ecx, eax
    bswap r11
    bswap rdx
    shr r11, cl
    shr rdx, cl
    mov rax, r11
    cmp rax, rdx
    je .Lcompare_lengths
.Lcompare_ordered:
    mov rcx, -1
    cmp rax, rdx
    mov eax, 1
    cmovb rax, rcx
    jmp .Lcompare_return
.Lcompare_lengths:
    mov rax, r8
    sub rax, r9
.Lcompare_return:
    ret
.Lcompare_assert_index:
    mov edi, 9
    call assert_fail
.Lcompare_assert_bound:
    mov edi, 10
    call assert_fail
.Lcompare_assert_tail:
    mov edi, 31
    call assert_fail
.size compare_entries, . - compare_entries

# Merges the sorted runs src[lo, mid) and src[mid, hi) into dst[lo, hi).
# In: rdi = src, rsi = dst, rdx = lo, rcx = mid, r8 = hi, r9 = the text; lo <= mid <= hi.
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
    mov rbp, MAX_LINES
.Lmerge_loop:  # bound: 1048577
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
    mov edi, 11
    call assert_fail
.Lmerge_assert_bound:
    mov edi, 13
    call assert_fail
.size merge, . - merge

# Merges each pair of neighboring sorted runs of a given width from src into dst.
# In: rdi = src, rsi = dst, rdx = how many entries, rcx = the run width, at least 1, r8 = the text.
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
    mov rbp, MAX_LINES
.Lmerge_pass_loop:  # bound: 1048577
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
    mov edi, 14
    call assert_fail
.Lmerge_pass_assert_bound:
    mov edi, 15
    call assert_fail
.size merge_pass, . - merge_pass

# Sorts line entries with a bottom-up merge sort, back and forth between two arrays.
# In: rdi = the entries, rsi = an array as large, rdx = how many, rcx = the text.
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
    cmp rdx, MAX_LINES
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
    mov edi, 16
    call assert_fail
.Lmerge_sort_assert_count:
    mov edi, 17
    call assert_fail
.Lmerge_sort_assert_bound:
    mov edi, 18
    call assert_fail
.size merge_sort, . - merge_sort

# Keeps the first of each run of equal sorted lines, in place.
# In: rdi = the sorted entries, rsi = how many, rdx = the text.
# Out: rax = how many are kept.
.type dedupe, @function
dedupe:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8
    xor r15d, r15d
    test rsi, rsi
    jz .Ldedupe_done
    cmp rsi, MAX_LINES
    ja .Ldedupe_assert_count
    mov rbx, rdx
    mov r13, rdi
    lea r12, [rdi + 8]
    lea r14, [rsi - 1]
    mov r15d, 1
    mov rbp, MAX_LINES
.Ldedupe_loop:  # bound: 1048577
    test r14, r14
    jz .Ldedupe_done
    mov rdi, qword ptr [r13]
    mov rsi, qword ptr [r12]
    mov rdx, rbx
    call compare_entries
    test rax, rax
    jz .Ldedupe_skip
    mov rax, qword ptr [r12]
    add r13, 8
    mov qword ptr [r13], rax
    inc r15
.Ldedupe_skip:
    add r12, 8
    dec r14
    dec rbp
    jnz .Ldedupe_loop
    jmp .Ldedupe_assert_bound
.Ldedupe_done:
    mov rax, r15
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.Ldedupe_assert_count:
    mov edi, 19
    call assert_fail
.Ldedupe_assert_bound:
    mov edi, 20
    call assert_fail
.size dedupe, . - dedupe

# Copies the lines in order into the output, each followed by a newline.
# In: rdi = the entries, rsi = how many, rdx = the text, rcx = OUT_BUF_LEN bytes of output.
# Out: rax = the output's length.
.type emit_lines, @function
emit_lines:
    push rbx
    xor eax, eax
    test rsi, rsi
    jz .Lemit_return
    mov r8, MAX_LINES
.Lemit_line:  # bound: 1048577
    mov r10, qword ptr [rdi]
    mov r11d, r10d
    shr r10, 32
    lea r11, [rdx + r11]
    test r10, r10
    jz .Lemit_newline
    mov r9, MAX_INPUT
.Lemit_byte:  # bound: 1048576
    cmp rax, OUT_BUF_LEN
    jae .Lemit_assert_byte
    movzx ebx, byte ptr [r11]
    mov byte ptr [rcx + rax], bl
    inc r11
    inc rax
    dec r10
    jz .Lemit_newline
    dec r9
    jnz .Lemit_byte
    jmp .Lemit_assert_line_bound
.Lemit_newline:
    cmp rax, OUT_BUF_LEN
    jae .Lemit_assert_newline
    mov byte ptr [rcx + rax], NEWLINE
    inc rax
    add rdi, 8
    dec rsi
    jz .Lemit_return
    dec r8
    jnz .Lemit_line
    jmp .Lemit_assert_lines_bound
.Lemit_return:
    pop rbx
    ret
.Lemit_assert_byte:
    mov edi, 21
    call assert_fail
.Lemit_assert_line_bound:
    mov edi, 22
    call assert_fail
.Lemit_assert_newline:
    mov edi, 23
    call assert_fail
.Lemit_assert_lines_bound:
    mov edi, 24
    call assert_fail
.size emit_lines, . - emit_lines

# Writes rdx bytes from rsi to file descriptor edi, retrying partial writes.
# In: edi = the file descriptor, rsi = the bytes, rdx = how many, at least 1.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_all, @function
write_all:
    sub rsp, 8
    test rsi, rsi
    jz .Lwrite_all_assert_null
    test rdx, rdx
    jz .Lwrite_all_assert_empty
    mov r8, MAX_WRITES
.Lwrite_all_loop:  # bound: 64
    call os_write
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
    add rsp, 8
    ret
.Lwrite_all_assert_null:
    mov edi, 25
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 26
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 27
    call assert_fail
.size write_all, . - write_all

