.arch armv8-a

// File descriptors and the working folder
.equ STDOUT, 1
.equ STDERR, 2
.equ AT_FDCWD, -100
// Opening to look, and the new folder's mode, rwxr-xr-x
.equ O_RDONLY, 0
.equ FOLDER_MODE, 0x1ed
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_MISSING, 2
.equ EXIT_FAILED, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_MISSING, -2
.equ ERROR_IO, -5
.equ ERROR_EXISTS, -17
.equ ERROR_INVALID, -22
// Sizes and limits
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

// Where the file goes: "archive/", its name and a 0.
.type target, @object
target:
    .zero TARGET_LEN
.size target, . - target

// The line it prints: a phrase, the file's name and a newline.
.type message, @object
message:
    .zero MESSAGE_LEN
.size message, . - message

.text
// Moves the file its one argument names into the folder archive, making the folder when it's
// missing, and says so.
.type main, @function
main:
    ldr x0, [sp]
    cmp x0, #ARGC_EXPECTED
    b.ne .Lmain_usage
    ldr x19, [sp, #16]
    cbz x19, .Lmain_assert_argv
    mov x0, x19
    bl name_length
    tbnz x0, #63, .Lmain_usage
    mov x20, x0
    mov x0, x19
    mov x1, x20
    bl archive_file
    cbz x0, .Lmain_archived
    cmp x0, #ERROR_MISSING
    b.ne .Lmain_failed
    mov x0, #STDERR
    adrp x1, missing_msg
    add x1, x1, :lo12:missing_msg
    mov x2, #MISSING_LEN
    mov x3, x19
    mov x4, x20
    bl report
    tbnz x0, #63, .Lmain_failed
    mov w0, #EXIT_MISSING
    b .Lmain_exit
.Lmain_archived:
    mov x0, #STDOUT
    adrp x1, archived_msg
    add x1, x1, :lo12:archived_msg
    mov x2, #ARCHIVED_LEN
    mov x3, x19
    mov x4, x20
    bl report
    tbnz x0, #63, .Lmain_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_usage:
    mov x0, #STDERR
    adrp x1, usage_msg
    add x1, x1, :lo12:usage_msg
    mov x2, #USAGE_LEN
    bl write_all
    tbnz x0, #63, .Lmain_failed
    mov w0, #EXIT_USAGE
    b .Lmain_exit
.Lmain_failed:
    mov w0, #EXIT_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_argv:
    mov w0, #1
    bl assert_fail
.size main, . - main

// The length of a file's name, 1 to MAX_NAME bytes with no slash.
// In: x0 = the name, ending with 0.
// Out: x0 = its length, or ERROR_INVALID for a name it can't take.
// returns: result
.type name_length, @function
name_length:
    cbz x0, .Lname_length_assert_null
    mov x2, #0
    mov x4, #MAX_NAME_SCAN
.Lname_length_loop:  // bound: 65
    cmp x2, #MAX_NAME_SCAN
    b.hs .Lname_length_assert_index
    ldrb w5, [x0, x2]
    cbz w5, .Lname_length_end
    cmp w5, #SLASH
    b.eq .Lname_length_invalid
    add x2, x2, #1
    subs x4, x4, #1
    b.ne .Lname_length_loop
    b .Lname_length_invalid
.Lname_length_end:
    cbz x2, .Lname_length_invalid
    mov x0, x2
    b .Lname_length_return
.Lname_length_invalid:
    mov x0, #ERROR_INVALID
.Lname_length_return:
    ret
.Lname_length_assert_null:
    mov w0, #2
    bl assert_fail
.Lname_length_assert_index:
    mov w0, #3
    bl assert_fail
.size name_length, . - name_length

// Moves a file into the folder archive, making the folder when it's missing.
// In: x0 = the file's name; x1 = its length, 1 to MAX_NAME.
// Out: x0 = 0, ERROR_MISSING when the file isn't there, or another negative error code.
// returns: result
.type archive_file, @function
archive_file:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    cbz x0, .Larchive_file_assert_null
    cbz x1, .Larchive_file_assert_empty
    cmp x1, #MAX_NAME
    b.hi .Larchive_file_assert_long
    mov x19, x0
    mov x20, x1
    mov x1, x0
    mov x0, #AT_FDCWD
    mov x2, #O_RDONLY
    mov x3, #0
    mov x8, #SYS_OPENAT
    svc #0
    tbnz x0, #63, .Larchive_file_return
    bl os_close
    tbnz x0, #63, .Larchive_file_return
    mov x0, #AT_FDCWD
    adrp x1, folder_name
    add x1, x1, :lo12:folder_name
    mov x2, #FOLDER_MODE
    mov x8, #SYS_MKDIRAT
    svc #0
    cbz x0, .Larchive_file_folder
    cmp x0, #ERROR_EXISTS
    b.ne .Larchive_file_return
.Larchive_file_folder:
    mov x0, x19
    mov x1, x20
    bl build_target
    mov x3, x0
    mov x0, #AT_FDCWD
    mov x1, x19
    mov x2, #AT_FDCWD
    mov x8, #SYS_RENAMEAT
    svc #0
    tbnz x0, #63, .Larchive_file_return
    mov x0, #0
.Larchive_file_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Larchive_file_assert_null:
    mov w0, #4
    bl assert_fail
.Larchive_file_assert_empty:
    mov w0, #5
    bl assert_fail
.Larchive_file_assert_long:
    mov w0, #6
    bl assert_fail
.size archive_file, . - archive_file

// Builds "archive/" and the file's name, ending with 0, where it goes.
// In: x0 = the name; x1 = its length, up to MAX_NAME.
// Out: x0 = where it goes, target.
.type build_target, @function
build_target:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    cbz x0, .Lbuild_target_assert_null
    cmp x1, #MAX_NAME
    b.hi .Lbuild_target_assert_long
    mov x19, x0
    mov x20, x1
    adrp x0, target
    add x0, x0, :lo12:target
    adrp x1, folder_prefix
    add x1, x1, :lo12:folder_prefix
    mov x2, #FOLDER_PREFIX_LEN
    bl copy_bytes
    adrp x0, target
    add x0, x0, :lo12:target
    add x0, x0, #FOLDER_PREFIX_LEN
    mov x1, x19
    mov x2, x20
    bl copy_bytes
    cmp x20, #MAX_NAME_SCAN
    b.hs .Lbuild_target_assert_end
    adrp x0, target
    add x0, x0, :lo12:target
    add x9, x0, x20
    strb wzr, [x9, #FOLDER_PREFIX_LEN]
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lbuild_target_assert_null:
    mov w0, #7
    bl assert_fail
.Lbuild_target_assert_long:
    mov w0, #19
    bl assert_fail
.Lbuild_target_assert_end:
    mov w0, #20
    bl assert_fail
.size build_target, . - build_target

// Prints a phrase, the file's name and a newline.
// In: x0 = where, a file descriptor; x1 = the phrase; x2 = its length, up to MISSING_LEN; x3 =
// the name; x4 = its length, 1 to MAX_NAME.
// Out: x0 = 0, or a negative error code.
// returns: result
.type report, @function
report:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x1, .Lreport_assert_null
    cmp x2, #MISSING_LEN
    b.hi .Lreport_assert_phrase
    cbz x4, .Lreport_assert_name
    cmp x4, #MAX_NAME
    b.hi .Lreport_assert_name_long
    mov x19, x0
    mov x20, x2
    mov x21, x3
    mov x22, x4
    adrp x23, message
    add x23, x23, :lo12:message
    mov x0, x23
    bl copy_bytes
    add x0, x23, x20
    mov x1, x21
    mov x2, x22
    bl copy_bytes
    add x3, x20, x22
    cmp x3, #MESSAGE_LEN
    b.hs .Lreport_assert_newline
    mov w9, #NEWLINE
    strb w9, [x23, x3]
    add x2, x3, #1
    mov x0, x19
    mov x1, x23
    bl write_all
    tbnz x0, #63, .Lreport_return
    mov x0, #0
.Lreport_return:
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lreport_assert_null:
    mov w0, #8
    bl assert_fail
.Lreport_assert_phrase:
    mov w0, #9
    bl assert_fail
.Lreport_assert_name:
    mov w0, #16
    bl assert_fail
.Lreport_assert_name_long:
    mov w0, #17
    bl assert_fail
.Lreport_assert_newline:
    mov w0, #18
    bl assert_fail
.size report, . - report

// Copies bytes.
// In: x0 = where they go; x1 = where they are; x2 = how many, up to MAX_COPY.
.type copy_bytes, @function
copy_bytes:
    cbz x0, .Lcopy_bytes_assert_null
    cmp x2, #MAX_COPY
    b.hi .Lcopy_bytes_assert_count
    mov x3, #0
    mov x4, #MAX_COPY
.Lcopy_bytes_loop:  // bound: 64
    cmp x3, x2
    b.hs .Lcopy_bytes_done
    cmp x3, #MAX_COPY
    b.hs .Lcopy_bytes_assert_index
    ldrb w5, [x1, x3]
    strb w5, [x0, x3]
    add x3, x3, #1
    subs x4, x4, #1
    b.ne .Lcopy_bytes_loop
.Lcopy_bytes_done:
    ret
.Lcopy_bytes_assert_null:
    mov w0, #13
    bl assert_fail
.Lcopy_bytes_assert_count:
    mov w0, #14
    bl assert_fail
.Lcopy_bytes_assert_index:
    mov w0, #15
    bl assert_fail
.size copy_bytes, . - copy_bytes

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

