.arch armv8-a

// File descriptors: lengua hands a server its listening socket as 3.
.equ LISTENING, 3
// os_poll's table: an entry of 8 bytes, the descriptor, then the events waited for and the events
// that came, 2 bytes each. A negative descriptor is a free slot, which os_poll passes over.
.equ POLLFD, 8
.equ EVENTS, 4
.equ REVENTS, 6
.equ POLLIN, 1
.equ FREE, -1
.equ WAIT_FOREVER, -1
// Sizes and limits: the listening socket, then up to CLIENTS connections at once.
.equ CLIENTS, 8
.equ SLOTS, 9
.equ LINE_MAX, 1024
.equ LINES_LEN, 8192
.equ LENGTHS_LEN, 64
.equ TAKEN_LEN, 8
.equ NEWLINE, 10

.section .data

// What os_poll waits on: the listening socket, then each client's connection, FREE where no
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
// Waits on the listening socket and every client's connection for as long as it runs: takes
// each new connection, then serves each connection with something to read. os_poll with no time
// limit (WAIT_FOREVER) waits for as long as it takes.
.type main, @function
main:
.Lmain_serve:  // bound: forever
    adrp x0, pollfds
    add x0, x0, :lo12:pollfds
    mov x1, #SLOTS
    mov x2, #WAIT_FOREVER
    bl os_poll
    tbnz x0, #63, .Lmain_next
    cmp x0, #SLOTS
    b.hi .Lmain_assert_ready
    bl take_connection
    bl serve_clients
.Lmain_next:
    b .Lmain_serve
.Lmain_assert_ready:
    mov w0, #1
    bl assert_fail
.size main, . - main

// Takes the next connection, when the listening socket has one, into the first free slot. With
// every slot taken, it stops listening until finish_slot frees one, so further clients wait their
// turn in the kernel's queue.
.type take_connection, @function
take_connection:
    stp x29, x30, [sp, #-16]!
    adrp x9, pollfds
    add x9, x9, :lo12:pollfds
    ldrh w10, [x9, #REVENTS]
    tst w10, #POLLIN
    b.eq .Ltake_connection_return
    mov x0, #LISTENING
    bl os_accept
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
    ldp x29, x30, [sp], #16
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
    bl os_read
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
    stp x29, x30, [sp, #-16]!
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
    bl os_write
    tbnz x0, #63, .Lfinish_slot_close
.Lfinish_slot_close:
    mov w0, w11
    bl os_close
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
    ldp x29, x30, [sp], #16
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

