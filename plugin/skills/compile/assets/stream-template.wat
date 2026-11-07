;; shout-stream: copies standard input of any length to standard output with every ASCII letter
;; in capitals, a buffer-full at a time, in constant memory. The WebAssembly counterpart of
;; stream-template.s, in the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; lengua's library reads each buffer-full with $fill and writes it with $write_all. The
;; transform here takes a byte at a time; vector-template.wat takes 16 at a time.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; Two pages: lengua's first 1024 bytes, then the buffer.
  (memory (export "memory") 2 2)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The buffer-fulls it takes at most: 2147483647 of 64 KiB, 128 TiB.
  (global $MAX_PIECES i32 (i32.const 2147483647))
  ;; Characters: a to z are the 26 bytes from 97, and each capital is 32 below its letter.
  (global $SMALL_A i32 (i32.const 97))
  (global $LETTERS i32 (i32.const 26))
  (global $CASE_BIT i32 (i32.const 32))

  ;; The one region (rule 3, M1): a buffer-full of standard input, capitalized in place, then
  ;; written. 64 KiB is sixteen of the pieces a pipe hands over, so each write sends many.
  (global $buffer i32 (i32.const 1024))
  (global $buffer.size i32 (i32.const 65536))

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
            (call $shout (global.get $buffer) (local.get $got))
            (br_if $write_failed
              (call $write_all (global.get $STDOUT) (global.get $buffer) (local.get $got)))
            ;; A buffer-full that isn't full is the input's last.
            (br_if $done (i32.lt_u (local.get $got) (global.get $buffer.size)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $piece (local.get $left)))
          ;; 128 TiB is all it takes.
          (call $assert_fail (i32.const 2))
          unreachable)
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Capitalizes the $len bytes at $at, which main lends it: a to z become A to Z, and the other
  ;; bytes stay as they are. Each byte's change is arithmetic, not a branch.
  (func $shout (param $at i32) (param $len i32)
    (local $i i32)
    (local $c i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (block $done
      (local.set $left (global.get $buffer.size))
      (loop $byte  ;; bound: 65536
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.ge_u (local.get $i) (global.get $buffer.size))
          (then (call $assert_fail (i32.const 4)) unreachable))
        (local.set $c (i32.load8_u (i32.add (local.get $at) (local.get $i))))
        ;; Below 'a' wraps to a large number, so one unsigned test finds a to z, and the test's 1
        ;; times the case bit is what comes off.
        (i32.store8 (i32.add (local.get $at) (local.get $i))
          (i32.sub (local.get $c)
            (i32.mul (global.get $CASE_BIT)
              (i32.lt_u (i32.sub (local.get $c) (global.get $SMALL_A)) (global.get $LETTERS)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))))
