.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_WRITE, 1
.equ SYS_CLOSE, 3
.equ SYS_FSTAT, 5
.equ SYS_EXIT, 60
.equ SYS_GETDENTS64, 217
.equ SYS_OPENAT, 257
.equ SYS_UNLINKAT, 263
# File descriptors and flags
.equ STDOUT, 1
.equ AT_FDCWD, -100
.equ O_RDONLY, 0
# Read-only, as a folder (O_RDONLY | O_DIRECTORY).
.equ OPEN_FOLDER, 0x10000
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_FAILED, 3
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
# A folder's entries, as getdents64 gives them: each a record with its length at byte 16, its
# kind at byte 18 and its NUL-terminated name from byte 19.
.equ ENTRIES_LEN, 4096
.equ RECORD_LEN_AT, 16
.equ RECORD_KIND_AT, 18
.equ RECORD_NAME_AT, 19
.equ RECORD_MIN, 24
.equ LAST_RECORD, 4072
.equ KIND_FILE, 8
# A file's details, as fstat gives them: its size at byte 48.
.equ DETAILS_LEN, 144
.equ SIZE_AT, 48
# Sizes and limits
.equ MAX_BATCHES, 64
.equ MAX_RECORDS, 171
.equ MAX_COUNT, 1000
.equ DELETED_LEN, 8
.equ OUT_BUF_LEN, 12
.equ MAX_WRITES, 64
# Characters
.equ NEWLINE, 10

.section .rodata

.type this_folder, @object
this_folder:
    .ascii ".\0"
.size this_folder, . - this_folder

.type deleted_msg, @object
deleted_msg:
    .ascii "deleted "
.size deleted_msg, . - deleted_msg

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

# One batch of the folder's entries.
.type entries, @object
.balign 8
entries:
    .zero ENTRIES_LEN
.size entries, . - entries

# One file's details.
.type details, @object
.balign 8
details:
    .zero DETAILS_LEN
.size details, . - details

# What the program prints.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

# Deletes the empty files in the working folder and says how many.
.type _start, @function
_start:
    mov edi, AT_FDCWD
    lea rsi, [rip + this_folder]
    mov edx, OPEN_FOLDER
    mov eax, SYS_OPENAT
    syscall
    test rax, rax
    js .Lstart_failed
    mov ebx, eax
    mov edi, eax
    call sweep_folder
    test rax, rax
    js .Lstart_failed
    mov r12, rax
    mov edi, ebx
    mov eax, SYS_CLOSE
    syscall
    test rax, rax
    js .Lstart_failed
    mov rdi, r12
    call print_count
    test rax, rax
    js .Lstart_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_failed:
    mov edi, EXIT_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.size _start, . - _start

# Deletes the empty files in an open folder, batch by batch.
# In: edi = the folder's file descriptor.
# Out: rax = how many it deleted, or a negative error code.
# returns: result
.type sweep_folder, @function
sweep_folder:
    push rbx
    push r12
    push r13
    test edi, edi
    js .Lsweep_folder_assert_fd
    mov ebx, edi
    xor r12d, r12d
    mov r13, MAX_BATCHES
.Lsweep_folder_loop:  # bound: 64
    mov edi, ebx
    lea rsi, [rip + entries]
    mov edx, ENTRIES_LEN
    mov eax, SYS_GETDENTS64
    syscall
    test rax, rax
    js .Lsweep_folder_return
    jz .Lsweep_folder_done
    cmp rax, ENTRIES_LEN
    ja .Lsweep_folder_assert_batch
    lea rdi, [rip + entries]
    mov rsi, rax
    call sweep_batch
    test rax, rax
    js .Lsweep_folder_return
    add r12, rax
    dec r13
    jnz .Lsweep_folder_loop
    mov rax, ERROR_IO
    jmp .Lsweep_folder_return
.Lsweep_folder_done:
    mov rax, r12
.Lsweep_folder_return:
    pop r13
    pop r12
    pop rbx
    ret
.Lsweep_folder_assert_fd:
    mov edi, 1
    call assert_fail
.Lsweep_folder_assert_batch:
    mov edi, 2
    call assert_fail
.size sweep_folder, . - sweep_folder

# Deletes the empty files one batch of entries names.
# In: rdi = the entries, rsi = their length in bytes, 1 to ENTRIES_LEN.
# Out: rax = how many it deleted, or a negative error code.
# returns: result
.type sweep_batch, @function
sweep_batch:
    push rbx
    push r12
    push r13
    push r14
    push r15
    test rdi, rdi
    jz .Lsweep_batch_assert_null
    cmp rsi, ENTRIES_LEN
    ja .Lsweep_batch_assert_length
    mov rbx, rdi
    mov r12, rsi
    xor r13d, r13d
    xor r14d, r14d
    mov r15, MAX_RECORDS
