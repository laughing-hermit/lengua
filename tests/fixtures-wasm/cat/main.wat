;; cat: prints standard input back unchanged. The WebAssembly twin of tests/fixtures/cat, in the
;; WebAssembly dialect 1 (references/power-of-10-webassembly.md): lengua's library reads the
;; input with $fill, until the buffer is full or the input ends, and writes it with $write_all.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; One page: lengua's first 1024 bytes, then the buffer.
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_IO_FAILED i32 (i32.const 3))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The fewest bytes a write takes.
  (global $ONE_BYTE i32 (i32.const 1))

  ;; The one region (rule 3, M1): standard input as it was read, the 5000 bytes the idea allows.
  (global $buffer i32 (i32.const 1024))
  (global $buffer.size i32 (i32.const 5000))

  ;; Reads standard input and prints it back. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (block $failed
      (local.set $got
        (call $fill (global.get $STDIN) (global.get $buffer) (global.get $buffer.size)
          (global.get $READ_TO_END)))
      (br_if $failed (i32.lt_s (local.get $got) (i32.const 0)))
      ;; $fill reads at most the room it is given.
      (if (i32.gt_u (local.get $got) (global.get $buffer.size))
        (then (call $assert_fail (i32.const 1)) unreachable))
      ;; Empty input prints nothing.
      (if (local.get $got)
        (then
          ;; A write here takes at least one byte.
          (if (i32.lt_u (local.get $got) (global.get $ONE_BYTE))
            (then (call $assert_fail (i32.const 2)) unreachable))
          (br_if $failed
            (call $write_all (global.get $STDOUT) (global.get $buffer) (local.get $got)))))
      (call $proc_exit (global.get $EXIT_OK))
      unreachable)
    (call $proc_exit (global.get $EXIT_IO_FAILED))
    unreachable)
)
