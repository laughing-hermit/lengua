.intel_syntax noprefix

# System calls (Linux x86-64)
.equ SYS_READ, 0
.equ SYS_CLOSE, 3
.equ SYS_POLL, 7
.equ SYS_SENDTO, 44
.equ SYS_ACCEPT4, 288
# File descriptors: lengua hands a server its listening socket as 3.
.equ LISTENING, 3
# poll's table: an entry of 8 bytes, the descriptor, then the events waited for and the events
# that came, 2 bytes each. A negative descriptor is a free slot, which poll passes over.
.equ POLLFD, 8
.equ EVENTS, 4
.equ REVENTS, 6
.equ POLLIN, 1
.equ FREE, -1
.equ WAIT_FOREVER, -1
# Socket flags: no SIGPIPE when a client hangs up early.
.equ MSG_NOSIGNAL, 0x4000
# Sizes and limits: the listening socket, then up to CLIENTS connections at once.
.equ CLIENTS, 8
.equ SLOTS, 9
.equ LINE_MAX, 1024
.equ LINES_LEN, 8192
.equ LENGTHS_LEN, 64
.equ TAKEN_LEN, 8
.equ NEWLINE, 10

.section .rodata

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .data

# What poll waits on: the listening socket, then each client's connection, FREE where no client
# is.
# shared: the loop polls it, take_connection fills its slots, serve_clients and serve_slot read
# them, and finish_slot frees them
.type pollfds, @object
pollfds:
    .long LISTENING
    .short POLLIN, 0
    .long FREE
    .short POLLIN, 0
    .long FREE
    .short POLLIN, 0
    .long FREE
    .short POLLIN, 0
    .long FREE
    .short POLLIN, 0
    .long FREE
    .short POLLIN, 0
    .long FREE
    .short POLLIN, 0
    .long FREE
    .short POLLIN, 0
    .long FREE
    .short POLLIN, 0
.size pollfds, . - pollfds

.section .bss

# Each client's line so far, LINE_MAX bytes a slot.
# shared: serve_slot reads into it, and finish_slot sends it back
.type lines, @object
lines:
    .zero LINES_LEN
.size lines, . - lines

# How many bytes of each client's line have come, 8 bytes a slot.
# shared: serve_slot counts them, and finish_slot sends that many and starts the slot over
.type lengths, @object
lengths:
    .zero LENGTHS_LEN
.size lengths, . - lengths

# How many slots hold a client.
# shared: take_connection counts each client it takes, and finish_slot each one it lets go
.type taken, @object
taken:
    .zero TAKEN_LEN
.size taken, . - taken

.text
.globl _start

# Waits on the listening socket and every client's connection for as long as it runs: takes
# each new connection, then serves each connection with something to read.
.type _start, @function
_start:
.Lstart_serve:  # bound: forever
    mov eax, SYS_POLL
    lea rdi, [rip + pollfds]
    mov esi, SLOTS
    mov edx, WAIT_FOREVER
    syscall
    test rax, rax
    js .Lstart_next
    cmp rax, SLOTS
    ja .Lstart_assert_ready
    call take_connection
    call serve_clients
.Lstart_next:
    jmp .Lstart_serve
.Lstart_assert_ready:
    mov edi, 1
    call assert_fail
.size _start, . - _start

# Takes the next connection, when the listening socket has one, into the first free slot. With
# every slot taken, it stops listening until finish_slot frees one, so further clients wait their
# turn in the kernel's queue.
.type take_connection, @function
take_connection:
    movzx eax, word ptr [rip + pollfds + REVENTS]
    test eax, POLLIN
    jz .Ltake_connection_return
    mov eax, SYS_ACCEPT4
    mov edi, LISTENING
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    syscall
    test rax, rax
    js .Ltake_connection_return
    cmp rax, LISTENING
    jbe .Ltake_connection_assert_descriptor
    lea rdi, [rip + pollfds]
    xor ecx, ecx
    mov r9, CLIENTS
.Ltake_connection_slot:  # bound: 8
    cmp rcx, CLIENTS
    jae .Ltake_connection_assert_slot
    cmp dword ptr [rdi + rcx*8 + POLLFD], FREE
    je .Ltake_connection_found
    inc rcx
    dec r9
    jnz .Ltake_connection_slot
    jmp .Ltake_connection_assert_room
.Ltake_connection_found:
    cmp rcx, CLIENTS
    jae .Ltake_connection_assert_found
    mov dword ptr [rdi + rcx*8 + POLLFD], eax
    mov word ptr [rdi + rcx*8 + POLLFD + REVENTS], 0
    inc qword ptr [rip + taken]
    cmp qword ptr [rip + taken], CLIENTS
    jb .Ltake_connection_return
    mov word ptr [rip + pollfds + EVENTS], 0
.Ltake_connection_return:
    ret
.Ltake_connection_assert_descriptor:
    mov edi, 2
    call assert_fail
.Ltake_connection_assert_slot:
    mov edi, 3
    call assert_fail
.Ltake_connection_assert_room:
    mov edi, 4
    call assert_fail
.Ltake_connection_assert_found:
    mov edi, 5
    call assert_fail