.Lsweep_batch_loop:  # bound: 171
    cmp r13, r12
    jae .Lsweep_batch_done
    cmp r13, LAST_RECORD
    ja .Lsweep_batch_assert_record
    movzx eax, byte ptr [rbx + r13 + RECORD_KIND_AT]
    cmp eax, KIND_FILE
    jne .Lsweep_batch_next
    lea rdi, [rbx + r13 + RECORD_NAME_AT]
    call delete_if_empty
    test rax, rax
    js .Lsweep_batch_return
    add r14, rax
.Lsweep_batch_next:
    cmp r13, LAST_RECORD
    ja .Lsweep_batch_assert_next
    movzx eax, word ptr [rbx + r13 + RECORD_LEN_AT]
    cmp eax, RECORD_MIN
    jb .Lsweep_batch_assert_size
    add r13, rax
    dec r15
    jnz .Lsweep_batch_loop
.Lsweep_batch_done:
    mov rax, r14
.Lsweep_batch_return:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Lsweep_batch_assert_null:
    mov edi, 3
    call assert_fail
.Lsweep_batch_assert_length:
    mov edi, 4
    call assert_fail
.Lsweep_batch_assert_record:
    mov edi, 5
    call assert_fail
.Lsweep_batch_assert_next:
    mov edi, 6
    call assert_fail
.Lsweep_batch_assert_size:
    mov edi, 7
    call assert_fail
.size sweep_batch, . - sweep_batch

# Deletes a file when it's empty.
# In: rdi = the file's NUL-terminated name.
# Out: rax = 1 when it deleted it, 0 when it kept it, or a negative error code.
# returns: result
.type delete_if_empty, @function
delete_if_empty:
    push rbx
    push r12
    push r13
    test rdi, rdi
    jz .Ldelete_if_empty_assert_null
    mov rbx, rdi
    mov edi, AT_FDCWD
    mov rsi, rbx
    mov edx, O_RDONLY
    mov eax, SYS_OPENAT
    syscall
    test rax, rax
    js .Ldelete_if_empty_return
    mov r12d, eax
    mov edi, eax
    lea rsi, [rip + details]
    mov eax, SYS_FSTAT
    syscall
    test rax, rax
    js .Ldelete_if_empty_return
    mov r13, qword ptr [rip + details + SIZE_AT]
    mov edi, r12d
    mov eax, SYS_CLOSE
    syscall
    test rax, rax
    js .Ldelete_if_empty_return
    xor eax, eax
    test r13, r13
    jnz .Ldelete_if_empty_return
    mov edi, AT_FDCWD
    mov rsi, rbx
    xor edx, edx
    mov eax, SYS_UNLINKAT
    syscall
    test rax, rax
    js .Ldelete_if_empty_return
    mov eax, 1
.Ldelete_if_empty_return:
    pop r13
    pop r12
    pop rbx
    ret
.Ldelete_if_empty_assert_null:
    mov edi, 8
    call assert_fail
.size delete_if_empty, . - delete_if_empty

# Prints "deleted ", a count in decimal, and a newline.
# In: rdi = the count, below MAX_COUNT.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_count, @function
print_count:
    sub rsp, 8
    cmp rdi, MAX_COUNT
    jae .Lprint_count_assert_count
    lea rsi, [rip + out_buf]
    mov rax, qword ptr [rip + deleted_msg]
    mov qword ptr [rsi], rax
    mov rax, rdi
    mov r8d, 10
    mov ecx, DELETED_LEN
    cmp rax, 100
    jb .Lprint_count_tens
    xor edx, edx
    mov r9d, 100
    div r9
    add eax, '0'
    mov byte ptr [rsi + DELETED_LEN], al
    inc ecx
    mov rax, rdx
    jmp .Lprint_count_two
.Lprint_count_tens:
    cmp rax, 10
    jb .Lprint_count_last
.Lprint_count_two:
    xor edx, edx
    div r8
    add eax, '0'
    cmp rcx, OUT_BUF_LEN - 2
    jae .Lprint_count_assert_index
    mov byte ptr [rsi + rcx], al
    inc ecx
    mov rax, rdx
.Lprint_count_last:
    add eax, '0'
    cmp rcx, OUT_BUF_LEN - 1
    jae .Lprint_count_assert_last
    mov byte ptr [rsi + rcx], al
    mov byte ptr [rsi + rcx + 1], NEWLINE
    lea rdx, [rcx + 2]
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lprint_count_return
    xor eax, eax
.Lprint_count_return:
    add rsp, 8
    ret
.Lprint_count_assert_count:
    mov edi, 9
    call assert_fail
.Lprint_count_assert_index:
    mov edi, 10
    call assert_fail
.Lprint_count_assert_last:
    mov edi, 11
    call assert_fail
.size print_count, . - print_count

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
    mov edi, 12
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 13
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 14
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
