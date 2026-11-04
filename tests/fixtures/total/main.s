.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_READ, 0
.equ SYS_WRITE, 1
.equ SYS_CLOSE, 3
.equ SYS_EXIT, 60
.equ SYS_OPENAT, 257
# File descriptors and open flags
.equ STDOUT, 1
.equ STDERR, 2
.equ AT_FDCWD, -100
.equ O_RDONLY, 0
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_OPEN_FAILED, 2
.equ EXIT_BAD_LINE, 3
.equ EXIT_READ_FAILED, 4
.equ EXIT_WRITE_FAILED, 5
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_OPEN, -2
.equ ERROR_INVALID, -22
# Sizes and limits
.equ ARGC_EXPECTED, 2
.equ FILE_MAX, 1048576
.equ SCAN_MAX, FILE_MAX + 1
.equ MAX_LINES, FILE_MAX
.equ MAX_READS, 1024
.equ MAX_WRITES, 64
.equ INT_DIGITS_LIMIT, 6
.equ MAX_CENTS, 9999999
.equ CENT_PLACES, 2
.equ NO_PLACES, 0
.equ NUM_BUF_LEN, 24
.equ MAX_NUM_CHARS, 24
.equ NEWLINE, 10
.equ COMMA, 44
.equ POINT, 46
.equ USAGE_MSG_LEN, 24
.equ OPEN_MSG_LEN, 24
.equ BAD_MSG_LEN, 16

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: total <file.csv>\n"
.size usage_msg, . - usage_msg

.type open_msg, @object
open_msg:
    .ascii "total: cannot open file\n"
.size open_msg, . - open_msg

.type bad_msg, @object
bad_msg:
    .ascii "total: bad line "
.size bad_msg, . - bad_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# The whole expenses file.
# shared: load_file reads the file into it and _start lends it to sum_lines
.type file_buf, @object
file_buf:
    .zero FILE_MAX
.size file_buf, . - file_buf

# Digits, a point and a newline.
.type num_buf, @object
num_buf:
    .zero NUM_BUF_LEN
.size num_buf, . - num_buf

.text
.globl _start

# Opens the file named by the argument, reads it, adds its amounts and prints the total.
.type _start, @function
_start:
    mov rax, qword ptr [rsp]
    cmp rax, ARGC_EXPECTED
    jne .Lstart_usage
    mov rdi, qword ptr [rsp + 16]
    test rdi, rdi
    jz .Lstart_assert_argv
    call load_file
    test rax, rax
    js .Lstart_load_failed
    mov rsi, rax
    lea rdi, [rip + file_buf]
    call sum_lines
    test rax, rax
    js .Lstart_bad_line
    mov rdi, rdx
    mov esi, CENT_PLACES
    mov edx, STDOUT
    call write_decimal
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_usage:
    lea rdi, [rip + usage_msg]
    mov esi, USAGE_MSG_LEN
    call report
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_USAGE
    jmp .Lstart_exit
.Lstart_load_failed:
    cmp rax, ERROR_OPEN
    jne .Lstart_read_failed
    lea rdi, [rip + open_msg]
    mov esi, OPEN_MSG_LEN
    call report
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OPEN_FAILED
    jmp .Lstart_exit
.Lstart_bad_line:
    mov rdi, rdx
    call report_bad_line
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_BAD_LINE
    jmp .Lstart_exit
.Lstart_read_failed:
    mov edi, EXIT_READ_FAILED
    jmp .Lstart_exit
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_argv:
    mov edi, 1
    call assert_fail
.size _start, . - _start

# Opens the file a path names, reads all of it into file_buf and closes it.
# In: rdi = the path, a NUL-terminated string.
# Out: rax = the bytes read, or ERROR_OPEN when the file can't be opened, or ERROR_IO.
# returns: result
.type load_file, @function
load_file:
    push rbx
    test rdi, rdi
    jz .Lload_file_assert_null
    mov rsi, rdi
    mov edi, AT_FDCWD
    mov edx, O_RDONLY
    xor r10d, r10d
    mov eax, SYS_OPENAT
    syscall
    test rax, rax
    js .Lload_file_open_failed
    mov rbx, rax
    mov edi, ebx
    lea rsi, [rip + file_buf]
    mov edx, FILE_MAX
    call read_all
    test rax, rax
    js .Lload_file_failed
    mov r8, rax
    mov edi, ebx
    mov eax, SYS_CLOSE
    syscall
    test rax, rax
    js .Lload_file_failed
    mov rax, r8
    jmp .Lload_file_return
