.intel_syntax noprefix

# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
.equ STDERR, 2
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_NO_COLUMN, 1
.equ EXIT_BAD_AMOUNT, 2
.equ EXIT_MISSING_AMOUNT, 3
.equ EXIT_IO_FAILED, 4
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_NOT_FOUND, -2
.equ ERROR_BAD, -22
.equ ERROR_MISSING, -61
# Sizes and limits
.equ MAX_INPUT, 1048576
.equ IN_BUF_LEN, MAX_INPUT + 1
.equ SCAN_LIMIT, MAX_INPUT + 1
.equ MAX_READS, MAX_INPUT + 1
.equ MAX_LINES, MAX_INPUT
.equ LINES_BASE, MAX_LINES + 2
.equ MAX_WHOLE_DIGITS, 6
.equ WHOLE_SCAN, MAX_WHOLE_DIGITS + 1
.equ FRACTION_DIGITS, 2
.equ MAX_DIGITS, 19
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ AMOUNT_LEN, 6
.equ AMOUNT_HEAD, 0x756f6d61
.equ AMOUNT_TAIL, 0x746e
.equ FIRST_ROW_LINE, 2
.equ NO_COLUMN_MSG_LEN, 24
.equ BAD_MSG_LEN, 26
.equ MISSING_MSG_LEN, 30
.equ OUT_BUF_LEN, 21
.equ CENTS_BUF_LEN, 23
.equ CENTS_POINT, CENTS_BUF_LEN - 4

.section .rodata

.type no_column_msg, @object
no_column_msg:
    .ascii "total: no amount column\n"
.size no_column_msg, . - no_column_msg

.type bad_msg, @object
bad_msg:
    .ascii "total: bad amount on line "
.size bad_msg, . - bad_msg

.type missing_msg, @object
missing_msg:
    .ascii "total: missing amount on line "
.size missing_msg, . - missing_msg

.section .bss

# All of standard input, plus one byte to notice input that is too long.
.type in_buf, @object
in_buf:
    .zero IN_BUF_LEN
.size in_buf, . - in_buf

# Sign, 19 digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

# Sign, 19 digits, a point and a newline, with a spare byte.
.type cents_buf, @object
cents_buf:
    .zero CENTS_BUF_LEN
.size cents_buf, . - cents_buf

.text
# Reads the CSV, finds the amount column, totals it and prints the total or an error.
.type main, @function
main:
    sub rsp, 8
    lea rbx, [rip + in_buf]
    mov rdi, rbx
    mov esi, IN_BUF_LEN
    call read_input
    test rax, rax
    js .Lmain_io_failed
    mov r12, rdx
    mov rdi, rbx
    mov rsi, r12
    call find_column
    test rax, rax
    js .Lmain_no_column
    mov rdi, rbx
    mov rsi, r12
    call sum_rows
    test rax, rax
    js .Lmain_row_error
    mov rdi, rdx
    call write_cents
    test rax, rax
    js .Lmain_io_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_no_column:
    mov edi, STDERR
    lea rsi, [rip + no_column_msg]
    mov edx, NO_COLUMN_MSG_LEN
    call write_all
    test rax, rax
    js .Lmain_io_failed
    mov edi, EXIT_NO_COLUMN
    jmp .Lmain_exit
.Lmain_row_error:
    cmp rax, ERROR_BAD
    je .Lmain_bad
    cmp rax, ERROR_MISSING
    je .Lmain_missing
    jmp .Lmain_assert_error
.Lmain_bad:
    lea rdi, [rip + bad_msg]
    mov esi, BAD_MSG_LEN
    call report_line
    test rax, rax
    js .Lmain_io_failed
    mov edi, EXIT_BAD_AMOUNT
    jmp .Lmain_exit
.Lmain_missing:
    lea rdi, [rip + missing_msg]
    mov esi, MISSING_MSG_LEN
    call report_line
    test rax, rax
    js .Lmain_io_failed
    mov edi, EXIT_MISSING_AMOUNT
    jmp .Lmain_exit
.Lmain_io_failed:
    mov edi, EXIT_IO_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_error:
    mov edi, 1
    call assert_fail
.size main, . - main

# Reads all of standard input into a buffer.
# In: rdi = the buffer, rsi = its size, one more than the largest input.
# Out: rax = 0 and rdx = the bytes read, or a negative error code (also when the input fills
# the buffer).
# returns: result
.type read_input, @function
read_input:
    push rbx
    test rdi, rdi
    jz .Lread_assert_null
    cmp rsi, IN_BUF_LEN
    ja .Lread_assert_size
    mov r9, rdi
    mov r10, rsi
    xor r8d, r8d
    mov rbx, MAX_READS