.size take_connection, . - take_connection

# Serves each client whose connection has something to read, or has closed.
.type serve_clients, @function
serve_clients:
    push rbx
    push r12
    sub rsp, 8
    xor r12d, r12d
    mov rbx, CLIENTS
.Lserve_clients_slot:  # bound: 8
    cmp r12, CLIENTS
    jae .Lserve_clients_assert_slot
    lea rdi, [rip + pollfds]
    movzx eax, word ptr [rdi + r12*8 + POLLFD + REVENTS]
    test eax, eax
    jz .Lserve_clients_next
    mov edi, r12d
    call serve_slot
.Lserve_clients_next:
    inc r12
    dec rbx
    jnz .Lserve_clients_slot
    add rsp, 8
    pop r12
    pop rbx
    ret
.Lserve_clients_assert_slot:
    mov edi, 6
    call assert_fail
.size serve_clients, . - serve_clients

# Reads what has come on a client's connection into its line. Once the line ends with a newline
# or holds LINE_MAX bytes, or the client has closed its side, it lets finish_slot answer.
# In: edi = the slot, 0 to CLIENTS - 1.
.type serve_slot, @function
serve_slot:
    push rbx
    push r12
    push r13
    mov r12d, edi
    cmp r12, CLIENTS
    jae .Lserve_slot_assert_slot
    lea rax, [rip + pollfds]
    mov ebx, dword ptr [rax + r12*8 + POLLFD]
    lea rax, [rip + lengths]
    mov r13, qword ptr [rax + r12*8]
    cmp r13, LINE_MAX
    jae .Lserve_slot_assert_length
    imul rsi, r12, LINE_MAX
    add rsi, r13
    lea rax, [rip + lines]
    add rsi, rax
    mov edx, LINE_MAX
    sub rdx, r13
    mov eax, SYS_READ
    mov edi, ebx
    syscall
    test rax, rax
    jle .Lserve_slot_finish
    cmp rax, rdx
    ja .Lserve_slot_assert_count
    add r13, rax
    lea rax, [rip + lengths]
    mov qword ptr [rax + r12*8], r13
    cmp r13, LINE_MAX
    je .Lserve_slot_finish
    imul rcx, r12, LINE_MAX
    add rcx, r13
    dec rcx
    cmp rcx, LINES_LEN
    jae .Lserve_slot_assert_index
    lea rax, [rip + lines]
    movzx eax, byte ptr [rax + rcx]
    cmp eax, NEWLINE
    jne .Lserve_slot_return
.Lserve_slot_finish:
    mov edi, r12d
    call finish_slot
.Lserve_slot_return:
    pop r13
    pop r12
    pop rbx
    ret
.Lserve_slot_assert_slot:
    mov edi, 7
    call assert_fail
.Lserve_slot_assert_length:
    mov edi, 8
    call assert_fail
.Lserve_slot_assert_count:
    mov edi, 9
    call assert_fail
.Lserve_slot_assert_index:
    mov edi, 10
    call assert_fail
.size serve_slot, . - serve_slot

# Sends a client's line back on its connection, closes it, and frees its slot for the next
# client, listening again.
# In: edi = the slot, 0 to CLIENTS - 1.
.type finish_slot, @function
finish_slot:
    push rbx
    push r12
    mov r12d, edi
    cmp r12, CLIENTS
    jae .Lfinish_slot_assert_slot
    lea rax, [rip + pollfds]
    mov ebx, dword ptr [rax + r12*8 + POLLFD]
    lea rax, [rip + lengths]
    mov rdx, qword ptr [rax + r12*8]
    cmp rdx, LINE_MAX
    ja .Lfinish_slot_assert_length
    test rdx, rdx
    jz .Lfinish_slot_close
    imul rsi, r12, LINE_MAX
    lea rcx, [rip + lines]
    add rsi, rcx
    mov eax, SYS_SENDTO
    mov edi, ebx
    mov r10d, MSG_NOSIGNAL
    xor r8d, r8d
    xor r9d, r9d
    syscall
    test rax, rax
    js .Lfinish_slot_close
.Lfinish_slot_close:
    mov eax, SYS_CLOSE
    mov edi, ebx
    syscall
    test rax, rax
    js .Lfinish_slot_free
.Lfinish_slot_free:
    cmp r12, CLIENTS
    jae .Lfinish_slot_assert_free
    lea rax, [rip + pollfds]
    mov dword ptr [rax + r12*8 + POLLFD], FREE
    mov word ptr [rax + r12*8 + POLLFD + REVENTS], 0
    lea rax, [rip + lengths]
    mov qword ptr [rax + r12*8], 0
    dec qword ptr [rip + taken]
    mov word ptr [rip + pollfds + EVENTS], POLLIN
    pop r12
    pop rbx
    ret
.Lfinish_slot_assert_slot:
    mov edi, 11
    call assert_fail
.Lfinish_slot_assert_length:
    mov edi, 12
    call assert_fail
.Lfinish_slot_assert_free:
    mov edi, 13
    call assert_fail
.size finish_slot, . - finish_slot

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