.Lload_file_open_failed:
    mov rax, ERROR_OPEN
    jmp .Lload_file_return
.Lload_file_failed:
    mov rax, ERROR_IO
.Lload_file_return:
    pop rbx
    ret
.Lload_file_assert_null:
    mov edi, 26
    call assert_fail
.size load_file, . - load_file

# Prints a message to standard error.
# In: rdi = the message, rsi = its length, at least 1.
# Out: rax = 0, or a negative error code.
# returns: result
.type report, @function
report:
    sub rsp, 8
    test rdi, rdi
    jz .Lreport_assert_null
    mov rdx, rsi
    mov rsi, rdi
    mov edi, STDERR
    call write_all
    test rax, rax
    js .Lreport_return
    xor eax, eax
.Lreport_return:
    add rsp, 8
    ret
.Lreport_assert_null:
    mov edi, 25
    call assert_fail
.size report, . - report

# Prints "total: bad line " and a line number, then a newline, to standard error.
# In: rdi = the line number.
# Out: rax = 0, or a negative error code.
# returns: result
.type report_bad_line, @function
report_bad_line:
    push rbx
    mov rbx, rdi
    lea rdi, [rip + bad_msg]
    mov esi, BAD_MSG_LEN
    call report
    test rax, rax
    js .Lreport_bad_line_return
    mov rdi, rbx
    mov esi, NO_PLACES
    mov edx, STDERR
    call write_decimal
    test rax, rax
    js .Lreport_bad_line_return
    xor eax, eax
.Lreport_bad_line_return:
    pop rbx
    ret
.size report_bad_line, . - report_bad_line

# Reads from a file descriptor until the input ends or the buffer is full.
# In: edi = the file descriptor, rsi = the buffer, rdx = its size, at least 1.
# Out: rax = the bytes read, or a negative error code.
# returns: result
.type read_all, @function
read_all:
    test rsi, rsi
    jz .Lread_all_assert_null
    test rdx, rdx
    jz .Lread_all_assert_empty
    mov r9, rdx
    xor r10d, r10d
    mov r8, MAX_READS
.Lread_all_loop:  # bound: 1024
    mov rdx, r9
    sub rdx, r10
    mov eax, SYS_READ
    syscall
    test rax, rax
    js .Lread_all_return
    jz .Lread_all_done
    cmp rax, rdx
    ja .Lread_all_assert_count
    add rsi, rax
    add r10, rax
    cmp r10, r9
    je .Lread_all_done
    dec r8
    jnz .Lread_all_loop
    mov rax, ERROR_IO
    jmp .Lread_all_return
.Lread_all_done:
    mov rax, r10
.Lread_all_return:
    ret
.Lread_all_assert_null:
    mov edi, 2
    call assert_fail
.Lread_all_assert_empty:
    mov edi, 3
    call assert_fail
.Lread_all_assert_count:
    mov edi, 4
    call assert_fail
.size read_all, . - read_all

# Adds the amounts of every line.
# In: rdi = the file's bytes, rsi = how many, at most FILE_MAX.
# Out: rax = 0 and rdx = the total in cents, or rax = ERROR_INVALID and rdx = the bad line's number.
# returns: result
.type sum_lines, @function
sum_lines:
    push rbx
    push r12
    push r13
    push r14
    push r15
    test rdi, rdi
    jz .Lsum_lines_assert_null
    cmp rsi, FILE_MAX
    ja .Lsum_lines_assert_len
    mov r12, rdi
    mov r13, rsi
    xor ebx, ebx
    xor r14d, r14d
    mov r15, MAX_LINES
.Lsum_lines_loop:  # bound: 1048576
    test r13, r13
    jz .Lsum_lines_done
    inc r14
    mov rdi, r12
    mov rsi, r13
    call parse_line
    test rax, rax
    js .Lsum_lines_bad
    cmp rax, r13
    ja .Lsum_lines_assert_consumed
    add r12, rax
    sub r13, rax
    add rbx, rdx
    jo .Lsum_lines_assert_total
    dec r15
    jnz .Lsum_lines_loop
    jmp .Lsum_lines_assert_lines
