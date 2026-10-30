.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_READ, 0
.equ SYS_WRITE, 1
.equ SYS_CLOSE, 3
.equ SYS_SOCKET, 41
.equ SYS_CONNECT, 42
.equ SYS_EXIT, 60
# File descriptors
.equ STDOUT, 1
.equ STDERR, 2
# Sockets: TCP over IPv4
.equ AF_INET, 2
.equ SOCK_STREAM, 1
# Exit codes
.equ EXIT_OK, 0
.equ EXIT_USAGE, 1
.equ EXIT_BAD_PORT, 2
.equ EXIT_UNREACHABLE, 3
.equ EXIT_WRITE_FAILED, 4
# Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ ERROR_INVALID, -22
# Sizes and limits
.equ ARGC_EXPECTED, 3
.equ PORT_MIN, 1
.equ PORT_MAX, 65535
.equ MAX_CHARS, 20
.equ MAX_WRITES, 64
.equ PATH_MAX, 100
.equ PATH_SCAN, 101
.equ GET_LEN, 4
.equ VERSION_LEN, 13
.equ REQUEST_LEN, 117
.equ RESPONSE_LEN, 65536
# fill's limit on reads, as assets/fill.s has it
.equ MAX_FILL_READS, 1099511627776
.equ ADDRESS_LEN, 16
.equ ADDRESS_PAD, 8
# The blank line ending a response's headers, "\r\n\r\n", read as one 32-bit word.
.equ HEADER_END, 0x0a0d0a0d
.equ HEADER_END_BYTES, 4
.equ USAGE_MSG_LEN, 27
.equ BAD_PORT_MSG_LEN, 16
.equ UNREACHABLE_MSG_LEN, 30

.section .rodata

.type usage_msg, @object
usage_msg:
    .ascii "usage: fetch <port> <path>\n"
.size usage_msg, . - usage_msg

.type bad_port_msg, @object
bad_port_msg:
    .ascii "error: bad port\n"
.size bad_port_msg, . - bad_port_msg

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

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .data

# The server's address: IPv4, its port (set before connecting), 127.0.0.1.
.type address, @object
address:
    .short AF_INET
    .short 0
    .byte 127, 0, 0, 1
    .zero ADDRESS_PAD
.size address, . - address

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
.globl _start

# Checks the arguments, fetches the page and prints its body, then exits.
.type _start, @function
_start:
    mov rax, qword ptr [rsp]
    cmp rax, ARGC_EXPECTED
    jne .Lstart_usage
    mov rdi, qword ptr [rsp + 16]
    test rdi, rdi
    jz .Lstart_assert_port
    call parse_i64
    test rax, rax
    js .Lstart_bad_port
    cmp rdx, PORT_MIN
    jl .Lstart_bad_port
    cmp rdx, PORT_MAX
    jg .Lstart_bad_port
    mov rbx, rdx
    mov rdi, qword ptr [rsp + 24]
    test rdi, rdi
    jz .Lstart_assert_path
    lea rsi, [rip + request]
    call build_request
    test rax, rax
    js .Lstart_usage
    mov edi, ebx
    lea rsi, [rip + request]
    lea rcx, [rip + response]
    call fetch_page
    test rax, rax
    js .Lstart_unreachable
    lea rdi, [rip + response]
    mov rsi, rdx
    call print_body
    test rax, rax
    js .Lstart_write_failed
    mov edi, EXIT_OK
    jmp .Lstart_exit
.Lstart_usage:
    lea rsi, [rip + usage_msg]
    mov edx, USAGE_MSG_LEN
    call complain
    mov edi, EXIT_USAGE
    jmp .Lstart_exit
.Lstart_bad_port:
    lea rsi, [rip + bad_port_msg]
    mov edx, BAD_PORT_MSG_LEN
    call complain
    mov edi, EXIT_BAD_PORT
    jmp .Lstart_exit
.Lstart_unreachable:
    lea rsi, [rip + unreachable_msg]
    mov edx, UNREACHABLE_MSG_LEN
    call complain
    mov edi, EXIT_UNREACHABLE
    jmp .Lstart_exit
.Lstart_write_failed:
    mov edi, EXIT_WRITE_FAILED
.Lstart_exit:
    mov eax, SYS_EXIT
    syscall
.Lstart_assert_port:
    mov edi, 1
    call assert_fail
.Lstart_assert_path:
    mov edi, 2
    call assert_fail
.size _start, . - _start

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
    mov eax, SYS_EXIT
    syscall
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

# Sends a request to a port on this computer and reads the whole response, until the server
# closes the connection.
# In: edi = the port; rsi = the request; rdx = its length; rcx = a buffer of RESPONSE_LEN bytes.
# Out: rax = 0 and rdx = the response's length, or a negative error code.
# returns: result
.type fetch_page, @function
fetch_page:
    push rbx
    push r12
    push r13
    push r14
    sub rsp, 8
    test rsi, rsi
    jz .Lfetch_page_assert_request
    test rcx, rcx
    jz .Lfetch_page_assert_buffer
    mov r12, rsi
    mov r13, rdx
    mov r14, rcx
    call connect_local
    test rax, rax
    js .Lfetch_page_return
    mov rbx, rax
    mov edi, ebx
    mov rsi, r12
    mov rdx, r13
    call write_all
    test rax, rax
    js .Lfetch_page_close
    mov edi, ebx
    mov rsi, r14
    call receive
    test rax, rax
    js .Lfetch_page_close
    mov r12, rdx
