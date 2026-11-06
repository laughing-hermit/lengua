.intel_syntax noprefix

# File descriptors and os_open's mode
.equ STDOUT, 1
.equ STDERR, 2
.equ OPEN_READ, 0
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_MISSING, 2
.equ EXIT_FAILED, 3
# Error codes returned by functions marked "returns: result"
.equ ERROR_MISSING, -2
.equ ERROR_IO, -5
.equ ERROR_EXISTS, -17
.equ ERROR_INVALID, -22
# Sizes and limits
.equ ARGC_EXPECTED, 2
.equ MAX_NAME, 64
.equ MAX_NAME_SCAN, 65
.equ MAX_COPY, 64
.equ FOLDER_PREFIX_LEN, 8
.equ TARGET_LEN, 73
.equ MESSAGE_LEN, 82
.equ ARCHIVED_LEN, 9
.equ MISSING_LEN, 17
.equ USAGE_LEN, 22
.equ MAX_WRITES, 64
# Characters
.equ SLASH, '/'
.equ NEWLINE, 10

.section .rodata

.type folder_name, @object
folder_name:
    .ascii "archive\0"
.size folder_name, . - folder_name

.type folder_prefix, @object
folder_prefix:
    .ascii "archive/"
.size folder_prefix, . - folder_prefix

.type archived_msg, @object
archived_msg:
    .ascii "archived "
.size archived_msg, . - archived_msg

.type missing_msg, @object
missing_msg:
    .ascii "archive: no file "
.size missing_msg, . - missing_msg

.type usage_msg, @object
usage_msg:
    .ascii "usage: archive <file>\n"
.size usage_msg, . - usage_msg

.section .bss

# Where the file goes: "archive/", its name, and a NUL.
.type target, @object
target:
    .zero TARGET_LEN
.size target, . - target

# What the program says about the file: a phrase, its name, and a newline.
.type message, @object
message:
    .zero MESSAGE_LEN
.size message, . - message

.text
# Moves the file the argument names into the archive folder, and says so.
.type main, @function
main:
    sub rsp, 8
    cmp rdi, ARGC_EXPECTED
    jne .Lmain_usage
    mov rbx, qword ptr [rsi + 8]
    test rbx, rbx
    jz .Lmain_assert_argv
    mov rdi, rbx
    call name_length
    test rax, rax
    js .Lmain_usage
    mov r12, rax
    mov rdi, rbx
    mov rsi, r12
    call archive_file
    test rax, rax
    jz .Lmain_archived
    cmp rax, ERROR_MISSING
    jne .Lmain_failed
    mov edi, STDERR
    lea rsi, [rip + missing_msg]
    mov edx, MISSING_LEN
    mov rcx, rbx
    mov r8, r12
    call report
    test rax, rax
    js .Lmain_failed
    mov edi, EXIT_MISSING
    jmp .Lmain_exit
.Lmain_archived:
    mov edi, STDOUT
    lea rsi, [rip + archived_msg]
    mov edx, ARCHIVED_LEN
    mov rcx, rbx
    mov r8, r12
    call report
    test rax, rax
    js .Lmain_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_usage:
    mov edi, STDERR
    lea rsi, [rip + usage_msg]
    mov edx, USAGE_LEN
    call write_all
    test rax, rax
    js .Lmain_failed
    mov edi, EXIT_USAGE
    jmp .Lmain_exit
.Lmain_failed:
    mov edi, EXIT_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_argv:
    mov edi, 1
    call assert_fail
.size main, . - main

# How long a file name is, when it's one the program takes.
# In: rdi = a NUL-terminated name.
# Out: rax = its length, 1 to MAX_NAME, or ERROR_INVALID when it's empty, longer, or has a /.
# returns: result
.type name_length, @function
name_length:
    test rdi, rdi
    jz .Lname_length_assert_null
    xor ecx, ecx
    mov r8, MAX_NAME_SCAN
.Lname_length_loop:  # bound: 65
    cmp rcx, MAX_NAME_SCAN
    jae .Lname_length_assert_index
    movzx eax, byte ptr [rdi + rcx]
    test eax, eax
    jz .Lname_length_end
    cmp eax, SLASH
    je .Lname_length_invalid
    inc rcx
    dec r8
    jnz .Lname_length_loop
    jmp .Lname_length_invalid
.Lname_length_end:
    test rcx, rcx
    jz .Lname_length_invalid
    mov rax, rcx
    jmp .Lname_length_return
.Lname_length_invalid:
    mov rax, ERROR_INVALID
.Lname_length_return:
    ret
.Lname_length_assert_null:
    mov edi, 2
    call assert_fail
.Lname_length_assert_index:
    mov edi, 3
    call assert_fail
.size name_length, . - name_length

# Moves a file into the archive folder, making the folder when it isn't there.
# In: rdi = the file's NUL-terminated name, rsi = its length, 1 to MAX_NAME.
# Out: rax = 0, ERROR_MISSING when there's no such file, or another negative error code.
# returns: result
.type archive_file, @function
archive_file:
    push rbx
    push r12
    sub rsp, 8
    test rdi, rdi
    jz .Larchive_file_assert_null
    test rsi, rsi
    jz .Larchive_file_assert_empty
    cmp rsi, MAX_NAME
    ja .Larchive_file_assert_long
    mov rbx, rdi
    mov r12, rsi
    mov rdi, rbx
    mov esi, OPEN_READ
    call os_open
    test rax, rax
    js .Larchive_file_return
    mov edi, eax
    call os_close
    test rax, rax
    js .Larchive_file_return
    lea rdi, [rip + folder_name]
    call os_mkdir
    test rax, rax
    jz .Larchive_file_folder
    cmp rax, ERROR_EXISTS
    jne .Larchive_file_return
