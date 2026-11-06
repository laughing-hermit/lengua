.arch armv8-a

// File descriptors
.equ STDOUT, 1
// os_open's modes, and os_remove's kind of a file
.equ OPEN_READ, 0
.equ OPEN_FOLDER, 4
.equ REMOVE_FILE, 0
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_FAILED, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// A batch of the folder's entries, as os_list gives them: each its length, a multiple of 8, in
// its first 2 bytes, its kind in the third (0 for a file) and its NUL-terminated name from the
// fifth
.equ ENTRIES_LEN, 4096
.equ ENTRY_LEN_AT, 0
.equ ENTRY_KIND_AT, 2
.equ ENTRY_NAME_AT, 4
.equ ENTRY_MIN, 8
.equ LAST_ENTRY, 4088
.equ KIND_FILE, 0
// A file's details, as os_info gives them: its size in the first 8 bytes
.equ DETAILS_LEN, 16
.equ SIZE_AT, 0
// Limits
.equ MAX_BATCHES, 64
.equ MAX_ENTRIES, 512
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
// Deletes the empty files directly in the working folder and says how many.
.type main, @function
main:
    adrp x0, this_folder
    add x0, x0, :lo12:this_folder
    mov x1, #OPEN_FOLDER
    bl os_open
    tbnz x0, #63, .Lmain_failed
    mov x19, x0
    bl sweep_folder
    tbnz x0, #63, .Lmain_failed
    mov x20, x0
    mov x0, x19
    bl os_close
    tbnz x0, #63, .Lmain_failed
    mov x0, x20
    bl print_count
    tbnz x0, #63, .Lmain_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_failed:
    mov w0, #EXIT_FAILED
.Lmain_exit:
    bl os_exit
.size main, . - main

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
    bl os_list
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
    mov x23, #MAX_ENTRIES
.Lsweep_batch_loop:  // bound: 512
    cmp x21, x20
    b.hs .Lsweep_batch_done
    cmp x21, #LAST_ENTRY
    b.hi .Lsweep_batch_assert_record
    add x9, x19, x21
    ldrb w10, [x9, #ENTRY_KIND_AT]
    cmp w10, #KIND_FILE
    b.ne .Lsweep_batch_next
    add x0, x9, #ENTRY_NAME_AT
    bl delete_if_empty
    tbnz x0, #63, .Lsweep_batch_return
    add x22, x22, x0
.Lsweep_batch_next:
    cmp x21, #LAST_ENTRY
    b.hi .Lsweep_batch_assert_next
    add x9, x19, x21
    ldrh w10, [x9, #ENTRY_LEN_AT]
    cmp w10, #ENTRY_MIN
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
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Ldelete_if_empty_assert_null
    mov x19, x0
    mov x1, #OPEN_READ
    bl os_open
    tbnz x0, #63, .Ldelete_if_empty_return
    mov x20, x0
    adrp x1, details
    add x1, x1, :lo12:details
    bl os_info
    tbnz x0, #63, .Ldelete_if_empty_return
    adrp x9, details
    add x9, x9, :lo12:details
    ldr x21, [x9, #SIZE_AT]
    mov x0, x20
    bl os_close
    tbnz x0, #63, .Ldelete_if_empty_return
    mov x0, #0
    cbnz x21, .Ldelete_if_empty_return
    mov x0, x19
    mov x1, #REMOVE_FILE
    bl os_remove
    tbnz x0, #63, .Ldelete_if_empty_return
    mov x0, #1
.Ldelete_if_empty_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
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