.Lread_loop:  # bound: 1048577
    mov rdx, r10
    sub rdx, r8
    jz .Lread_too_long
    mov edi, STDIN
    lea rsi, [r9 + r8]
    call os_read
    test rax, rax
    js .Lread_return
    jz .Lread_done
    add r8, rax
    cmp r8, r10
    ja .Lread_assert_count
    dec rbx
    jnz .Lread_loop
    jmp .Lread_assert_reads
.Lread_too_long:
    mov rax, ERROR_IO
    jmp .Lread_return
.Lread_done:
    mov rdx, r8
    xor eax, eax
.Lread_return:
    pop rbx
    ret
.Lread_assert_null:
    mov edi, 2
    call assert_fail
.Lread_assert_size:
    mov edi, 3
    call assert_fail
.Lread_assert_count:
    mov edi, 4
    call assert_fail
.Lread_assert_reads:
    mov edi, 5
    call assert_fail
.size read_input, . - read_input

# Finds the column named "amount" in the header line.
# In: rdi = the input, rsi = its length.
# Out: rax = 0, rdx = the column's index and rcx = where the first row starts, or
# rax = ERROR_NOT_FOUND when the header has no amount column.
# returns: result
.type find_column, @function
find_column:
    test rdi, rdi
    jz .Lfind_column_assert_null
    xor ecx, ecx
    xor r8d, r8d
    xor r9d, r9d
    mov r10, -1
    mov r11, SCAN_LIMIT
.Lfind_column_loop:  # bound: 1048577
    cmp rcx, rsi
    jae .Lfind_column_not_found
    movzx eax, byte ptr [rdi + rcx]
    cmp eax, ','
    je .Lfind_column_end_field
    cmp eax, NEWLINE
    jne .Lfind_column_next
.Lfind_column_end_field:
    mov rdx, rcx
    sub rdx, r8
    cmp rdx, AMOUNT_LEN
    jne .Lfind_column_after
    cmp r8, rsi
    jae .Lfind_column_assert_start
    mov edx, dword ptr [rdi + r8]
    cmp edx, AMOUNT_HEAD
    jne .Lfind_column_after
    movzx edx, word ptr [rdi + r8 + 4]
    cmp edx, AMOUNT_TAIL
    jne .Lfind_column_after
    test r10, r10
    jns .Lfind_column_after
    mov r10, r9
.Lfind_column_after:
    inc r9
    lea r8, [rcx + 1]
    cmp eax, NEWLINE
    je .Lfind_column_header_end
.Lfind_column_next:
    inc rcx
    dec r11
    jnz .Lfind_column_loop
    jmp .Lfind_column_assert_scan
.Lfind_column_header_end:
    test r10, r10
    js .Lfind_column_not_found
    mov rdx, r10
    inc rcx
    xor eax, eax
    jmp .Lfind_column_return
.Lfind_column_not_found:
    mov rax, ERROR_NOT_FOUND
.Lfind_column_return:
    ret
.Lfind_column_assert_null:
    mov edi, 6
    call assert_fail
.Lfind_column_assert_start:
    mov edi, 7
    call assert_fail
.Lfind_column_assert_scan:
    mov edi, 8
    call assert_fail
.size find_column, . - find_column

# Adds up the amount field of every row.
# In: rdi = the input, rsi = its length, rdx = the amount column, rcx = where the rows start.
# Out: rax = 0 and rdx = the total in cents, or rax = ERROR_BAD or ERROR_MISSING and
# rdx = the line number of the row at fault.
# returns: result
.type sum_rows, @function
sum_rows:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8
    test rdi, rdi
    jz .Lsum_assert_null
    cmp rcx, rsi
    ja .Lsum_assert_start
    mov rbx, rdi
    mov r12, rsi
    mov r13, rdx
    mov r14, rcx
    xor r15d, r15d
    mov rbp, MAX_LINES
.Lsum_line:  # bound: 1048576
    cmp r14, r12
    jae .Lsum_done
    mov rdi, rbx
    mov rsi, r12
    mov rdx, r13
    mov rcx, r14
    call row_amount
    test rax, rax
    js .Lsum_fault
    add r15, rdx
    jo .Lsum_assert_overflow
    mov r14, rcx
    dec rbp
    jnz .Lsum_line
    jmp .Lsum_assert_lines
