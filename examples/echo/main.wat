;; echo: a server that sends each line back to the client that sent it, one client at a time, in
;; the WebAssembly dialect 1 (references/power-of-10-webassembly.md), after the compile skill's
;; server-template.wat: one loop that never ends takes each connection with sock_accept on the
;; listener that lengua binds and hands over as descriptor 4, and lengua's library gives
;; $assert_fail, $fill and $write_all.
(module
  (import "wasi_snapshot_preview1" "sock_accept"
    (func $sock_accept (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors: lengua hands a server its listener as 4, and numbers each connection after it.
  (global $LISTENER i32 (i32.const 4))
  ;; sock_accept's flags: none, so a read waits for what the client sends.
  (global $NO_FLAGS i32 (i32.const 0))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))

  ;; Regions (rule 3, M1), from 1024 up: where sock_accept writes the connection's descriptor,
  ;; and the line, at most 1023 characters and a newline.
  (global $accepted i32 (i32.const 1024))
  (global $accepted.size i32 (i32.const 4))
  (global $line i32 (i32.const 2048))
  (global $line.size i32 (i32.const 1024))

  ;; Takes each connection, reads a line from it, sends the line back, and closes it, for as long
  ;; as it runs. A connection that can't be taken, read, answered or closed stops nothing: the
  ;; server goes on to the next one.
  (func $main (export "_start")
    (local $err i32)
    (local $fd i32)
    (local $got i32)
    (loop $serve  ;; bound: forever
      (block $next
        (local.set $err
          (call $sock_accept (global.get $LISTENER) (global.get $NO_FLAGS) (global.get $accepted)))
        ;; A connection that couldn't be taken leaves nothing to answer or close.
        (br_if $next (local.get $err))
        (local.set $fd (i32.load (global.get $accepted)))
        (if (i32.le_u (local.get $fd) (global.get $LISTENER))
          (then (call $assert_fail (i32.const 1)) unreachable))
        (block $answered
          ;; Until what came ends with a newline, the client closes its side, or the line is full.
          (local.set $got
            (call $fill (local.get $fd) (global.get $line) (global.get $line.size)
              (global.get $NEWLINE)))
          ;; A read that fails leaves nothing to send back.
          (br_if $answered (i32.lt_s (local.get $got) (i32.const 0)))
          ;; $fill reads at most the room it's lent. The bound of the count it gives first, which
          ;; makes it a number the rules follow.
          (if (i32.gt_u (local.get $got) (global.get $line.size))
            (then (call $assert_fail (i32.const 2)) unreachable))
          ;; Exactly what came, however many writes it takes.
          (local.set $err (call $write_all (local.get $fd) (global.get $line) (local.get $got)))
          ;; A client that went away gets nothing more.
          (br_if $answered (local.get $err)))
        (local.set $err (call $fd_close (local.get $fd)))
        ;; A connection that doesn't close is lengua's to drop: the next one is served all the
        ;; same.
        (br_if $next (local.get $err)))
      (br $serve)))
)
