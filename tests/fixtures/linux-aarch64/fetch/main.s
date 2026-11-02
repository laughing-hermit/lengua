.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_READ, 63
.equ SYS_WRITE, 64
.equ SYS_CLOSE, 57
.equ SYS_EXIT, 93
// File descriptors
.equ STDOUT, 1
.equ STDERR, 2
// The connection lengua makes to the server before the program starts: its first, 4.
.equ CONNECTION, 4
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_UNREACHABLE, 3
.equ EXIT_WRITE_FAILED, 4
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
// Sizes and limits
.equ ARGC_EXPECTED, 2
.equ MAX_WRITES, 64
.equ PATH_MAX, 100
.equ PATH_SCAN, 101
.equ GET_LEN, 4
.equ VERSION_LEN, 13
.equ REQUEST_LEN, 117
.equ RESPONSE_LEN, 65536
// fill's limit on reads, as fill.s has it
.equ MAX_FILL_READS, 1099511627776
// The blank line ending a response's headers, "\r\n\r\n", read as one 32-bit word, and the
// places a word of it can start in the response.
.equ HEADER_END, 0x0a0d0a0d
.equ HEADER_END_BYTES, 4
.equ BODY_SCAN, RESPONSE_LEN - HEADER_END_BYTES + 1
.equ USAGE_MSG_LEN, 20
.equ UNREACHABLE_MSG_LEN, 30

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: fetch <path>\n"
.size usage_msg, . - usage_msg

.type unreachable_msg, @object
unreachable_msg:
    .ascii "error: can't reach the server\n"
.size unreachable_msg, . - unreachable_msg

// The request's first and last parts, around the path.
.type get, @object
get:
    .ascii "GET "
.size get, . - get

.type version, @object
version:
    .ascii " HTTP/1.0\r\n\r\n"
.size version, . - version

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// The request: "GET ", the path and " HTTP/1.0" with a blank line.
.type request, @object
request:
    .zero REQUEST_LEN
.size request, . - request

// Everything the server sends.
.type response, @object
response:
    .zero RESPONSE_LEN
.size response, . - response

.text
.globl _start