.Lsum_done:
    mov rdx, r15
    xor eax, eax
    jmp .Lsum_return
.Lsum_fault:
    mov rdx, LINES_BASE
    sub rdx, rbp
.Lsum_return:
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.Lsum_assert_null:
    mov edi, 9
    call assert_fail
.Lsum_assert_start:
    mov edi, 10
    call assert_fail
.Lsum_assert_overflow:
    mov edi, 11
    call assert_fail
.Lsum_assert_lines:
    mov edi, 12
    call assert_fail
.size sum_rows, . - sum_rows

# Reads one row's amount and finds where the next row starts.
# In: rdi = the input, rsi = its length, rdx = the amount column, rcx = where the row starts.
# Out: rax = 0, rdx = the amount in cents and rcx = where the next row starts, or
# rax = ERROR_MISSING or ERROR_BAD.
# returns: result
.type row_amount, @function
row_amount:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Lrow_assert_null
    cmp rcx, rsi
    jae .Lrow_assert_start
    mov rbx, rdi
    mov r12, rsi
    call find_field
    test rax, rax
    js .Lrow_return
    mov rdi, rbx
    mov rsi, r12
    call parse_amount
    test rax, rax
    js .Lrow_return
    mov r13, rdx
    mov rdi, rbx
    mov rsi, r12
    mov rdx, rcx
    call skip_line
    mov rcx, rax
    mov rdx, r13
    xor eax, eax
.Lrow_return:
    pop r13
    pop r12
    pop rbx
    ret
.Lrow_assert_null:
    mov edi, 35
    call assert_fail
.Lrow_assert_start:
    mov edi, 36
    call assert_fail
.size row_amount, . - row_amount

# Finds where a row's field starts by skipping the fields before it.
# In: rdi = the input, rsi = its length, rdx = the field's index, rcx = where the row starts.
# Out: rax = 0 and rdx = where the field starts, or rax = ERROR_MISSING when the row ends
# first.
# returns: result
.type find_field, @function
find_field:
    test rdi, rdi
    jz .Lfind_field_assert_null
    cmp rcx, rsi
    ja .Lfind_field_assert_start
    mov r8, rdx
    test r8, r8
    jz .Lfind_field_found
    mov r9, SCAN_LIMIT
.Lfind_field_loop:  # bound: 1048577
    cmp rcx, rsi
    jae .Lfind_field_missing
    movzx eax, byte ptr [rdi + rcx]
    inc rcx
    cmp eax, NEWLINE
    je .Lfind_field_missing
    cmp eax, ','
    jne .Lfind_field_next
    dec r8
    jz .Lfind_field_found
.Lfind_field_next:
    dec r9
    jnz .Lfind_field_loop
    jmp .Lfind_field_assert_scan
.Lfind_field_found:
    mov rdx, rcx
    xor eax, eax
    jmp .Lfind_field_return
.Lfind_field_missing:
    mov rax, ERROR_MISSING
.Lfind_field_return:
    ret
.Lfind_field_assert_null:
    mov edi, 13
    call assert_fail
.Lfind_field_assert_start:
    mov edi, 14
    call assert_fail
.Lfind_field_assert_scan:
    mov edi, 15
    call assert_fail
.size find_field, . - find_field

# Parses an amount field: an optional minus sign, then the cents.
# In: rdi = the input, rsi = its length, rdx = where the field starts.
# Out: rax = 0, rdx = the amount in cents and rcx = where the field ends, or
# rax = ERROR_BAD.
# returns: result
.type parse_amount, @function
parse_amount:
    push r12
    test rdi, rdi
    jz .Lparse_assert_null
    cmp rdx, rsi
    ja .Lparse_assert_start
    xor r12d, r12d
    cmp rdx, rsi
    jae .Lparse_unsigned
    cmp byte ptr [rdi + rdx], '-'
    jne .Lparse_unsigned
    mov r12d, 1
    inc rdx
.Lparse_unsigned:
    call parse_cents
    test rax, rax
    js .Lparse_return
    test r12d, r12d
    jz .Lparse_return
    neg rdx
.Lparse_return:
    pop r12
    ret
.Lparse_assert_null:
    mov edi, 16
    call assert_fail
.Lparse_assert_start:
    mov edi, 37
    call assert_fail
.size parse_amount, . - parse_amount