.Lsum_lines_done:
    xor eax, eax
    mov rdx, rbx
    jmp .Lsum_lines_return
.Lsum_lines_bad:
    mov rdx, r14
.Lsum_lines_return:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Lsum_lines_assert_null:
    mov edi, 5
    call assert_fail
.Lsum_lines_assert_len:
    mov edi, 6
    call assert_fail
.Lsum_lines_assert_consumed:
    mov edi, 7
    call assert_fail
.Lsum_lines_assert_total:
    mov edi, 8
    call assert_fail
.Lsum_lines_assert_lines:
    mov edi, 9
    call assert_fail
.size sum_lines, . - sum_lines

# Reads one line: a date, a comma, an amount, then a newline or the end of the file.
# In: rdi = the line's first byte, rsi = the bytes left in the file, at least 1.
# Out: rax = the bytes the line takes, newline included, and rdx = its amount in cents,
# or rax = ERROR_INVALID.
# returns: result
.type parse_line, @function
parse_line:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Lparse_line_assert_null
    mov rbx, rdi
    mov r12, rsi
    call skip_date
    test rax, rax
    js .Lparse_line_return
    lea r13, [rax + 1]
    cmp r13, r12
    ja .Lparse_line_assert_comma
    lea rdi, [rbx + r13]
    mov rsi, r12
    sub rsi, r13
    call parse_amount
    test rax, rax
    js .Lparse_line_return
    add r13, rax
    cmp r13, r12
    jae .Lparse_line_end
    movzx ecx, byte ptr [rbx + r13]
    cmp ecx, NEWLINE
    jne .Lparse_line_invalid
    inc r13
.Lparse_line_end:
    cmp r13, r12
    ja .Lparse_line_assert_amount
    mov rax, r13
    jmp .Lparse_line_return
.Lparse_line_invalid:
    mov rax, ERROR_INVALID
.Lparse_line_return:
    pop r13
    pop r12
    pop rbx
    ret
.Lparse_line_assert_null:
    mov edi, 10
    call assert_fail
.Lparse_line_assert_comma:
    mov edi, 11
    call assert_fail
.Lparse_line_assert_amount:
    mov edi, 12
    call assert_fail
.size parse_line, . - parse_line

# Finds the comma that ends a line's date.
# In: rdi = the line's first byte, rsi = the bytes left in the file, at most FILE_MAX.
# Out: rax = the comma's index, or ERROR_INVALID when a newline or the end comes first.
# returns: result
.type skip_date, @function
skip_date:
    test rdi, rdi
    jz .Lskip_date_assert_null
    cmp rsi, FILE_MAX
    ja .Lskip_date_assert_len
    xor ecx, ecx
    mov r8, SCAN_MAX
.Lskip_date_loop:  # bound: 1048577
    cmp rcx, rsi
    jae .Lskip_date_invalid
    movzx edx, byte ptr [rdi + rcx]
    cmp edx, NEWLINE
    je .Lskip_date_invalid
    cmp edx, COMMA
    jne .Lskip_date_next
    mov rax, rcx
    jmp .Lskip_date_return
.Lskip_date_next:
    inc rcx
    dec r8
    jnz .Lskip_date_loop
    jmp .Lskip_date_assert_scan
.Lskip_date_invalid:
    mov rax, ERROR_INVALID
.Lskip_date_return:
    ret
.Lskip_date_assert_null:
    mov edi, 13
    call assert_fail
.Lskip_date_assert_len:
    mov edi, 14
    call assert_fail
.Lskip_date_assert_scan:
    mov edi, 15
    call assert_fail
.size skip_date, . - skip_date

# Reads an amount of 1 to 5 digits, a point and 2 digits.
# In: rdi = its first byte, rsi = the bytes left in the file, at most FILE_MAX.
# Out: rax = the bytes it takes and rdx = the amount in cents, or rax = ERROR_INVALID.
# returns: result
.type parse_amount, @function
parse_amount:
    test rdi, rdi
    jz .Lparse_amount_assert_null
    cmp rsi, FILE_MAX
    ja .Lparse_amount_assert_len
    xor r9d, r9d
    xor ecx, ecx
    mov r8, INT_DIGITS_LIMIT