.Lfetch_page_close:
    mov r13, rax
    mov eax, SYS_CLOSE
    mov edi, ebx
    syscall
    test rax, rax
    js .Lfetch_page_result
.Lfetch_page_result:
    mov rax, r13
    mov rdx, r12
.Lfetch_page_return:
    add rsp, 8
    pop r14
    pop r13
    pop r12
    pop rbx
    ret
.Lfetch_page_assert_request:
    mov edi, 7
    call assert_fail
.Lfetch_page_assert_buffer:
    mov edi, 8
    call assert_fail
.size fetch_page, . - fetch_page

# Opens a TCP connection to a port on this computer, 127.0.0.1.
# In: edi = the port, from PORT_MIN to PORT_MAX.
# Out: rax = the connection's file descriptor, or a negative error code.
# returns: result
.type connect_local, @function
connect_local:
    push rbx
    test edi, edi
    jz .Lconnect_local_assert_port
    cmp edi, PORT_MAX
    ja .Lconnect_local_assert_port_max
    mov r8d, edi
    mov eax, SYS_SOCKET
    mov edi, AF_INET
    mov esi, SOCK_STREAM
    xor edx, edx
    syscall
    test rax, rax
    js .Lconnect_local_return
    mov rbx, rax
    mov eax, r8d
    rol ax, 8
    mov word ptr [rip + address + 2], ax
    mov eax, SYS_CONNECT
    mov rdi, rbx
    lea rsi, [rip + address]
    mov edx, ADDRESS_LEN
    syscall
    test rax, rax
    js .Lconnect_local_failed
    mov rax, rbx
    jmp .Lconnect_local_return
.Lconnect_local_failed:
    mov r9, rax
    mov eax, SYS_CLOSE
    mov rdi, rbx
    syscall
    test rax, rax
    js .Lconnect_local_closed
.Lconnect_local_closed:
    mov rax, r9
.Lconnect_local_return:
    pop rbx
    ret
.Lconnect_local_assert_port:
    mov edi, 9
    call assert_fail
.Lconnect_local_assert_port_max:
    mov edi, 10
    call assert_fail
.size connect_local, . - connect_local

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

# Reads from file descriptor edi into the rdx bytes at rsi until they're full, the input ends,
# or, when ecx is a byte (0 to 255), the last byte read is ecx; with ecx = -1 it reads to the
# end. However the input arrives, it reads the same.
# In: edi = the file descriptor, rsi = the buffer, rdx = its size, at least 1, ecx = the end
# byte or -1.
# Out: rax = how many bytes it read, or a negative error code.
# returns: result
.type fill, @function
fill:
    test rsi, rsi
    jz .Lfill_assert_null
    test rdx, rdx
    jz .Lfill_assert_empty
    mov r8, rdx
    mov r10d, ecx
    mov r9, MAX_FILL_READS
.Lfill_loop:  # bound: 1099511627776
    mov eax, SYS_READ
    syscall
    test rax, rax
    js .Lfill_return
    jz .Lfill_done
    cmp rax, rdx
    ja .Lfill_assert_count
    add rsi, rax
    sub rdx, rax
    jz .Lfill_done
    movzx eax, byte ptr [rsi - 1]
    cmp eax, r10d
    je .Lfill_done
    dec r9
    jnz .Lfill_loop
    mov rax, ERROR_IO
    jmp .Lfill_return
.Lfill_done:
    mov rax, r8
    sub rax, rdx
.Lfill_return:
    ret
.Lfill_assert_null:
    mov edi, 990
    call assert_fail
.Lfill_assert_empty:
    mov edi, 991
    call assert_fail
.Lfill_assert_count:
    mov edi, 992
    call assert_fail
.size fill, . - fill

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

# Parses a decimal whole number with an optional leading minus sign.
# In: rdi = a NUL-terminated string.
# Out: rax = 0 and rdx = the number, or rax = ERROR_INVALID when the text isn't a number
# between -9223372036854775807 and 9223372036854775807.
# returns: result
.type parse_i64, @function
parse_i64:
    test rdi, rdi
    jz .Lparse_assert_null
    xor r9d, r9d
    cmp byte ptr [rdi], '-'
    jne .Lparse_digits
    mov r9d, 1
    inc rdi
.Lparse_digits:
    cmp byte ptr [rdi], 0
    je .Lparse_invalid
    xor eax, eax
    xor ecx, ecx
    mov r8, MAX_CHARS
.Lparse_loop:  # bound: 20
    cmp rcx, MAX_CHARS
    jae .Lparse_assert_index
    movzx edx, byte ptr [rdi + rcx]
    test edx, edx
    jz .Lparse_end
    sub edx, '0'
    cmp edx, 9
    ja .Lparse_invalid
    imul rax, rax, 10
    jo .Lparse_invalid
    add rax, rdx
    jo .Lparse_invalid
    inc rcx
    dec r8
    jnz .Lparse_loop
    jmp .Lparse_invalid
.Lparse_end:
    test r9d, r9d
    jz .Lparse_positive
    neg rax
.Lparse_positive:
    mov rdx, rax
    xor eax, eax
    jmp .Lparse_return
.Lparse_invalid:
    mov rax, ERROR_INVALID
.Lparse_return:
    ret
.Lparse_assert_null:
    mov edi, 13
    call assert_fail
.Lparse_assert_index:
    mov edi, 14
    call assert_fail
.size parse_i64, . - parse_i64

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
    mov edi, 19
    call assert_fail
.Lwrite_all_assert_empty:
    mov edi, 20
    call assert_fail
.Lwrite_all_assert_count:
    mov edi, 21
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
