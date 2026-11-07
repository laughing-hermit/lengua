;; echo: a server that sends each line back to the client that sent it. A fixture of the
;; WebAssembly prover, in the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; lengua's library gives $assert_fail, $fill and $write_all. Its line is the last bytes of the
;; memory.
(module
  (import "wasi_snapshot_preview1" "sock_accept"
    (func $sock_accept (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $LISTENER i32 (i32.const 4))
  ;; Flags.
  (global $NO_FLAGS i32 (i32.const 0))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))

  ;; Regions (rule 3, M1): where sock_accept writes the connection's descriptor, and the line,
  ;; at the memory's end.
  (global $accepted i32 (i32.const 1024))
  (global $accepted.size i32 (i32.const 4))
  (global $line i32 (i32.const 65520))
  (global $line.size i32 (i32.const 16))

  ;; Takes each connection, reads a line from it, sends the line back, and closes it.
  (func $main (export "_start")
    (local $err i32)
    (local $fd i32)
    (local $got i32)
    (loop $serve  ;; bound: forever
      (local.set $err
        (call $sock_accept (global.get $LISTENER) (global.get $NO_FLAGS) (global.get $accepted)))
      (if (local.get $err)
        (then (call $assert_fail (i32.const 1)) unreachable))
      (local.set $fd (i32.load (global.get $accepted)))
      (block $answered
        (local.set $got
          (call $fill (local.get $fd) (global.get $line) (global.get $line.size) (global.get $NEWLINE)))
        ;; A read that fails leaves nothing to send back.
        (br_if $answered (i32.lt_s (local.get $got) (i32.const 0)))
        (local.set $err (call $write_all (local.get $fd) (global.get $line) (local.get $got)))
        ;; A client that went away gets nothing more.
        (br_if $answered (local.get $err)))
      (local.set $err (call $fd_close (local.get $fd)))
      (if (local.get $err)
        (then (call $assert_fail (i32.const 2)) unreachable))
      (br $serve)))
)