.Lparse_amount_loop:  # bound: 6
    cmp rcx, rsi
    jae .Lparse_amount_invalid
    movzx edx, byte ptr [rdi + rcx]
    cmp edx, POINT
    je .Lparse_amount_point
    sub edx, '0'
    cmp edx, 9
    ja .Lparse_amount_invalid
    imul r9, r9, 10
    add r9, rdx
    inc rcx
    dec r8
    jnz .Lparse_amount_loop
    jmp .Lparse_amount_invalid
.Lparse_amount_point:
    test rcx, rcx
    jz .Lparse_amount_invalid
    inc rcx
    cmp rcx, rsi
    jae .Lparse_amount_invalid
    movzx edx, byte ptr [rdi + rcx]
    sub edx, '0'
    cmp edx, 9
    ja .Lparse_amount_invalid
    imul r9, r9, 10
    add r9, rdx
    inc rcx
    cmp rcx, rsi
    jae .Lparse_amount_invalid
    movzx edx, byte ptr [rdi + rcx]
    sub edx, '0'
    cmp edx, 9
    ja .Lparse_amount_invalid
    imul r9, r9, 10
    add r9, rdx
    inc rcx
    cmp r9, MAX_CENTS
    ja .Lparse_amount_assert_cents
    mov rax, rcx
    mov rdx, r9
    jmp .Lparse_amount_return
.Lparse_amount_invalid:
    mov rax, ERROR_INVALID
.Lparse_amount_return:
    ret
.Lparse_amount_assert_null:
    mov edi, 16
    call assert_fail
.Lparse_amount_assert_len:
    mov edi, 17
    call assert_fail
.Lparse_amount_assert_cents:
    mov edi, 18
    call assert_fail
.size parse_amount, . - parse_amount

# Writes a whole number of units as a decimal with a given number of places, then a newline.
# In: rdi = the number, esi = the places (0 or 2), edx = the file descriptor.
# Out: rax = 0, or a negative error code.
# returns: result
.type write_decimal, @function
write_decimal:
    sub rsp, 8
    cmp esi, CENT_PLACES
    ja .Lwrite_decimal_assert_places
    mov r11d, edx
    mov rax, rdi
    mov r8d, esi
    lea rsi, [rip + num_buf]
    mov ecx, NUM_BUF_LEN - 1
    mov byte ptr [rip + num_buf + NUM_BUF_LEN - 1], NEWLINE
    mov r10d, 10
    mov r9, MAX_NUM_CHARS
.Lwrite_decimal_loop:  # bound: 24
    xor edx, edx
    div r10
    add edx, '0'
    dec ecx
    cmp ecx, NUM_BUF_LEN
    jae .Lwrite_decimal_assert_index
    mov byte ptr [rsi + rcx], dl
    dec r8
    jnz .Lwrite_decimal_more
    dec ecx
    cmp ecx, NUM_BUF_LEN
    jae .Lwrite_decimal_assert_point
    mov byte ptr [rsi + rcx], POINT
.Lwrite_decimal_more:
    test rax, rax
    jnz .Lwrite_decimal_next
    test r8, r8
    js .Lwrite_decimal_output
.Lwrite_decimal_next:
    dec r9
    jnz .Lwrite_decimal_loop
    jmp .Lwrite_decimal_assert_chars
.Lwrite_decimal_output:
    add rsi, rcx
    mov edx, NUM_BUF_LEN
    sub edx, ecx
    mov edi, r11d
    call write_all
    test rax, rax
    js .Lwrite_decimal_return
    xor eax, eax
.Lwrite_decimal_return:
    add rsp, 8
    ret
.Lwrite_decimal_assert_places:
    mov edi, 19
    call assert_fail
.Lwrite_decimal_assert_index:
    mov edi, 20
    call assert_fail
.Lwrite_decimal_assert_point:
    mov edi, 27
    call assert_fail
.Lwrite_decimal_assert_chars:
    mov edi, 21
    call assert_fail
.size write_decimal, . - write_decimal

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
    mov edi, 22
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 23
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 24
    call assert_fail
.size write_all, . - write_all

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