.Larchive_file_folder:
    mov rdi, rbx
    mov rsi, r12
    call build_target
    mov rsi, rax
    mov rdi, rbx
    call os_rename
    test rax, rax
    js .Larchive_file_return
    xor eax, eax
.Larchive_file_return:
    add rsp, 8
    pop r12
    pop rbx
    ret
.Larchive_file_assert_null:
    mov edi, 4
    call assert_fail
.Larchive_file_assert_empty:
    mov edi, 5
    call assert_fail
.Larchive_file_assert_long:
    mov edi, 6
    call assert_fail
.size archive_file, . - archive_file

# Writes where a file goes in the archive: "archive/", its name, and a NUL.
# In: rdi = the name, rsi = its length, 1 to MAX_NAME.
# Out: rax = the path, in target.
.type build_target, @function
build_target:
    push rbx
    push r12
    sub rsp, 8
    test rdi, rdi
    jz .Lbuild_target_assert_null
    cmp rsi, MAX_NAME
    ja .Lbuild_target_assert_long
    mov rbx, rdi
    mov r12, rsi
    lea rdi, [rip + target]
    lea rsi, [rip + folder_prefix]
    mov edx, FOLDER_PREFIX_LEN
    call copy_bytes
    lea rdi, [rip + target + FOLDER_PREFIX_LEN]
    mov rsi, rbx
    mov rdx, r12
    call copy_bytes
    cmp r12, MAX_NAME_SCAN
    jae .Lbuild_target_assert_end
    lea rax, [rip + target]
    mov byte ptr [rax + r12 + FOLDER_PREFIX_LEN], 0
    add rsp, 8
    pop r12
    pop rbx
    ret
.Lbuild_target_assert_null:
    mov edi, 7
    call assert_fail
.Lbuild_target_assert_long:
    mov edi, 19
    call assert_fail
.Lbuild_target_assert_end:
    mov edi, 20
    call assert_fail
.size build_target, . - build_target

# Writes a phrase, a name and a newline to a file descriptor.
# In: edi = the file descriptor, rsi = the phrase, rdx = its length, at most MISSING_LEN,
# rcx = the name, r8 = its length, 1 to MAX_NAME.
# Out: rax = 0, or a negative error code.
# returns: result
.type report, @function
report:
    push rbx
    push r12
    push r13
    push r14
    push r15
    test rsi, rsi
    jz .Lreport_assert_null
    cmp rdx, MISSING_LEN
    ja .Lreport_assert_phrase
    test r8, r8
    jz .Lreport_assert_name
    cmp r8, MAX_NAME
    ja .Lreport_assert_name_long
    mov ebx, edi
    mov r12, rdx
    mov r13, rcx
    mov r14, r8
    lea r15, [rip + message]
    mov rdi, r15
    call copy_bytes
    lea rdi, [r15 + r12]
    mov rsi, r13
    mov rdx, r14
    call copy_bytes
    lea rcx, [r12 + r14]
    cmp rcx, MESSAGE_LEN
    jae .Lreport_assert_newline
    mov byte ptr [r15 + rcx], NEWLINE
    lea rdx, [rcx + 1]
    mov edi, ebx
    mov rsi, r15
    call write_all
    test rax, rax
    js .Lreport_return
    xor eax, eax
.Lreport_return:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Lreport_assert_null:
    mov edi, 8
    call assert_fail
.Lreport_assert_phrase:
    mov edi, 9
    call assert_fail
.Lreport_assert_name:
    mov edi, 10
    call assert_fail
.Lreport_assert_name_long:
    mov edi, 11
    call assert_fail
.Lreport_assert_newline:
    mov edi, 12
    call assert_fail
.size report, . - report

# Copies bytes from one buffer to another.
# In: rdi = where to, rsi = from where, rdx = how many, at most MAX_COPY.
.type copy_bytes, @function
copy_bytes:
    test rdi, rdi
    jz .Lcopy_bytes_assert_null
    cmp rdx, MAX_COPY
    ja .Lcopy_bytes_assert_count
    xor ecx, ecx
    mov r8, MAX_COPY
.Lcopy_bytes_loop:  # bound: 64
    cmp rcx, rdx
    jae .Lcopy_bytes_done
    cmp rcx, MAX_COPY
    jae .Lcopy_bytes_assert_index
    movzx eax, byte ptr [rsi + rcx]
    mov byte ptr [rdi + rcx], al
    inc rcx
    dec r8
    jnz .Lcopy_bytes_loop
.Lcopy_bytes_done:
    ret
.Lcopy_bytes_assert_null:
    mov edi, 13
    call assert_fail
.Lcopy_bytes_assert_count:
    mov edi, 14
    call assert_fail
.Lcopy_bytes_assert_index:
    mov edi, 15
    call assert_fail
.size copy_bytes, . - copy_bytes

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
    mov edi, 16
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 17
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 18
    call assert_fail
.size write_all, . - write_all

