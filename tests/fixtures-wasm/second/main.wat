;; second: reads two lines from standard input and prints the second line. The WebAssembly twin of
;; tests/fixtures/second, in the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; lengua's library gives $fill and $write_all. The native twin is wrong by design: its one fill
;; stops at the first read that ends with a newline, so it reads both lines only when they arrive
;; in one read. This one reads to the end of the input, however the input arrives, and passes
;; every test; $NEWLINE in place of $READ_TO_END in its call of $fill is the native twin's bug.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_FAILED i32 (i32.const 1))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))

  ;; Regions (rule 3, M1), from 1024 up: the input as it came, 64 bytes for two lines of at most
  ;; 21 bytes each, and the second line, copied to a start of its own to print.
  (global $input i32 (i32.const 1024))
  (global $input.size i32 (i32.const 64))
  (global $line i32 (i32.const 1088))
  (global $line.size i32 (i32.const 64))

  ;; Reads the input to its end, finds where the second line starts and ends, and prints it.
  ;; Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $start i32)
    (local $end i32)
    (local $len i32)
    (block $failed
      ;; Every byte until the input ends, in as many reads as the input arrives in.
      (local.set $got
        (call $fill (global.get $STDIN) (global.get $input) (global.get $input.size)
          (global.get $READ_TO_END)))
      (br_if $failed (i32.lt_s (local.get $got) (i32.const 0)))
      (if (i32.gt_u (local.get $got) (global.get $input.size))
        (then (call $assert_fail (i32.const 1)) unreachable))
      ;; The second line starts after the first line's newline and ends after its own.
      (local.set $start (call $line_end (global.get $input) (i32.const 0) (local.get $got)))
      (local.set $end (call $line_end (global.get $input) (local.get $start) (local.get $got)))
      (local.set $len
        (call $copy_line (global.get $input) (global.get $line) (local.get $start)
          (local.get $end)))
      (if (i32.gt_u (local.get $len) (global.get $line.size))
        (then (call $assert_fail (i32.const 2)) unreachable))
      (br_if $failed (call $write_all (global.get $STDOUT) (global.get $line) (local.get $len)))
      (call $proc_exit (global.get $EXIT_OK))
      unreachable)
    (call $proc_exit (global.get $EXIT_FAILED))
    unreachable)

  ;; Gives the place just past the first newline among the bytes at $at, which main lends it
  ;; with room for 64, from byte $from up to byte $got; or $got, when none of them is a newline.
  (func $line_end (param $at i32) (param $from i32) (param $got i32) (result i32)
    (local $i i32)
    (local $end i32)
    (local $left i32)
    (if (i32.gt_u (local.get $got) (global.get $input.size))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (local.set $end (local.get $got))
    (local.set $i (local.get $from))
    (block $found
      (local.set $left (global.get $input.size))
      (loop $byte  ;; bound: 64
        (br_if $found (i32.ge_u (local.get $i) (local.get $got)))
        (if (i32.ge_u (local.get $i) (global.get $input.size))
          (then (call $assert_fail (i32.const 4)) unreachable))
        (if (i32.eq (i32.load8_u (i32.add (local.get $at) (local.get $i))) (global.get $NEWLINE))
          (then
            (local.set $end (i32.add (local.get $i) (i32.const 1)))
            (br $found)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $end))

  ;; Copies the bytes from $start to $end of $src, which main lends it with room for 64, to the
  ;; start of $dst, which main lends it with room for 64 too, one at a time, each place checked,
  ;; and gives how many it copied.
  (func $copy_line (param $src i32) (param $dst i32) (param $start i32) (param $end i32)
    (result i32)
    (local $k i32)
    (local $i i32)
    (local $left i32)
    (if (i32.gt_u (local.get $end) (global.get $input.size))
      (then (call $assert_fail (i32.const 5)) unreachable))
    (block $copied
      (local.set $left (global.get $line.size))
      (loop $byte  ;; bound: 64
        (local.set $i (i32.add (local.get $start) (local.get $k)))
        (br_if $copied (i32.ge_u (local.get $i) (local.get $end)))
        (if (i32.ge_u (local.get $i) (global.get $input.size))
          (then (call $assert_fail (i32.const 6)) unreachable))
        (if (i32.ge_u (local.get $k) (global.get $line.size))
          (then (call $assert_fail (i32.const 7)) unreachable))
        (i32.store8 (i32.add (local.get $dst) (local.get $k))
          (i32.load8_u (i32.add (local.get $src) (local.get $i))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $k))
)
