.intel_syntax noprefix

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
# Sizes and limits: the listening socket, then up to CLIENTS connections at once.
.equ CLIENTS, 8
.equ SLOTS, 9
.equ LINE_MAX, 1024
.equ LINES_LEN, 8192
.equ LENGTHS_LEN, 64
.equ TAKEN_LEN, 8
.equ NEWLINE, 10

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
# Waits on the listening socket and every client's connection for as long as it runs: takes
# each new connection, then serves each connection with something to read.
.type main, @function
main:
    sub rsp, 8
.Lmain_serve:  # bound: forever
    lea rdi, [rip + pollfds]
    mov esi, SLOTS
    mov edx, WAIT_FOREVER
    call os_poll
    test rax, rax
    js .Lmain_next
    cmp rax, SLOTS
    ja .Lmain_assert_ready
    call take_connection
    call serve_clients
.Lmain_next:
    jmp .Lmain_serve
.Lmain_assert_ready:
    mov edi, 1
    call assert_fail
.size main, . - main

# Takes the next connection, when the listening socket has one, into the first free slot. With
# every slot taken, it stops listening until finish_slot frees one, so further clients wait their
# turn in the kernel's queue.
.type take_connection, @function
take_connection:
    sub rsp, 8
    movzx eax, word ptr [rip + pollfds + REVENTS]
    test eax, POLLIN
    jz .Ltake_connection_return
    mov edi, LISTENING
    call os_accept
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
    add rsp, 8
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
    mov edi, ebx
    call os_read
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
    sub rsp, 8
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
    mov edi, ebx
    call os_write
    test rax, rax
    js .Lfinish_slot_close
.Lfinish_slot_close:
    mov edi, ebx
    call os_close
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
    add rsp, 8
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

