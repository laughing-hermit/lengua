.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_WRITE, 64
.equ SYS_CLOSE, 57
.equ SYS_FSTAT, 80
.equ SYS_EXIT, 93
.equ SYS_GETDENTS64, 61
.equ SYS_OPENAT, 56
.equ SYS_UNLINKAT, 35
// File descriptors and the working folder
.equ STDOUT, 1
.equ AT_FDCWD, -100
// How files open: read only, and a folder
.equ O_RDONLY, 0
.equ OPEN_FOLDER, 0x4000
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_FAILED, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// A batch of the folder's records: each its length at 16, its kind at 18 and its name at 19
.equ ENTRIES_LEN, 4096
.equ RECORD_LEN_AT, 16
.equ RECORD_KIND_AT, 18
.equ RECORD_NAME_AT, 19
.equ RECORD_MIN, 24
.equ LAST_RECORD, 4072
.equ KIND_FILE, 8
// A file's details, its size at 48
.equ DETAILS_LEN, 144
.equ SIZE_AT, 48
// Limits
.equ MAX_BATCHES, 64
.equ MAX_RECORDS, 171
.equ MAX_COUNT, 1000
// "deleted ", and the line it prints
.equ DELETED_LEN, 8
.equ OUT_BUF_LEN, 12
.equ MAX_WRITES, 64
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

// One batch of the folder's entries.
.type entries, @object
.balign 8
entries:
    .zero ENTRIES_LEN
.size entries, . - entries

// One file's details.
.type details, @object
.balign 8
details:
    .zero DETAILS_LEN
.size details, . - details

// What the program prints.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

// Deletes the empty files directly in the working folder and says how many.
.type _start, @function
_start:
    mov x0, #AT_FDCWD
    adrp x1, this_folder
    add x1, x1, :lo12:this_folder
    mov x2, #OPEN_FOLDER
    mov x3, #0
    mov x8, #SYS_OPENAT
    svc #0
    tbnz x0, #63, .Lstart_failed
    mov x19, x0
    bl sweep_folder
    tbnz x0, #63, .Lstart_failed
    mov x20, x0
    mov x0, x19
    mov x8, #SYS_CLOSE
    svc #0
    tbnz x0, #63, .Lstart_failed
    mov x0, x20
    bl print_count
    tbnz x0, #63, .Lstart_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_failed:
    mov w0, #EXIT_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.size _start, . - _start

// Deletes the empty files the open folder lists, a batch of its records at a time.
// In: x0 = the open folder.
// Out: x0 = how many it deleted, or a negative error code.
// returns: result
.type sweep_folder, @function
sweep_folder:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    tbnz x0, #63, .Lsweep_folder_assert_fd
    mov x19, x0
    mov x20, #0
    mov x21, #MAX_BATCHES
.Lsweep_folder_loop:  // bound: 64
    mov x0, x19
    adrp x1, entries
    add x1, x1, :lo12:entries
    mov x2, #ENTRIES_LEN
    mov x8, #SYS_GETDENTS64
    svc #0
    tbnz x0, #63, .Lsweep_folder_return
    cbz x0, .Lsweep_folder_done
    cmp x0, #ENTRIES_LEN
    b.hi .Lsweep_folder_assert_batch
    mov x1, x0
    adrp x0, entries
    add x0, x0, :lo12:entries
    bl sweep_batch
    tbnz x0, #63, .Lsweep_folder_return
    add x20, x20, x0
    subs x21, x21, #1
    b.ne .Lsweep_folder_loop
    mov x0, #ERROR_IO
    b .Lsweep_folder_return
.Lsweep_folder_done:
    mov x0, x20
.Lsweep_folder_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lsweep_folder_assert_fd:
    mov w0, #1
    bl assert_fail
.Lsweep_folder_assert_batch:
    mov w0, #2
    bl assert_fail
.size sweep_folder, . - sweep_folder

// Deletes the empty files among a batch of the folder's records.
// In: x0 = the records; x1 = their bytes, up to ENTRIES_LEN.
// Out: x0 = how many it deleted, or a negative error code.
// returns: result
.type sweep_batch, @function
sweep_batch:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    cbz x0, .Lsweep_batch_assert_null
    cmp x1, #ENTRIES_LEN
    b.hi .Lsweep_batch_assert_length
    mov x19, x0
    mov x20, x1
    mov x21, #0
    mov x22, #0
    mov x23, #MAX_RECORDS
