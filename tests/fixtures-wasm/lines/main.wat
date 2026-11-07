;; lines-stream: counts the newlines of standard input of any length and prints the count. The
;; WebAssembly twin of tests/fixtures/lines, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): standard input a 4 KiB buffer-full at a time, as
;; stream-template.wat reads it, and each buffer-full's newlines counted 16 bytes at a time, as
;; vector-template.wat finds a byte: a vector compare, its bitmask, and the bitmask's bits.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; One page: lengua's first 1024 bytes, the buffer and the count's digits.
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
  ;; A buffer-full's vectors of 16 bytes, where the last of them starts, and the bytes after the
  ;; whole vectors of a buffer-full that isn't full.
  (global $CHUNK i32 (i32.const 16))
  (global $CHUNKS i32 (i32.const 256))
  (global $LAST_CHUNK i32 (i32.const 4080))
  (global $TAIL_MAX i32 (i32.const 15))
  ;; Printing the count: at most 19 digits, then a newline.
  (global $MAX_DIGITS i32 (i32.const 19))
  (global $NEWLINE_AT i32 (i32.const 19))
  (global $TEN i64 (i64.const 10))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))
  (global $ZERO i32 (i32.const 48))

  ;; Regions (rule 3, M1), from 1024 up: a buffer-full of standard input, then the count's digits
  ;; and its newline.
  (global $buffer i32 (i32.const 1024))  ;; shared: main fills it, and count_lines reads it
  (global $buffer.size i32 (i32.const 4096))
  (global $out i32 (i32.const 5120))
  (global $out.size i32 (i32.const 20))

  ;; Counts the newlines of standard input a buffer-full at a time, then prints the count. Never
  ;; returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $lines i64)
    (local $left i32)
    (block $write_failed
      (block $read_failed
        (block $counted
          (local.set $left (global.get $MAX_PIECES))
          (loop $piece  ;; bound: 2147483647
            (local.set $got
              (call $fill (global.get $STDIN) (global.get $buffer) (global.get $buffer.size)
                (global.get $READ_TO_END)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $buffer.size))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (local.set $lines
              (i64.add (local.get $lines)
                (i64.extend_i32_u (call $count_lines (local.get $got)))))
            ;; A buffer-full that isn't full is the input's last.
            (br_if $counted (i32.lt_u (local.get $got) (global.get $buffer.size)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $piece (local.get $left)))
          ;; 8 TiB is all it takes.
          (call $assert_fail (i32.const 2))
          unreachable)
        (br_if $write_failed (call $write_count (local.get $lines)))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Counts the newlines among the first $len bytes of the buffer: 16 bytes at a time while 16
  ;; are left, each a compare with a vector of newlines whose bitmask has a bit for each lane that
  ;; is one, then the rest one at a time, a scalar tail.
  (func $count_lines (param $len i32) (result i32)
    (local $i i32)
    (local $count i32)
    (local $left i32)
    (local $newlines v128)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 3)) unreachable))
    ;; The newline spread over every lane, once, before the loop.
    (local.set $newlines (i8x16.splat (global.get $NEWLINE)))
    (block $tail
      (local.set $left (global.get $CHUNKS))
      (loop $chunks  ;; bound: 256
        (br_if $tail (i32.gt_u (i32.add (local.get $i) (global.get $CHUNK)) (local.get $len)))
        ;; 16 bytes are left, so the vector starts at LAST_CHUNK or before.
        (if (i32.gt_u (local.get $i) (global.get $LAST_CHUNK))
          (then (call $assert_fail (i32.const 4)) unreachable))
        (local.set $count
          (i32.add (local.get $count)
            (i32.popcnt
              (i8x16.bitmask
                (i8x16.eq (v128.load (i32.add (global.get $buffer) (local.get $i)))
                  (local.get $newlines))))))
        (local.set $i (i32.add (local.get $i) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $chunks (local.get $left))))
    ;; The scalar tail: fewer than 16 bytes are left, one at a time.
    (block $done
      (local.set $left (global.get $TAIL_MAX))
      (loop $bytes  ;; bound: 15
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.ge_u (local.get $i) (global.get $buffer.size))
          (then (call $assert_fail (i32.const 5)) unreachable))
        (local.set $count
          (i32.add (local.get $count)
            (i32.eq (i32.load8_u (i32.add (global.get $buffer) (local.get $i)))
              (global.get $NEWLINE))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $bytes (local.get $left))))
    (local.get $count))

  ;; Writes $value, which is never negative, in decimal, then a newline, to standard output: the
  ;; newline last, and the digits before it from the last. Gives 0, or an error number.
  (func $write_count (param $value i64) (result i32)  ;; returns: result
    (local $rest i64)
    (local $at i32)
    (local $left i32)
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (local.set $rest (local.get $value))
    (local.set $at (global.get $NEWLINE_AT))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 19
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 7)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; A number below 2 to the 63rd has at most 19 digits.
      (call $assert_fail (i32.const 8))
      unreachable)
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
