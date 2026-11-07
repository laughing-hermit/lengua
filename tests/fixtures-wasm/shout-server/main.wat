;; shout-server: sends each line back to its client in capitals, for up to 64 clients at once, and
;; keeps each connection open for more lines. In the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): the server template's one loop that never ends, which
;; waits with poll_oneoff on every client's connection and, while a slot is free, on the listener
;; that lengua binds and hands over as descriptor 4. It takes each new connection with
;; sock_accept, reads what has come with one sock_recv an event, capitalizing each byte as it adds
;; it to the slot's line, and sends the line back once the data ends with a newline, in as many
;; sends as it takes; the connection stays open until the client closes its side.
(module
  (import "wasi_snapshot_preview1" "poll_oneoff"
    (func $poll_oneoff (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_accept"
    (func $sock_accept (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_recv"
    (func $sock_recv (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_send"
    (func $sock_send (param i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  ;; Two pages: lengua's first 1024 bytes, then the regions below, which end at 73728.
  (memory (export "memory") 2 2)

  ;; Descriptors: lengua hands a server its listener as 4, and numbers each connection after it.
  (global $LISTENER i32 (i32.const 4))
  ;; poll_oneoff's records: a subscription of 48 bytes, its own number first, its kind at 8 and
  ;; its descriptor at 16, and an event of 32 bytes, its subscription's number first. A wait to
  ;; read is kind 1.
  (global $SUBSCRIPTION i32 (i32.const 48))
  (global $EVENT i32 (i32.const 32))
  (global $FD_READ i32 (i32.const 1))
  ;; The socket calls' flags: none. Their buffers: one.
  (global $NO_FLAGS i32 (i32.const 0))
  (global $ONE_BUFFER i32 (i32.const 1))
  ;; Sizes and limits: up to CLIENTS connections at once, each line up to LINE_MAX bytes. A wait
  ;; is on each client's connection, and on the listener only while a slot is free, so it is on
  ;; at most CLIENTS descriptors.
  (global $CLIENTS i32 (i32.const 64))
  (global $LINE_MAX i32 (i32.const 1024))
  (global $WORD i32 (i32.const 4))
  (global $NEWLINE i32 (i32.const 10))
  ;; A slot with no client: 0, which no connection is.
  (global $FREE i32 (i32.const 0))
  ;; The listener's own number in a subscription, after every slot's.
  (global $LISTENING i32 (i32.const 64))
  ;; What answer does after it sends: keep the connection for the next line, or close it.
  (global $KEEP i32 (i32.const 0))
  (global $CLOSE i32 (i32.const 1))
  ;; Characters: a to z are the 26 bytes from 97, and each capital is 32 below its letter.
  (global $SMALL_A i32 (i32.const 97))
  (global $LETTERS i32 (i32.const 26))
  (global $CASE_BIT i32 (i32.const 32))

  ;; Regions (rule 3, M1), from 1024 up. What poll_oneoff waits on, and what it says happened.
  (global $subscriptions i32 (i32.const 1024))  ;; shared: subscribe writes it, main waits on it
  (global $subscriptions.size i32 (i32.const 3120))
  (global $events i32 (i32.const 4160))
  (global $events.size i32 (i32.const 2080))
  ;; How many events poll_oneoff wrote, and the descriptor sock_accept gave.
  (global $event_count i32 (i32.const 6272))
  (global $event_count.size i32 (i32.const 4))
  (global $accepted i32 (i32.const 6288))
  (global $accepted.size i32 (i32.const 4))
  ;; One buffer's address and length, for sock_recv and sock_send, and the counts they write.
  (global $iovec i32 (i32.const 6304))  ;; shared: receive and answer each give the host a buffer
  (global $iovec.size i32 (i32.const 8))
  (global $received i32 (i32.const 6320))
  (global $received.size i32 (i32.const 4))
  (global $received_flags i32 (i32.const 6328))
  (global $received_flags.size i32 (i32.const 2))
  (global $sent i32 (i32.const 6336))
  (global $sent.size i32 (i32.const 4))
  ;; Each slot's descriptor, FREE where no client is, and how many bytes of its line have come,
  ;; 4 bytes a slot.
  (global $fds i32 (i32.const 6400))  ;; shared: take fills a slot, answer frees it, all read it
  (global $fds.size i32 (i32.const 256))
  (global $lengths i32 (i32.const 6656))  ;; shared: receive counts, answer starts a slot over
  (global $lengths.size i32 (i32.const 256))
  ;; What one sock_recv brought, and each slot's line so far, LINE_MAX bytes a slot.
  (global $chunk i32 (i32.const 6912))  ;; shared: receive reads into it, append copies it out
  (global $chunk.size i32 (i32.const 1024))
  (global $lines i32 (i32.const 8192))  ;; shared: append and shift write lines, answer sends them
  (global $lines.size i32 (i32.const 65536))

  ;; Waits, for as long as it runs, on every client's connection and, while a slot is free, on
  ;; the listener; then takes each new connection, and serves each connection that has something
  ;; to read or whose client has closed its side.
  (func $main (export "_start")
    (local $count i32)
    (local $err i32)
    (local $happened i32)
    (local $e i32)
    (local $slot i32)
    (local $left i32)
    (loop $serve  ;; bound: forever
      (local.set $count (call $subscribe))
      (if (i32.gt_u (local.get $count) (global.get $CLIENTS))
        (then (call $assert_fail (i32.const 1)) unreachable))
      (block $served
        (local.set $err
          (call $poll_oneoff (global.get $subscriptions) (global.get $events) (local.get $count)
            (global.get $event_count)))
        ;; A wait that fails is made again.
        (br_if $served (local.get $err))
        (local.set $happened (i32.load (global.get $event_count)))
        ;; poll_oneoff writes at most one event a subscription.
        (if (i32.gt_u (local.get $happened) (local.get $count))
          (then (call $assert_fail (i32.const 2)) unreachable))
        (local.set $e (i32.const 0))
        (local.set $left (global.get $CLIENTS))
        (loop $event  ;; bound: 64
          (br_if $served (i32.ge_u (local.get $e) (local.get $happened)))
          (if (i32.ge_u (local.get $e) (global.get $CLIENTS))
            (then (call $assert_fail (i32.const 3)) unreachable))
          ;; An event starts with its subscription's own number: a slot, or LISTENING.
          (local.set $slot
            (i32.wrap_i64
              (i64.load
                (i32.add (global.get $events) (i32.mul (local.get $e) (global.get $EVENT))))))
          (if (i32.eq (local.get $slot) (global.get $LISTENING))
            (then (call $take))
            (else (call $receive (local.get $slot))))
          (local.set $e (i32.add (local.get $e) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $event (local.get $left))))
      (br $serve)))

  ;; Writes a wait to read on each client's connection, then on the listener while a slot is
  ;; free, each with its own number, the slot's or LISTENING, and gives how many it wrote. With
  ;; every slot taken, further clients wait their turn in the listener's queue.
  (func $subscribe (result i32)
    (local $slot i32)
    (local $count i32)
    (local $fd i32)
    (local $at i32)
    (local $left i32)
    (local.set $left (global.get $CLIENTS))
    (loop $slots  ;; bound: 64
      (if (i32.ge_u (local.get $slot) (global.get $CLIENTS))
        (then (call $assert_fail (i32.const 4)) unreachable))
      (local.set $fd
        (i32.load (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD)))))
      (if (i32.ne (local.get $fd) (global.get $FREE))
        (then
          (if (i32.ge_u (local.get $count) (global.get $CLIENTS))
            (then (call $assert_fail (i32.const 5)) unreachable))
          (local.set $at
            (i32.add (global.get $subscriptions)
              (i32.mul (local.get $count) (global.get $SUBSCRIPTION))))
          (i64.store (local.get $at) (i64.extend_i32_u (local.get $slot)))
          (i32.store8 offset=8 (local.get $at) (global.get $FD_READ))
          (i32.store offset=16 (local.get $at) (local.get $fd))
          (local.set $count (i32.add (local.get $count) (i32.const 1)))))
      (local.set $slot (i32.add (local.get $slot) (i32.const 1)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $slots (local.get $left)))
    (if (i32.lt_u (local.get $count) (global.get $CLIENTS))
      (then
        (local.set $at
          (i32.add (global.get $subscriptions)
            (i32.mul (local.get $count) (global.get $SUBSCRIPTION))))
        (i64.store (local.get $at) (i64.extend_i32_u (global.get $LISTENING)))
        (i32.store8 offset=8 (local.get $at) (global.get $FD_READ))
        (i32.store offset=16 (local.get $at) (global.get $LISTENER))
        (local.set $count (i32.add (local.get $count) (i32.const 1)))))
    (local.get $count))

  ;; Takes the next connection into the first free slot.
  (func $take
    (local $err i32)
    (local $fd i32)
    (local $slot i32)
    (local $left i32)
    (block $taken
      (local.set $err
        (call $sock_accept (global.get $LISTENER) (global.get $NO_FLAGS) (global.get $accepted)))
      ;; A client that left before its turn leaves nothing to take.
      (br_if $taken (local.get $err))
      (local.set $fd (i32.load (global.get $accepted)))
      (if (i32.le_u (local.get $fd) (global.get $LISTENER))
        (then (call $assert_fail (i32.const 6)) unreachable))
      (block $found
        (local.set $left (global.get $CLIENTS))
        (loop $free  ;; bound: 64
          (if (i32.ge_u (local.get $slot) (global.get $CLIENTS))
            (then (call $assert_fail (i32.const 7)) unreachable))
          (br_if $found
            (i32.eq
              (i32.load (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD))))
              (global.get $FREE)))
          (local.set $slot (i32.add (local.get $slot) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $free (local.get $left)))
        ;; main waits on the listener only while a slot is free.
        (call $assert_fail (i32.const 8))
        unreachable)
      (i32.store (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD)))
        (local.get $fd))))

  ;; Receives what has come on slot $slot's connection, at most what its line has room for, and
  ;; adds it to the line in capitals. Once the data ends with a newline or the line holds LINE_MAX
  ;; bytes, answer sends it back and keeps the connection; once the client has closed its side, or
  ;; the connection fails, answer sends back what remains and closes it.
  (func $receive (param $slot i32)
    (local $err i32)
    (local $fd i32)
    (local $len i32)
    (local $room i32)
    (local $got i32)
    (if (i32.ge_u (local.get $slot) (global.get $CLIENTS))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (local.set $fd
      (i32.load (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD)))))
    (local.set $len
      (i32.load (i32.add (global.get $lengths) (i32.mul (local.get $slot) (global.get $WORD)))))
    ;; A line is answered as soon as it is full, so a line so far has room.
    (if (i32.ge_u (local.get $len) (global.get $LINE_MAX))
      (then (call $assert_fail (i32.const 10)) unreachable))
    (local.set $room (i32.sub (global.get $LINE_MAX) (local.get $len)))
    (block $waits
      (block $ended
        (block $whole
          ;; One buffer: the chunk, as long as the room the line has left. The rules check where
          ;; a buffer starts apart from how long it is, so the chunk starts at its region's start.
          (i32.store (global.get $iovec) (global.get $chunk))
          (i32.store offset=4 (global.get $iovec) (local.get $room))
          (local.set $err
            (call $sock_recv (local.get $fd) (global.get $iovec) (global.get $ONE_BUFFER)
              (global.get $NO_FLAGS) (global.get $received) (global.get $received_flags)))
          ;; A connection that fails is answered with what came before, and closed.
          (br_if $ended (local.get $err))
          ;; The bound of a count read from memory first, which makes it a number the rules follow.
          (local.set $got (i32.load (global.get $received)))
          (if (i32.gt_u (local.get $got) (local.get $room))
            (then (call $assert_fail (i32.const 11)) unreachable))
          ;; The client has closed its side.
          (br_if $ended (i32.eqz (local.get $got)))
          (call $append (i32.add (i32.mul (local.get $slot) (global.get $LINE_MAX)) (local.get $len))
            (local.get $got))
          (local.set $len (i32.add (local.get $len) (local.get $got)))
          (i32.store (i32.add (global.get $lengths) (i32.mul (local.get $slot) (global.get $WORD)))
            (local.get $len))
          (br_if $whole (i32.eq (local.get $len) (global.get $LINE_MAX)))
          ;; What has come so far ends where the chunk does: a newline there ends the data.
          (br_if $waits
            (i32.ne
              (i32.load8_u (i32.add (global.get $chunk) (i32.sub (local.get $got) (i32.const 1))))
              (global.get $NEWLINE))))
        (call $answer (local.get $slot) (global.get $KEEP))
        (br $waits))
      (call $answer (local.get $slot) (global.get $CLOSE))))

  ;; Copies the $count bytes at the chunk's start to byte $at of the lines, one at a time, each
  ;; place checked, and each a to z in capitals: receive gives the place where the slot's line so
  ;; far ends. Below `a` wraps to a large number, so one unsigned test finds a to z, and the
  ;; test's 1 times the case bit is what comes off.
  (func $append (param $at i32) (param $count i32)
    (local $k i32)
    (local $to i32)
    (local $c i32)
    (local $left i32)
    (if (i32.gt_u (local.get $count) (global.get $LINE_MAX))
      (then (call $assert_fail (i32.const 12)) unreachable))
    (block $done
      (local.set $left (global.get $LINE_MAX))
      (loop $byte  ;; bound: 1024
        (br_if $done (i32.ge_u (local.get $k) (local.get $count)))
        (local.set $to (i32.add (local.get $at) (local.get $k)))
        (if (i32.ge_u (local.get $to) (global.get $lines.size))
          (then (call $assert_fail (i32.const 13)) unreachable))
        (local.set $c (i32.load8_u (i32.add (global.get $chunk) (local.get $k))))
        (i32.store8 (i32.add (global.get $lines) (local.get $to))
          (i32.sub (local.get $c)
            (i32.mul (global.get $CASE_BIT)
              (i32.lt_u (i32.sub (local.get $c) (global.get $SMALL_A)) (global.get $LETTERS)))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left)))))

  ;; Sends slot $slot's line back on its connection, in as many sends as it takes: a send takes
  ;; what goes at once, which may be only the first part of the line, and shift then moves the
  ;; rest to the line's start for the next send. Then, where $close is CLOSE or a send failed, it
  ;; closes the connection and frees the slot; else the slot's line starts over for the next.
  (func $answer (param $slot i32) (param $close i32)
    (local $err i32)
    (local $fd i32)
    (local $len i32)
    (local $went i32)
    (local $shut i32)
    (local $left i32)
    (if (i32.ge_u (local.get $slot) (global.get $CLIENTS))
      (then (call $assert_fail (i32.const 14)) unreachable))
    (if (i32.gt_u (local.get $close) (global.get $CLOSE))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (local.set $shut (local.get $close))
    (local.set $fd
      (i32.load (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD)))))
    (local.set $len
      (i32.load (i32.add (global.get $lengths) (i32.mul (local.get $slot) (global.get $WORD)))))
    (block $sent
      ;; Each send takes at least one of the LINE_MAX bytes a line holds at most.
      (local.set $left (global.get $LINE_MAX))
      (loop $send  ;; bound: 1024
        ;; What is left of the line, bounded each time round, as the loop changes it.
        (if (i32.gt_u (local.get $len) (global.get $LINE_MAX))
          (then (call $assert_fail (i32.const 16)) unreachable))
        (br_if $sent (i32.eqz (local.get $len)))
        ;; The buffer starts where the slot's line does, as the rules check where a buffer
        ;; starts apart from how long it is.
        (i32.store (global.get $iovec)
          (i32.add (global.get $lines) (i32.mul (local.get $slot) (global.get $LINE_MAX))))
        (i32.store offset=4 (global.get $iovec) (local.get $len))
        (local.set $err
          (call $sock_send (local.get $fd) (global.get $iovec) (global.get $ONE_BUFFER)
            (global.get $NO_FLAGS) (global.get $sent)))
        ;; A connection that fails takes nothing more: it is closed.
        (if (local.get $err)
          (then
            (local.set $shut (global.get $CLOSE))
            (br $sent)))
        ;; The bound of the count the send wrote first, as receive bounds its count.
        (local.set $went (i32.load (global.get $sent)))
        (if (i32.gt_u (local.get $went) (local.get $len))
          (then (call $assert_fail (i32.const 17)) unreachable))
        (br_if $sent (i32.eq (local.get $went) (local.get $len)))
        ;; A send that takes nothing would never end: the connection is closed.
        (if (i32.eqz (local.get $went))
          (then
            (local.set $shut (global.get $CLOSE))
            (br $sent)))
        (call $shift (local.get $slot) (local.get $went) (local.get $len))
        (local.set $len (i32.sub (local.get $len) (local.get $went)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $send (local.get $left))))
    (i32.store (i32.add (global.get $lengths) (i32.mul (local.get $slot) (global.get $WORD)))
      (i32.const 0))
    (if (local.get $shut)
      (then
        (block $closed
          (local.set $err (call $fd_close (local.get $fd)))
          ;; A connection that doesn't close is lengua's to drop: the slot is free all the same.
          (br_if $closed (local.get $err)))
        (i32.store (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD)))
          (global.get $FREE)))))

  ;; Moves the bytes from $from to $len of slot $slot's line to the line's start, one at a time,
  ;; each place checked: answer gives what a send left of the line.
  (func $shift (param $slot i32) (param $from i32) (param $len i32)
    (local $base i32)
    (local $k i32)
    (local $i i32)
    (local $src i32)
    (local $dst i32)
    (local $left i32)
    (if (i32.ge_u (local.get $slot) (global.get $CLIENTS))
      (then (call $assert_fail (i32.const 18)) unreachable))
    (if (i32.gt_u (local.get $len) (global.get $LINE_MAX))
      (then (call $assert_fail (i32.const 19)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $len))
      (then (call $assert_fail (i32.const 20)) unreachable))
    (local.set $base (i32.mul (local.get $slot) (global.get $LINE_MAX)))
    (block $done
      (local.set $left (global.get $LINE_MAX))
      (loop $byte  ;; bound: 1024
        ;; The place in the line of the next byte to move.
        (local.set $i (i32.add (local.get $from) (local.get $k)))
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (local.set $src (i32.add (local.get $base) (local.get $i)))
        (local.set $dst (i32.add (local.get $base) (local.get $k)))
        (if (i32.ge_u (local.get $src) (global.get $lines.size))
          (then (call $assert_fail (i32.const 21)) unreachable))
        (if (i32.ge_u (local.get $dst) (global.get $lines.size))
          (then (call $assert_fail (i32.const 22)) unreachable))
        (i32.store8 (i32.add (global.get $lines) (local.get $dst))
          (i32.load8_u (i32.add (global.get $lines) (local.get $src))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))))
)
