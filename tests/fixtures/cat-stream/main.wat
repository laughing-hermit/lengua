;; cat-stream: prints standard input of any length back unchanged, a buffer-full at a time, in
;; constant memory. The WebAssembly twin of tests/fixtures/cat-stream, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md), in the shape of stream-template.wat: lengua's library
;; reads each buffer-full with $fill and writes it with $write_all.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; One page: lengua's first 1024 bytes, then the buffer.
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The buffer-fulls it takes at most: 2147483647 of 4 KiB, 8 TiB.
  (global $MAX_PIECES i32 (i32.const 2147483647))

  ;; The one region (rule 3, M1): a buffer-full of standard input, 4 KiB, which the tests' longer
  ;; inputs fill more than once.
  (global $buffer i32 (i32.const 1024))
  (global $buffer.size i32 (i32.const 4096))

  ;; Copies standard input to standard output a buffer-full at a time, until the input ends.
  ;; Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $left i32)
    (block $write_failed
      (block $read_failed
        (block $done
          (local.set $left (global.get $MAX_PIECES))
          (loop $piece  ;; bound: 2147483647
            (local.set $got
              (call $fill (global.get $STDIN) (global.get $buffer) (global.get $buffer.size)
                (global.get $READ_TO_END)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            ;; The input has ended.
            (br_if $done (i32.eqz (local.get $got)))
            (if (i32.gt_u (local.get $got) (global.get $buffer.size))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (br_if $write_failed
              (call $write_all (global.get $STDOUT) (global.get $buffer) (local.get $got)))
            ;; A buffer-full that isn't full is the input's last.
            (br_if $done (i32.lt_u (local.get $got) (global.get $buffer.size)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $piece (local.get $left)))
          ;; 8 TiB is all it takes.
          (call $assert_fail (i32.const 2))
          unreachable)
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)
)