# Parses an amount without its sign: 1 to 6 digits without a leading zero (except a lone 0),
# a point and two digits, ended by a comma, a newline or the input's end.
# In: rdi = the input, rsi = its length, rdx = where the digits start.
# Out: rax = 0, rdx = the cents and rcx = where the field ends, or rax = ERROR_BAD.
# returns: result
.type parse_cents, @function
parse_cents:
    test rdi, rdi
    jz .Lcents_parse_assert_null
    mov rcx, rdx
    mov r8, rdx
    xor eax, eax
    mov r9, WHOLE_SCAN
.Lcents_parse_whole:  # bound: 7
    cmp rcx, rsi
    jae .Lcents_parse_bad
    movzx edx, byte ptr [rdi + rcx]
    cmp edx, '.'
    je .Lcents_parse_point
    sub edx, '0'
    cmp edx, 9
    ja .Lcents_parse_bad
    imul rax, rax, 10
    add rax, rdx
    inc rcx
    dec r9
    jnz .Lcents_parse_whole
    jmp .Lcents_parse_bad
.Lcents_parse_point:
    mov rdx, rcx
    sub rdx, r8
    jz .Lcents_parse_bad
    cmp rdx, 1
    je .Lcents_parse_fraction
    cmp r8, rsi
    jae .Lcents_parse_assert_start
    cmp byte ptr [rdi + r8], '0'
    je .Lcents_parse_bad
.Lcents_parse_fraction:
    inc rcx
    mov r9, FRACTION_DIGITS
.Lcents_parse_fraction_loop:  # bound: 2
    cmp rcx, rsi
    jae .Lcents_parse_bad
    movzx edx, byte ptr [rdi + rcx]
    sub edx, '0'
    cmp edx, 9
    ja .Lcents_parse_bad
    imul rax, rax, 10
    add rax, rdx
    inc rcx
    dec r9
    jnz .Lcents_parse_fraction_loop
    cmp rcx, rsi
    jae .Lcents_parse_ok
    movzx edx, byte ptr [rdi + rcx]
    cmp edx, ','
    je .Lcents_parse_ok
    cmp edx, NEWLINE
    jne .Lcents_parse_bad
.Lcents_parse_ok:
    mov rdx, rax
    xor eax, eax
    jmp .Lcents_parse_return
.Lcents_parse_bad:
    mov rax, ERROR_BAD
.Lcents_parse_return:
    ret
.Lcents_parse_assert_null:
    mov edi, 38
    call assert_fail
.Lcents_parse_assert_start:
    mov edi, 18
    call assert_fail
.size parse_cents, . - parse_cents

# Finds where the next line starts.
# In: rdi = the input, rsi = its length, rdx = a position inside a line.
# Out: rax = the position after the line's newline, or the input's length.
.type skip_line, @function
skip_line:
    test rdi, rdi
    jz .Lskip_assert_null
    cmp rdx, rsi
    ja .Lskip_assert_start
    mov rcx, rdx
    mov r8, SCAN_LIMIT
.Lskip_loop:  # bound: 1048577
    cmp rcx, rsi
    jae .Lskip_end
    movzx eax, byte ptr [rdi + rcx]
    inc rcx
    cmp eax, NEWLINE
    je .Lskip_end
    dec r8
    jnz .Lskip_loop
    jmp .Lskip_assert_scan
.Lskip_end:
    mov rax, rcx
    ret
.Lskip_assert_null:
    mov edi, 19
    call assert_fail
.Lskip_assert_start:
    mov edi, 20
    call assert_fail
.Lskip_assert_scan:
    mov edi, 21
    call assert_fail
.size skip_line, . - skip_line

# Prints an amount in cents with two decimals, then a newline, to standard output.
# In: rdi = the cents, never -9223372036854775808.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_cents, @function
write_cents:
    sub rsp, 8
    mov rax, rdi
    mov r9, rdi
    test rax, rax
    jns .Lcents_digits
    neg rax
    jo .Lcents_assert_min
.Lcents_digits:
    mov byte ptr [rip + cents_buf + CENTS_BUF_LEN - 1], NEWLINE
    lea rsi, [rip + cents_buf]
    mov ecx, CENTS_BUF_LEN - 1
    mov r10d, 10
    mov r8, MAX_DIGITS
