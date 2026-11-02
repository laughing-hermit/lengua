.arch armv8-a

// System calls (Linux AArch64)
.equ SYS_READ, 63
.equ SYS_CLOSE, 57
.equ SYS_PPOLL, 73
.equ SYS_SENDTO, 206
.equ SYS_ACCEPT4, 242
// File descriptors: lengua hands a server its listening socket as 3.
.equ LISTENING, 3
// ppoll's table: an entry of 8 bytes, the descriptor, then the events waited for and the events
// that came, 2 bytes each. A negative descriptor is a free slot, which ppoll passes over.
.equ POLLFD, 8
.equ EVENTS, 4
.equ REVENTS, 6
.equ POLLIN, 1
.equ FREE, -1
// Socket flags: no SIGPIPE when a client hangs up early.
.equ MSG_NOSIGNAL, 0x4000
// Sizes and limits: the listening socket, then up to CLIENTS connections at once.
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

// What ppoll waits on: the listening socket, then each client's connection, FREE where no
// client is.
// shared: the loop polls it, take_connection fills its slots, serve_clients and serve_slot read
// them, and finish_slot frees them
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

// Each client's line so far, LINE_MAX bytes a slot.
// shared: serve_slot reads into it, and finish_slot sends it back
.type lines, @object
lines:
    .zero LINES_LEN
.size lines, . - lines

// How many bytes of each client's line have come, 8 bytes a slot.
// shared: serve_slot counts them, and finish_slot sends that many and starts the slot over
.type lengths, @object
lengths:
    .zero LENGTHS_LEN
.size lengths, . - lengths

// How many slots hold a client.
// shared: take_connection counts each client it takes, and finish_slot each one it lets go
.type taken, @object
taken:
    .zero TAKEN_LEN
.size taken, . - taken

.text
.globl _start

// Waits on the listening socket and every client's connection for as long as it runs: takes
// each new connection, then serves each connection with something to read. ppoll with no time
// limit (x2 = 0) and no signal mask (x3 = 0) is x86-64's poll that waits forever.
.type _start, @function
_start:
.Lstart_serve:  // bound: forever
    adrp x0, pollfds
    add x0, x0, :lo12:pollfds
    mov x1, #SLOTS
    mov x2, #0
    mov x3, #0
    mov x8, #SYS_PPOLL
    svc #0
    tbnz x0, #63, .Lstart_next
    cmp x0, #SLOTS
    b.hi .Lstart_assert_ready
    bl take_connection
    bl serve_clients
.Lstart_next:
    b .Lstart_serve
.Lstart_assert_ready:
    mov w0, #1
    bl assert_fail
.size _start, . - _start

