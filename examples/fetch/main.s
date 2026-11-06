.intel_syntax noprefix

# File descriptors
.equ STDOUT, 1
.equ STDERR, 2
# The connection lengua makes to the server before the program starts: its first, 4.
.equ CONNECTION, 4
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_UNREACHABLE, 3
.equ EXIT_WRITE_FAILED, 4
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
# Sizes and limits
.equ ARGC_EXPECTED, 2
.equ MAX_WRITES, 64
.equ PATH_MAX, 100
.equ PATH_SCAN, 101
.equ GET_LEN, 4
.equ VERSION_LEN, 13
.equ REQUEST_LEN, 117
.equ RESPONSE_LEN, 65536
# The blank line ending a response's headers, "\r\n\r\n", read as one 32-bit word.
.equ HEADER_END, 0x0a0d0a0d
.equ HEADER_END_BYTES, 4
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

# The request's first and last parts, around the path.
.type get, @object
get:
    .ascii "GET "
.size get, . - get

.type version, @object
version:
    .ascii " HTTP/1.0\r\n\r\n"
.size version, . - version

.section .bss

# The request: "GET ", the path and " HTTP/1.0" with a blank line.
.type request, @object
request:
    .zero REQUEST_LEN
.size request, . - request

# Everything the server sends.
.type response, @object
response:
    .zero RESPONSE_LEN
.size response, . - response

.text
# Checks the arguments, fetches the page and prints its body, then exits.
.type main, @function
main:
    sub rsp, 8
    cmp rdi, ARGC_EXPECTED
    jne .Lmain_usage
    mov rdi, qword ptr [rsi + 8]
    test rdi, rdi
    jz .Lmain_assert_path
    lea rsi, [rip + request]
    call build_request
    test rax, rax
    js .Lmain_usage
    lea rsi, [rip + request]
    lea rcx, [rip + response]
    call fetch_page
    test rax, rax
    js .Lmain_unreachable
    lea rdi, [rip + response]
    mov rsi, rdx
    call print_body
    test rax, rax
    js .Lmain_write_failed
    mov edi, EXIT_OK
    jmp .Lmain_exit
.Lmain_usage:
    lea rsi, [rip + usage_msg]
    mov edx, USAGE_MSG_LEN
    call complain
    mov edi, EXIT_USAGE
    jmp .Lmain_exit
.Lmain_unreachable:
    lea rsi, [rip + unreachable_msg]
    mov edx, UNREACHABLE_MSG_LEN
    call complain
    mov edi, EXIT_UNREACHABLE
    jmp .Lmain_exit
.Lmain_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lmain_exit:
    call os_exit
.Lmain_assert_path:
    mov edi, 2
    call assert_fail
.size main, . - main

# Writes a message to standard error, or exits with EXIT_WRITE_FAILED when it can't.
# In: rsi = the message; edx = its length, at least 1.
.type complain, @function
complain:
    sub rsp, 8
    test rsi, rsi
    jz .Lcomplain_assert_message
    test edx, edx
    jz .Lcomplain_assert_empty
    mov edi, STDERR
    call write_all
    test rax, rax
    js .Lcomplain_failed
    add rsp, 8
    ret
.Lcomplain_failed:
    mov edi, EXIT_WRITE_FAILED
    call os_exit
.Lcomplain_assert_message:
    mov edi, 22
    call assert_fail
.Lcomplain_assert_empty:
    mov edi, 23
    call assert_fail
.size complain, . - complain

# Prints a response's body, everything after its first blank line, to standard output.
# In: rdi = the response; rsi = its length, at most RESPONSE_LEN.
# Out: rax = 0, or a negative error code.
# returns: result
.type print_body, @function
print_body:
    push rbx
    push r12
    sub rsp, 8
    test rdi, rdi
    jz .Lprint_body_assert_response
    cmp rsi, RESPONSE_LEN
    ja .Lprint_body_assert_length
    mov rbx, rdi
    mov r12, rsi
    call find_body
    mov rdx, r12
    sub rdx, rax
    jz .Lprint_body_empty
    lea rsi, [rbx + rax]
    mov edi, STDOUT
    call write_all
    test rax, rax
    js .Lprint_body_return
.Lprint_body_empty:
    xor eax, eax
.Lprint_body_return:
    add rsp, 8
    pop r12
    pop rbx
    ret
.Lprint_body_assert_response:
    mov edi, 24
    call assert_fail
.Lprint_body_assert_length:
    mov edi, 25
    call assert_fail
.size print_body, . - print_body

# Writes "GET <path> HTTP/1.0", a line break and a blank line into a buffer of REQUEST_LEN bytes.
# In: rdi = the path, NUL-terminated; rsi = the buffer.
# Out: rax = 0 and rdx = the request's length, or rax = ERROR_INVALID when the path is empty or
# longer than PATH_MAX.
# returns: result
.type build_request, @function
build_request:
    test rdi, rdi
    jz .Lbuild_request_assert_path
    test rsi, rsi
    jz .Lbuild_request_assert_buffer
    mov eax, dword ptr [rip + get]
    mov dword ptr [rsi], eax
    xor ecx, ecx
    mov r8, PATH_SCAN