// Checks the arguments, fetches the page and prints its body, then exits.
.type _start, @function
_start:
    ldr x9, [sp]
    cmp x9, #ARGC_EXPECTED
    b.ne .Lstart_usage
    ldr x0, [sp, #16]
    cbz x0, .Lstart_assert_path
    adrp x1, request
    add x1, x1, :lo12:request
    bl build_request
    tbnz x0, #63, .Lstart_usage
    mov x2, x1
    adrp x1, request
    add x1, x1, :lo12:request
    adrp x3, response
    add x3, x3, :lo12:response
    bl fetch_page
    tbnz x0, #63, .Lstart_unreachable
    adrp x0, response
    add x0, x0, :lo12:response
    bl print_body
    tbnz x0, #63, .Lstart_write_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_usage:
    adrp x1, usage_msg
    add x1, x1, :lo12:usage_msg
    mov x2, #USAGE_MSG_LEN
    bl complain
    mov w0, #EXIT_USAGE
    b .Lstart_exit
.Lstart_unreachable:
    adrp x1, unreachable_msg
    add x1, x1, :lo12:unreachable_msg
    mov x2, #UNREACHABLE_MSG_LEN
    bl complain
    mov w0, #EXIT_UNREACHABLE
    b .Lstart_exit
.Lstart_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_path:
    mov w0, #2
    bl assert_fail
.size _start, . - _start

// Writes a message to standard error, or exits with EXIT_WRITE_FAILED when it can't.
// In: x1 = the message; x2 = its length, at least 1.
.type complain, @function
complain:
    stp x29, x30, [sp, #-16]!
    cbz x1, .Lcomplain_assert_message
    cbz x2, .Lcomplain_assert_empty
    mov x0, #STDERR
    bl write_all
    tbnz x0, #63, .Lcomplain_failed
    ldp x29, x30, [sp], #16
    ret
.Lcomplain_failed:
    mov w0, #EXIT_WRITE_FAILED
    mov x8, #SYS_EXIT
    svc #0
.Lcomplain_assert_message:
    mov w0, #22
    bl assert_fail
.Lcomplain_assert_empty:
    mov w0, #23
    bl assert_fail
.size complain, . - complain

// Prints a response's body, everything after its first blank line, to standard output.
// In: x0 = the response; x1 = its length, at most RESPONSE_LEN.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_body, @function
print_body:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    cbz x0, .Lprint_body_assert_response
    cmp x1, #RESPONSE_LEN
    b.hi .Lprint_body_assert_length
    mov x19, x0
    mov x20, x1
    bl find_body
    subs x2, x20, x0
    b.eq .Lprint_body_empty
    add x1, x19, x0
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lprint_body_return
.Lprint_body_empty:
    mov x0, #0
.Lprint_body_return:
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lprint_body_assert_response:
    mov w0, #24
    bl assert_fail
.Lprint_body_assert_length:
    mov w0, #25
    bl assert_fail
.size print_body, . - print_body

// Writes "GET <path> HTTP/1.0", a line break and a blank line into a buffer of REQUEST_LEN bytes.
// In: x0 = the path, NUL-terminated; x1 = the buffer.
// Out: x0 = 0 and x1 = the request's length, or x0 = ERROR_INVALID when the path is empty or
// longer than PATH_MAX.
// returns: result
.type build_request, @function
build_request:
    cbz x0, .Lbuild_request_assert_path
    cbz x1, .Lbuild_request_assert_buffer
    adrp x9, get
    add x9, x9, :lo12:get
    ldr w10, [x9]
    str w10, [x1]
    mov x11, #0
    mov x12, #PATH_SCAN
.Lbuild_request_copy:  // bound: 101
    cmp x11, #PATH_SCAN
    b.hs .Lbuild_request_assert_scan
    ldrb w13, [x0, x11]
    cbz w13, .Lbuild_request_end
    cmp x11, #PATH_MAX
    b.hs .Lbuild_request_invalid
    add x14, x11, #GET_LEN
    cmp x14, #REQUEST_LEN
    b.hs .Lbuild_request_assert_index
    strb w13, [x1, x14]
    add x11, x11, #1
    subs x12, x12, #1
    b.ne .Lbuild_request_copy
    b .Lbuild_request_invalid
.Lbuild_request_end:
    cbz x11, .Lbuild_request_invalid
    cmp x11, #PATH_SCAN
    b.hs .Lbuild_request_assert_end
    adrp x9, version
    add x9, x9, :lo12:version
    ldr x10, [x9]
    add x14, x1, x11
    stur x10, [x14, #GET_LEN]
    ldur x10, [x9, #VERSION_LEN - 8]
    stur x10, [x14, #GET_LEN + VERSION_LEN - 8]
    add x1, x11, #GET_LEN + VERSION_LEN
    mov x0, #0
    b .Lbuild_request_return
.Lbuild_request_invalid:
    mov x0, #ERROR_INVALID
.Lbuild_request_return:
    ret
.Lbuild_request_assert_path:
    mov w0, #3
    bl assert_fail
.Lbuild_request_assert_buffer:
    mov w0, #4
    bl assert_fail
.Lbuild_request_assert_scan:
    mov w0, #5
    bl assert_fail
.Lbuild_request_assert_index:
    mov w0, #6
    bl assert_fail
.Lbuild_request_assert_end:
    mov w0, #18
    bl assert_fail
.size build_request, . - build_request

// Sends a request on the connection lengua made to the server, and reads the whole response,
// until the server closes the connection.
// In: x1 = the request; x2 = its length; x3 = a buffer of RESPONSE_LEN bytes.
// Out: x0 = 0 and x1 = the response's length, or a negative error code.
// returns: result
.type fetch_page, @function
fetch_page:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    cbz x1, .Lfetch_page_assert_request
    cbz x3, .Lfetch_page_assert_buffer
    mov x20, x1
    mov x21, x2
    mov x22, x3
    mov x0, #CONNECTION
    bl write_all
    tbnz x0, #63, .Lfetch_page_close
    mov x0, #CONNECTION
    mov x1, x22
    bl receive
    tbnz x0, #63, .Lfetch_page_close
    mov x20, x1
.Lfetch_page_close:
    mov x21, x0
    mov x0, #CONNECTION
    mov x8, #SYS_CLOSE
    svc #0
    tbnz x0, #63, .Lfetch_page_result
.Lfetch_page_result:
    mov x0, x21
    mov x1, x20
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lfetch_page_assert_request:
    mov w0, #7
    bl assert_fail
.Lfetch_page_assert_buffer:
    mov w0, #8
    bl assert_fail
.size fetch_page, . - fetch_page

// Reads from a connection until the server closes it, into a buffer of RESPONSE_LEN bytes.
// In: x0 = the connection; x1 = the buffer.
// Out: x0 = 0 and x1 = how many bytes came, or a negative error code (ERROR_IO when the
// response doesn't fit).
// returns: result
.type receive, @function
receive:
    stp x29, x30, [sp, #-16]!
    cbz x1, .Lreceive_assert_buffer
    mov x2, #RESPONSE_LEN
    mov w3, #-1
    bl fill
    tbnz x0, #63, .Lreceive_return
    cmp x0, #RESPONSE_LEN
    b.hs .Lreceive_too_long
    mov x1, x0
    mov x0, #0
    b .Lreceive_return
.Lreceive_too_long:
    mov x0, #ERROR_IO
.Lreceive_return:
    ldp x29, x30, [sp], #16
    ret
.Lreceive_assert_buffer:
    mov w0, #11
    bl assert_fail
.size receive, . - receive

// Finds where a response's body starts: just after the first blank line.
// In: x0 = the response; x1 = its length, at most RESPONSE_LEN.
// Out: x0 = the body's offset, or the length when there's no blank line.
.type find_body, @function
find_body:
    cbz x0, .Lfind_body_assert_response
    cmp x1, #RESPONSE_LEN
    b.hi .Lfind_body_assert_length
    mov x9, x0
    mov x0, x1
    mov x10, #0
    mov w11, #HEADER_END & 0xffff
    movk w11, #HEADER_END >> 16, lsl #16
    mov x15, #BODY_SCAN
    mov x12, #RESPONSE_LEN
.Lfind_body_loop:  // bound: 65536
    add x13, x10, #HEADER_END_BYTES
    cmp x13, x1
    b.hi .Lfind_body_return
    cmp x10, x15
    b.hs .Lfind_body_assert_index
    ldr w14, [x9, x10]
    cmp w14, w11
    b.eq .Lfind_body_found
    add x10, x10, #1
    subs x12, x12, #1
    b.ne .Lfind_body_loop
    b .Lfind_body_return
.Lfind_body_found:
    add x0, x10, #HEADER_END_BYTES
.Lfind_body_return:
    ret
.Lfind_body_assert_response:
    mov w0, #15
    bl assert_fail
.Lfind_body_assert_length:
    mov w0, #16
    bl assert_fail
.Lfind_body_assert_index:
    mov w0, #17
    bl assert_fail
.size find_body, . - find_body

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
    mov w0, #19
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #20
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #21
    bl assert_fail
.size write_all, . - write_all

// Reads from file descriptor x0 into the x2 bytes at x1 until they're full, the input ends,
// or, when w3 is a byte (0 to 255), the last byte read is w3; with w3 = -1 it reads to the
// end. However the input arrives, it reads the same.
// In: x0 = the file descriptor, x1 = the buffer, x2 = its size, at least 1, w3 = the end
// byte or -1.
// Out: x0 = how many bytes it read, or a negative error code.
// returns: result
.type fill, @function
fill:
    cbz x1, .Lfill_assert_null
    cbz x2, .Lfill_assert_empty
    mov x9, x0
    mov x10, x2
    mov w11, w3
    mov x12, #MAX_FILL_READS
.Lfill_loop:  // bound: 1099511627776
    mov x0, x9
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lfill_return
    cbz x0, .Lfill_done
    cmp x0, x2
    b.hi .Lfill_assert_count
    add x1, x1, x0
    subs x2, x2, x0
    b.eq .Lfill_done
    ldurb w13, [x1, #-1]
    cmp w13, w11
    b.eq .Lfill_done
    subs x12, x12, #1
    b.ne .Lfill_loop
    mov x0, #ERROR_IO
    b .Lfill_return
.Lfill_done:
    sub x0, x10, x2
.Lfill_return:
    ret
.Lfill_assert_null:
    mov w0, #990
    bl assert_fail
.Lfill_assert_empty:
    mov w0, #991
    bl assert_fail
.Lfill_assert_count:
    mov w0, #992
    bl assert_fail
.size fill, . - fill

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