// Takes the next connection, when the listening socket has one, into the first free slot. With
// every slot taken, it stops listening until finish_slot frees one, so further clients wait their
// turn in the kernel's queue.
.type take_connection, @function
take_connection:
    adrp x9, pollfds
    add x9, x9, :lo12:pollfds
    ldrh w10, [x9, #REVENTS]
    tst w10, #POLLIN
    b.eq .Ltake_connection_return
    mov x0, #LISTENING
    mov x1, #0
    mov x2, #0
    mov x3, #0
    mov x8, #SYS_ACCEPT4
    svc #0
    tbnz x0, #63, .Ltake_connection_return
    cmp x0, #LISTENING
    b.ls .Ltake_connection_assert_descriptor
    mov x11, #0
    mov x12, #CLIENTS
.Ltake_connection_slot:  // bound: 8
    cmp x11, #CLIENTS
    b.hs .Ltake_connection_assert_slot
    add x13, x9, x11, lsl #3
    ldr w14, [x13, #POLLFD]
    cmn w14, #1
    b.eq .Ltake_connection_found
    add x11, x11, #1
    subs x12, x12, #1
    b.ne .Ltake_connection_slot
    b .Ltake_connection_assert_room
.Ltake_connection_found:
    cmp x11, #CLIENTS
    b.hs .Ltake_connection_assert_found
    add x13, x9, x11, lsl #3
    str w0, [x13, #POLLFD]
    strh wzr, [x13, #POLLFD + REVENTS]
    adrp x14, taken
    add x14, x14, :lo12:taken
    ldr x15, [x14]
    add x15, x15, #1
    str x15, [x14]
    cmp x15, #CLIENTS
    b.lo .Ltake_connection_return
    strh wzr, [x9, #EVENTS]
.Ltake_connection_return:
    ret
.Ltake_connection_assert_descriptor:
    mov w0, #2
    bl assert_fail
.Ltake_connection_assert_slot:
    mov w0, #3
    bl assert_fail
.Ltake_connection_assert_room:
    mov w0, #4
    bl assert_fail
.Ltake_connection_assert_found:
    mov w0, #5
    bl assert_fail
.size take_connection, . - take_connection

// Serves each client whose connection has something to read, or has closed.
.type serve_clients, @function
serve_clients:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    mov x19, #0
    mov x20, #CLIENTS
.Lserve_clients_slot:  // bound: 8
    cmp x19, #CLIENTS
    b.hs .Lserve_clients_assert_slot
    adrp x9, pollfds
    add x9, x9, :lo12:pollfds
    add x9, x9, x19, lsl #3
    ldrh w10, [x9, #POLLFD + REVENTS]
    cbz w10, .Lserve_clients_next
    mov w0, w19
    bl serve_slot
.Lserve_clients_next:
    add x19, x19, #1
    subs x20, x20, #1
    b.ne .Lserve_clients_slot
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lserve_clients_assert_slot:
    mov w0, #6
    bl assert_fail
.size serve_clients, . - serve_clients

// Reads what has come on a client's connection into its line. Once the line ends with a newline
// or holds LINE_MAX bytes, or the client has closed its side, it lets finish_slot answer.
// In: w0 = the slot, 0 to CLIENTS - 1.
.type serve_slot, @function
serve_slot:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    mov w19, w0
    cmp x19, #CLIENTS
    b.hs .Lserve_slot_assert_slot
    adrp x9, pollfds
    add x9, x9, :lo12:pollfds
    add x9, x9, x19, lsl #3
    ldr w20, [x9, #POLLFD]
    adrp x10, lengths
    add x10, x10, :lo12:lengths
    ldr x21, [x10, x19, lsl #3]
    cmp x21, #LINE_MAX
    b.hs .Lserve_slot_assert_length
    mov x11, #LINE_MAX
    madd x1, x19, x11, x21
    adrp x12, lines
    add x12, x12, :lo12:lines
    add x1, x12, x1
    sub x2, x11, x21
    mov w0, w20
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lserve_slot_finish
    cbz x0, .Lserve_slot_finish
    cmp x0, x2
    b.hi .Lserve_slot_assert_count
    add x21, x21, x0
    str x21, [x10, x19, lsl #3]
    cmp x21, #LINE_MAX
    b.eq .Lserve_slot_finish
    madd x13, x19, x11, x21
    sub x13, x13, #1
    cmp x13, #LINES_LEN
    b.hs .Lserve_slot_assert_index
    ldrb w14, [x12, x13]
    cmp w14, #NEWLINE
    b.ne .Lserve_slot_return
.Lserve_slot_finish:
    mov w0, w19
    bl finish_slot
.Lserve_slot_return:
    ldp x21, x22, [sp], #16
    ldp x19, x20, [sp], #16
    ldp x29, x30, [sp], #16
    ret
.Lserve_slot_assert_slot:
    mov w0, #7
    bl assert_fail
.Lserve_slot_assert_length:
    mov w0, #8
    bl assert_fail
.Lserve_slot_assert_count:
    mov w0, #9
    bl assert_fail
.Lserve_slot_assert_index:
    mov w0, #10
    bl assert_fail
.size serve_slot, . - serve_slot

// Sends a client's line back on its connection, closes it, and frees its slot for the next
// client, listening again.
// In: w0 = the slot, 0 to CLIENTS - 1.
.type finish_slot, @function
finish_slot:
    mov w9, w0
    cmp x9, #CLIENTS
    b.hs .Lfinish_slot_assert_slot
    adrp x10, pollfds
    add x10, x10, :lo12:pollfds
    add x10, x10, x9, lsl #3
    ldr w11, [x10, #POLLFD]
    adrp x12, lengths
    add x12, x12, :lo12:lengths
    ldr x2, [x12, x9, lsl #3]
    cmp x2, #LINE_MAX
    b.hi .Lfinish_slot_assert_length
    cbz x2, .Lfinish_slot_close
    mov x13, #LINE_MAX
    mul x1, x9, x13
    adrp x14, lines
    add x14, x14, :lo12:lines
    add x1, x14, x1
    mov w0, w11
    mov x3, #MSG_NOSIGNAL
    mov x4, #0
    mov x5, #0
    mov x8, #SYS_SENDTO
    svc #0
    tbnz x0, #63, .Lfinish_slot_close
.Lfinish_slot_close:
    mov w0, w11
    mov x8, #SYS_CLOSE
    svc #0
    tbnz x0, #63, .Lfinish_slot_free
.Lfinish_slot_free:
    cmp x9, #CLIENTS
    b.hs .Lfinish_slot_assert_free
    mov w13, #FREE
    str w13, [x10, #POLLFD]
    strh wzr, [x10, #POLLFD + REVENTS]
    str xzr, [x12, x9, lsl #3]
    adrp x14, taken
    add x14, x14, :lo12:taken
    ldr x15, [x14]
    sub x15, x15, #1
    str x15, [x14]
    adrp x13, pollfds
    add x13, x13, :lo12:pollfds
    mov w15, #POLLIN
    strh w15, [x13, #EVENTS]
    ret
.Lfinish_slot_assert_slot:
    mov w0, #11
    bl assert_fail
.Lfinish_slot_assert_length:
    mov w0, #12
    bl assert_fail
.Lfinish_slot_assert_free:
    mov w0, #13
    bl assert_fail
.size finish_slot, . - finish_slot

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