.Lbuild_request_copy:  # bound: 101
    cmp rcx, PATH_SCAN
    jae .Lbuild_request_assert_scan
    movzx edx, byte ptr [rdi + rcx]
    test edx, edx
    jz .Lbuild_request_end
    cmp rcx, PATH_MAX
    jae .Lbuild_request_invalid
    lea r9, [rcx + GET_LEN]
    cmp r9, REQUEST_LEN
    jae .Lbuild_request_assert_index
    mov byte ptr [rsi + r9], dl
    inc rcx
    dec r8
    jnz .Lbuild_request_copy
    jmp .Lbuild_request_invalid
.Lbuild_request_end:
    test rcx, rcx
    jz .Lbuild_request_invalid
    cmp rcx, PATH_SCAN
    jae .Lbuild_request_assert_end
    mov rax, qword ptr [rip + version]
    mov qword ptr [rsi + rcx + GET_LEN], rax
    mov rax, qword ptr [rip + version + VERSION_LEN - 8]
    mov qword ptr [rsi + rcx + GET_LEN + VERSION_LEN - 8], rax
    lea rdx, [rcx + GET_LEN + VERSION_LEN]
    xor eax, eax
    jmp .Lbuild_request_return
.Lbuild_request_invalid:
    mov rax, ERROR_INVALID
.Lbuild_request_return:
    ret
.Lbuild_request_assert_path:
    mov edi, 3
    call assert_fail
.Lbuild_request_assert_buffer:
    mov edi, 4
    call assert_fail
.Lbuild_request_assert_scan:
    mov edi, 5
    call assert_fail
.Lbuild_request_assert_index:
    mov edi, 6
    call assert_fail
.Lbuild_request_assert_end:
    mov edi, 18
    call assert_fail
.size build_request, . - build_request

# Sends a request on the connection lengua made to the server, and reads the whole response,
# until the server closes the connection.
# In: rsi = the request; rdx = its length; rcx = a buffer of RESPONSE_LEN bytes.
# Out: rax = 0 and rdx = the response's length, or a negative error code.
# returns: result
.type fetch_page, @function
fetch_page:
    push r12
    push r13
    push r14
    test rsi, rsi
    jz .Lfetch_page_assert_request
    test rcx, rcx
    jz .Lfetch_page_assert_buffer
    mov r12, rsi
    mov r13, rdx
    mov r14, rcx
    mov edi, CONNECTION
    call write_all
    test rax, rax
    js .Lfetch_page_close
    mov edi, CONNECTION
    mov rsi, r14
    call receive
    test rax, rax
    js .Lfetch_page_close
    mov r12, rdx
.Lfetch_page_close:
    mov r13, rax
    mov edi, CONNECTION
    call os_close
    test rax, rax
    js .Lfetch_page_result
.Lfetch_page_result:
    mov rax, r13
    mov rdx, r12
    pop r14
    pop r13
    pop r12
    ret
.Lfetch_page_assert_request:
    mov edi, 7
    call assert_fail
.Lfetch_page_assert_buffer:
    mov edi, 8
    call assert_fail
.size fetch_page, . - fetch_page

# Reads from a connection until the server closes it, into a buffer of RESPONSE_LEN bytes.
# In: edi = the connection; rsi = the buffer.
# Out: rax = 0 and rdx = how many bytes came, or a negative error code (ERROR_IO when the
# response doesn't fit).
# returns: result
.type receive, @function
receive:
    push rbx
    test rsi, rsi
    jz .Lreceive_assert_buffer
    mov edx, RESPONSE_LEN
    mov ecx, -1
    call fill
    test rax, rax
    js .Lreceive_return
    cmp rax, RESPONSE_LEN
    jae .Lreceive_too_long
    mov rdx, rax
    xor eax, eax
    jmp .Lreceive_return
.Lreceive_too_long:
    mov rax, ERROR_IO
.Lreceive_return:
    pop rbx
    ret
.Lreceive_assert_buffer:
    mov edi, 11
    call assert_fail
.size receive, . - receive

# Finds where a response's body starts: just after the first blank line.
# In: rdi = the response; rsi = its length, at most RESPONSE_LEN.
# Out: rax = the body's offset, or the length when there's no blank line.
.type find_body, @function
find_body:
    test rdi, rdi
    jz .Lfind_body_assert_response
    cmp rsi, RESPONSE_LEN
    ja .Lfind_body_assert_length
    mov rax, rsi
    xor ecx, ecx
    mov r8, RESPONSE_LEN
.Lfind_body_loop:  # bound: 65536
    lea rdx, [rcx + HEADER_END_BYTES]
    cmp rdx, rsi
    ja .Lfind_body_return
    cmp rcx, RESPONSE_LEN - HEADER_END_BYTES + 1
    jae .Lfind_body_assert_index
    cmp dword ptr [rdi + rcx], HEADER_END
    je .Lfind_body_found
    inc rcx
    dec r8
    jnz .Lfind_body_loop
    jmp .Lfind_body_return
.Lfind_body_found:
    lea rax, [rcx + HEADER_END_BYTES]
.Lfind_body_return:
    ret
.Lfind_body_assert_response:
    mov edi, 15
    call assert_fail
.Lfind_body_assert_length:
    mov edi, 16
    call assert_fail
.Lfind_body_assert_index:
    mov edi, 17
    call assert_fail
.size find_body, . - find_body

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
    mov edi, 19
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 20
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 21
    call assert_fail
.size write_all, . - write_all

