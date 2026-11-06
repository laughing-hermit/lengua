.arch armv8-a

// File descriptors
.equ STDOUT, 1
.equ STDERR, 2
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_WRITE_FAILED, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
// Limits on what the kernel hands the program
.equ MAX_ARGS, 4096
.equ MAX_VARIABLES, 256
// "NAME=", the longest name, and how far the scan for its end goes
.equ PREFIX_LEN, 5
.equ MAX_NAME, 64
.equ MAX_NAME_SCAN, 65
// "Hello, ", the line's room, and the usage message
.equ HELLO_LEN, 7
.equ OUT_BUF_LEN, 73
.equ USAGE_LEN, 24
.equ MAX_WRITES, 64
.equ NEWLINE, 10

.section .rodata

.type prefix, @object
prefix:
    .ascii "NAME="
.size prefix, . - prefix

.type hello, @object
hello:
    .ascii "Hello, "
.size hello, . - hello

.type usage_msg, @object
usage_msg:
    .ascii "usage: NAME=<who> greet\n"
.size usage_msg, . - usage_msg

.section .bss

// The greeting: "Hello, ", the name, "!" and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
// Finds NAME in the environment and greets it; without one, or with one too long, prints the
// usage.
// lengua's `_start` calls main with the argument count, the array of arguments and the array of
// environment variables in x0, x1 and x2; main never returns.
.type main, @function
main:
    cmp x0, #MAX_ARGS
    b.hs .Lmain_assert_argc
    mov x0, x2
    bl find_name
    cbz x0, .Lmain_usage
    mov x19, x0
    bl name_length
    tbnz x0, #63, .Lmain_usage
    mov x1, x0
    mov x0, x19
    bl greet
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_usage:
    mov x0, #STDERR
    adrp x1, usage_msg
    add x1, x1, :lo12:usage_msg
    mov x2, #USAGE_LEN
    bl write_all
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_USAGE
    b .Lmain_exit
.Lmain_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_argc:
    mov w0, #1
    bl assert_fail
.size main, . - main

// Finds the value of NAME among the environment's variables.
// In: x0 = the environment, a list of strings that ends with 0.
// Out: x0 = NAME's value, or 0 where there's none.
.type find_name, @function
find_name:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x0, .Lfind_name_assert_null
    mov x19, x0
    mov x20, #0
    mov x21, #MAX_VARIABLES
.Lfind_name_loop:  // bound: 256
    cmp x20, #MAX_VARIABLES
    b.hs .Lfind_name_assert_index
    ldr x0, [x19, x20, lsl #3]
    cbz x0, .Lfind_name_none
    bl value_if_name
    cbnz x0, .Lfind_name_return
    add x20, x20, #1
    subs x21, x21, #1
    b.ne .Lfind_name_loop
.Lfind_name_none:
    mov x0, #0
.Lfind_name_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lfind_name_assert_null:
    mov w0, #2
    bl assert_fail
.Lfind_name_assert_index:
    mov w0, #3
    bl assert_fail
.size find_name, . - find_name

// The value of a variable that starts with "NAME=".
// In: x0 = the variable, "NAME=value" or another.
// Out: x0 = where its value starts, or 0 for another variable.
.type value_if_name, @function
value_if_name:
    cbz x0, .Lvalue_if_name_assert_null
    adrp x1, prefix
    add x1, x1, :lo12:prefix
    mov x2, #0
    mov x4, #PREFIX_LEN
.Lvalue_if_name_loop:  // bound: 5
    cmp x2, #PREFIX_LEN
    b.hs .Lvalue_if_name_assert_index
    ldrb w5, [x0, x2]
    ldrb w6, [x1, x2]
    cmp w5, w6
    b.ne .Lvalue_if_name_other
    add x2, x2, #1
    subs x4, x4, #1
    b.ne .Lvalue_if_name_loop
    add x0, x0, #PREFIX_LEN
    b .Lvalue_if_name_return
.Lvalue_if_name_other:
    mov x0, #0
.Lvalue_if_name_return:
    ret
.Lvalue_if_name_assert_null:
    mov w0, #4
    bl assert_fail
.Lvalue_if_name_assert_index:
    mov w0, #5
    bl assert_fail
.size value_if_name, . - value_if_name

// The length of a name, which ends with 0.
// In: x0 = the name.
// Out: x0 = its length, 1 to MAX_NAME, or a negative error code for none or one too long.
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
    add x2, x2, #1
    subs x4, x4, #1
    b.ne .Lname_length_loop
    mov x0, #ERROR_INVALID
    b .Lname_length_return
.Lname_length_end:
    mov x0, x2
    cbnz x2, .Lname_length_return
    mov x0, #ERROR_INVALID
.Lname_length_return:
    ret
.Lname_length_assert_null:
    mov w0, #6
    bl assert_fail
.Lname_length_assert_index:
    mov w0, #7
    bl assert_fail
.size name_length, . - name_length

// Prints "Hello, ", the name, "!" and a newline.
// In: x0 = the name; x1 = its length, 1 to MAX_NAME.
// Out: x0 = 0, or a negative error code.
// returns: result
.type greet, @function
greet:
    stp x29, x30, [sp, #-16]!
    cbz x0, .Lgreet_assert_null
    cbz x1, .Lgreet_assert_empty
    cmp x1, #MAX_NAME
    b.hi .Lgreet_assert_long
    adrp x2, out_buf
    add x2, x2, :lo12:out_buf
    adrp x3, hello
    add x3, x3, :lo12:hello
    mov x4, #0
    mov x5, #HELLO_LEN
.Lgreet_hello:  // bound: 7
    cmp x4, #HELLO_LEN
    b.hs .Lgreet_assert_hello_index
    ldrb w6, [x3, x4]
    strb w6, [x2, x4]
    add x4, x4, #1
    subs x5, x5, #1
    b.ne .Lgreet_hello
    add x7, x2, #HELLO_LEN
    mov x4, #0
    mov x5, #MAX_NAME
.Lgreet_name:  // bound: 64
    cmp x4, x1
    b.hs .Lgreet_end
    cmp x4, #MAX_NAME
    b.hs .Lgreet_assert_name_index
    ldrb w6, [x0, x4]
    strb w6, [x7, x4]
    add x4, x4, #1
    subs x5, x5, #1
    b.ne .Lgreet_name
.Lgreet_end:
    cmp x4, #MAX_NAME_SCAN
    b.hs .Lgreet_assert_end_index
    add x9, x7, x4
    mov w6, #'!'
    strb w6, [x9]
    mov w6, #NEWLINE
    strb w6, [x9, #1]
    add x2, x4, #HELLO_LEN + 2
    adrp x1, out_buf
    add x1, x1, :lo12:out_buf
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lgreet_return
    mov x0, #0
.Lgreet_return:
    ldp x29, x30, [sp], #16
    ret
.Lgreet_assert_null:
    mov w0, #8
    bl assert_fail
.Lgreet_assert_empty:
    mov w0, #9
    bl assert_fail
.Lgreet_assert_long:
    mov w0, #14
    bl assert_fail
.Lgreet_assert_hello_index:
    mov w0, #15
    bl assert_fail
.Lgreet_assert_name_index:
    mov w0, #16
    bl assert_fail
.Lgreet_assert_end_index:
    mov w0, #13
    bl assert_fail
.size greet, . - greet

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