.Lcents_digit:  # bound: 19
    xor edx, edx
    div r10
    add edx, '0'
    dec ecx
    cmp ecx, CENTS_BUF_LEN
    jae .Lcents_assert_index
    mov byte ptr [rsi + rcx], dl
    cmp ecx, CENTS_POINT + 1
    jne .Lcents_more
    mov byte ptr [rip + cents_buf + CENTS_POINT], '.'
    dec ecx
.Lcents_more:
    test rax, rax
    jnz .Lcents_next
    cmp ecx, CENTS_POINT
    jb .Lcents_sign
.Lcents_next:
    dec r8
    jnz .Lcents_digit
    jmp .Lcents_assert_digits
.Lcents_sign:
    test r9, r9
    jns .Lcents_output
    dec ecx
    cmp ecx, CENTS_BUF_LEN
    jae .Lcents_assert_sign_index
    mov byte ptr [rsi + rcx], '-'
.Lcents_output:
    add rsi, rcx
    mov edx, CENTS_BUF_LEN
    sub edx, ecx
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lcents_return
    xor eax, eax
.Lcents_return:
    add rsp, 8
    ret
.Lcents_assert_min:
    mov edi, 22
    call assert_fail
.Lcents_assert_index:
    mov edi, 23
    call assert_fail
.Lcents_assert_digits:
    mov edi, 24
    call assert_fail
.Lcents_assert_sign_index:
    mov edi, 34
    call assert_fail
.size write_cents, . - write_cents

# Prints a message, then a line number and a newline, to standard error.
# In: rdi = the message, rsi = its length, rdx = the line number, at least 2.
# Out: rax = 0, or a negative error code.
# returns: result
.type report_line, @function
report_line:
    push r12
    test rdi, rdi
    jz .Lreport_assert_null
    cmp rdx, FIRST_ROW_LINE
    jb .Lreport_assert_line
    mov r12, rdx
    mov rdx, rsi
    mov rsi, rdi
    mov edi, STDERR
    call write_all
    test rax, rax
    js .Lreport_return
    mov rdi, r12
    mov esi, STDERR
    call write_i64
    test rax, rax
    js .Lreport_return
    xor eax, eax
.Lreport_return:
    pop r12
    ret
.Lreport_assert_null:
    mov edi, 25
    call assert_fail
.Lreport_assert_line:
    mov edi, 26
    call assert_fail
.size report_line, . - report_line

# Writes a whole number in decimal, then a newline.
# In: rdi = the number, never -9223372036854775808, esi = the file descriptor.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_i64, @function
write_i64:
    sub rsp, 8
    mov r11d, esi
    mov rax, rdi
    mov r9, rdi
    test rax, rax
    jns .Lwrite_i64_digits
    neg rax
    jo .Lwrite_i64_assert_min
.Lwrite_i64_digits:
    mov byte ptr [rip + out_buf + OUT_BUF_LEN - 1], NEWLINE
    lea rsi, [rip + out_buf]
    mov ecx, OUT_BUF_LEN - 1
    mov r10d, 10
    mov r8, MAX_DIGITS
.Lwrite_i64_digit:  # bound: 19
    xor edx, edx
    div r10
    add edx, '0'
    dec ecx
    cmp ecx, OUT_BUF_LEN
    jae .Lwrite_i64_assert_digit_index
    mov byte ptr [rsi + rcx], dl
    test rax, rax
    jz .Lwrite_i64_sign
    dec r8
    jnz .Lwrite_i64_digit
    jmp .Lwrite_i64_assert_digits
.Lwrite_i64_sign:
    test r9, r9
    jns .Lwrite_i64_output
    dec ecx
    cmp ecx, OUT_BUF_LEN
    jae .Lwrite_i64_assert_sign_index
    mov byte ptr [rsi + rcx], '-'
.Lwrite_i64_output:
    add rsi, rcx
    mov edx, OUT_BUF_LEN
    sub edx, ecx
    mov edi, r11d
    call write_all
    test rax, rax
    js .Lwrite_i64_return
    xor eax, eax
.Lwrite_i64_return:
    add rsp, 8
    ret
.Lwrite_i64_assert_min:
    mov edi, 27
    call assert_fail
.Lwrite_i64_assert_digit_index:
    mov edi, 28
    call assert_fail
.Lwrite_i64_assert_digits:
    mov edi, 29
    call assert_fail
.Lwrite_i64_assert_sign_index:
    mov edi, 30
    call assert_fail
.size write_i64, . - write_i64

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
    mov edi, 31
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 32
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 33
    call assert_fail
.size write_all, . - write_all