.Lsweep_batch_loop:  // bound: 171
    cmp x21, x20
    b.hs .Lsweep_batch_done
    cmp x21, #LAST_RECORD
    b.hi .Lsweep_batch_assert_record
    add x9, x19, x21
    ldrb w10, [x9, #RECORD_KIND_AT]
    cmp w10, #KIND_FILE
    b.ne .Lsweep_batch_next
    add x0, x9, #RECORD_NAME_AT
    bl delete_if_empty
    tbnz x0, #63, .Lsweep_batch_return
    add x22, x22, x0
.Lsweep_batch_next:
    cmp x21, #LAST_RECORD
    b.hi .Lsweep_batch_assert_next
    add x9, x19, x21
    ldrh w10, [x9, #RECORD_LEN_AT]
    cmp w10, #RECORD_MIN
    b.lo .Lsweep_batch_assert_size
    add x21, x21, x10
    subs x23, x23, #1
    b.ne .Lsweep_batch_loop
.Lsweep_batch_done:
    mov x0, x22
.Lsweep_batch_return:
    ldp x23, x24, [sp], #16
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lsweep_batch_assert_null:
    mov w0, #3
    bl assert_fail
.Lsweep_batch_assert_length:
    mov w0, #4
    bl assert_fail
.Lsweep_batch_assert_record:
    mov w0, #5
    bl assert_fail
.Lsweep_batch_assert_next:
    mov w0, #6
    bl assert_fail
.Lsweep_batch_assert_size:
    mov w0, #7
    bl assert_fail
.size sweep_batch, . - sweep_batch

// Deletes a file when it's empty.
// In: x0 = its name, ending with 0.
// Out: x0 = 1 when it deleted it, 0 when it holds something, or a negative error code.
// returns: result
.type delete_if_empty, @function
delete_if_empty:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Ldelete_if_empty_assert_null
    mov x19, x0
    mov x1, x0
    mov x0, #AT_FDCWD
    mov x2, #O_RDONLY
    mov x3, #0
    mov x8, #SYS_OPENAT
    svc #0
    tbnz x0, #63, .Ldelete_if_empty_return
    mov x20, x0
    adrp x1, details
    add x1, x1, :lo12:details
    mov x8, #SYS_FSTAT
    svc #0
    tbnz x0, #63, .Ldelete_if_empty_return
    adrp x9, details
    add x9, x9, :lo12:details
    ldr x21, [x9, #SIZE_AT]
    mov x0, x20
    mov x8, #SYS_CLOSE
    svc #0
    tbnz x0, #63, .Ldelete_if_empty_return
    mov x0, #0
    cbnz x21, .Ldelete_if_empty_return
    mov x0, #AT_FDCWD
    mov x1, x19
    mov x2, #0
    mov x8, #SYS_UNLINKAT
    svc #0
    tbnz x0, #63, .Ldelete_if_empty_return
    mov x0, #1
.Ldelete_if_empty_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Ldelete_if_empty_assert_null:
    mov w0, #8
    bl assert_fail
.size delete_if_empty, . - delete_if_empty

// Prints "deleted ", a count from 0 to 999, and a newline.
// In: x0 = the count.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_count, @function
print_count:
    stp x29, x30, [sp, #-16]!
    cmp x0, #MAX_COUNT
    b.hs .Lprint_count_assert_count
    adrp x1, out_buf
    add x1, x1, :lo12:out_buf
    adrp x9, deleted_msg
    add x9, x9, :lo12:deleted_msg
    ldr x10, [x9]
    str x10, [x1]
    mov x11, #10
    mov x3, #DELETED_LEN
    cmp x0, #100
    b.lo .Lprint_count_tens
    mov x12, #100
    udiv x13, x0, x12
    msub x0, x13, x12, x0
    add w13, w13, #'0'
    strb w13, [x1, #DELETED_LEN]
    add x3, x3, #1
    b .Lprint_count_two
.Lprint_count_tens:
    cmp x0, #10
    b.lo .Lprint_count_last
.Lprint_count_two:
    udiv x13, x0, x11
    msub x0, x13, x11, x0
    add w13, w13, #'0'
    cmp x3, #OUT_BUF_LEN - 2
    b.hs .Lprint_count_assert_index
    strb w13, [x1, x3]
    add x3, x3, #1
.Lprint_count_last:
    add w0, w0, #'0'
    cmp x3, #OUT_BUF_LEN - 1
    b.hs .Lprint_count_assert_last
    strb w0, [x1, x3]
    add x9, x1, x3
    mov w10, #NEWLINE
    strb w10, [x9, #1]
    add x2, x3, #2
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lprint_count_return
    mov x0, #0
.Lprint_count_return:
    ldp x29, x30, [sp], #16
    ret
.Lprint_count_assert_count:
    mov w0, #9
    bl assert_fail
.Lprint_count_assert_index:
    mov w0, #14
    bl assert_fail
.Lprint_count_assert_last:
    mov w0, #15
    bl assert_fail
.size print_count, . - print_count

// Writes x2 bytes from x1 to file descriptor x0, retrying partial writes.
// In: x0 = the file descriptor, x1 = the bytes, x2 = how many, at least 1.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_all, @function
write_all:
    cbz x1, .Lwrite_all_assert_null
    cbz x2, .Lwrite_all_assert_empty
    mov x9, x0
    mov x10, #MAX_WRITES
.Lwrite_all_loop:  // bound: 64
    mov x0, x9
    mov x8, #SYS_WRITE
    svc #0
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

// lengua's assertion handler. check compares it word for word: copy it exactly.
// In: w0 = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
// standard error and ends the program, every thread of it, with code 134.
.type assert_fail, @function
assert_fail:
    sub sp, sp, #32
    adrp x1, assert_message
    add x1, x1, :lo12:assert_message
    ldp x2, x3, [x1]
    stp x2, x3, [sp]
    ldr x2, [x1, #16]
    str x2, [sp, #16]
    ldr w2, [x1, #24]
    str w2, [sp, #24]
    ldrb w2, [x1, #28]
    strb w2, [sp, #28]
    mov w2, #100
    udiv w3, w0, w2
    msub w0, w3, w2, w0
    add w3, w3, #48
    strb w3, [sp, #18]
    mov w2, #10
    udiv w3, w0, w2
    msub w0, w3, w2, w0
    add w3, w3, #48
    strb w3, [sp, #19]
    add w0, w0, #48
    strb w0, [sp, #20]
    mov x0, #2
    mov x1, sp
    mov x2, #29
    mov x8, #64
    svc #0
    mov x0, #134
    mov x8, #94
    svc #0
.size assert_fail, . - assert_fail

