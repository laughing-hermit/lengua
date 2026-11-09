;; pairs-stream: counts the pairs of a letter in standard input of any length. The WebAssembly port
;; of the native pairs, in the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; lengua's library gives $fill and $write_all. It reads a buffer-full of 256 bytes at a time, as
;; stream-template.wat does, and what crosses from one buffer-full to the next is whether its last
;; byte is a letter that starts no pair yet.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
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
  ;; The buffer-fulls it takes at most: 2147483647 of 256 bytes, 512 GiB.
  (global $MAX_PIECES i32 (i32.const 2147483647))
  ;; The letter whose pairs it counts: `a`.
  (global $LETTER i32 (i32.const 97))
  ;; Printing: a count of up to 20 digits, then a newline.
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $NEWLINE_AT i32 (i32.const 20))
  (global $NEWLINE i32 (i32.const 10))
  (global $ZERO i32 (i32.const 48))
  (global $TEN i64 (i64.const 10))

  ;; Regions (rule 3, M1), from 1024 up: a buffer-full of standard input; what count_pairs says
  ;; of one, the pairs that end in it and whether its last byte is a letter that starts no pair
  ;; yet, 4 bytes each; and the line it prints.
  (global $buffer i32 (i32.const 1024))
  (global $buffer.size i32 (i32.const 256))
  (global $tally i32 (i32.const 1280))
  (global $tally.size i32 (i32.const 8))
  (global $out i32 (i32.const 1296))
  (global $out.size i32 (i32.const 21))

  ;; Counts the pairs of standard input a buffer-full at a time, until the input ends, then
  ;; prints the count. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $open i32)
    (local $pairs i64)
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
            (call $count_pairs (global.get $buffer) (local.get $got) (local.get $open)
              (global.get $tally))
            (local.set $pairs (i64.add (local.get $pairs) (i64.load32_u (global.get $tally))))
            (local.set $open (i32.load offset=4 (global.get $tally)))
            ;; A buffer-full that isn't full is the input's last.
            (br_if $done (i32.lt_u (local.get $got) (global.get $buffer.size)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $piece (local.get $left)))
          ;; 512 GiB is all it takes.
          (call $assert_fail (i32.const 2))
          unreachable)
        (br_if $write_failed (call $write_count (local.get $pairs)))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Counts the pairs of LETTER among the $len bytes at $at, which main lends it, each letter in
  ;; at most one pair, taken from the left: a letter right after one that starts no pair yet
  ;; ends a pair. $open is 1 when the byte before them is a letter that starts no pair yet, else
  ;; 0. Writes at $into, which main lends it, how many pairs end among the bytes, then 1 when the
  ;; last of them is a letter that starts no pair yet, else 0. Each byte's change is arithmetic,
  ;; not a branch.
  (func $count_pairs (param $at i32) (param $len i32) (param $open i32) (param $into i32)
    (local $i i32)
    (local $letter i32)
    (local $pairs i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (if (i32.gt_u (local.get $open) (i32.const 1))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (block $done
      (local.set $left (global.get $buffer.size))
      (loop $byte  ;; bound: 256
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.ge_u (local.get $i) (global.get $buffer.size))
          (then (call $assert_fail (i32.const 5)) unreachable))
        (local.set $letter
          (i32.eq (i32.load8_u (i32.add (local.get $at) (local.get $i))) (global.get $LETTER)))
        ;; A letter ends a pair where the one before starts one, and starts one where it doesn't.
        (local.set $pairs
          (i32.add (local.get $pairs) (i32.and (local.get $letter) (local.get $open))))
        (local.set $open (i32.and (local.get $letter) (i32.xor (local.get $open) (i32.const 1))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (i32.store (local.get $into) (local.get $pairs))
    (i32.store offset=4 (local.get $into) (local.get $open)))

  ;; Writes $count in decimal, then a newline, to standard output: the newline last, the digits
  ;; before it from the last. Gives 0, or an error number.
  (func $write_count (param $count i64) (result i32)  ;; returns: result
    (local $rest i64)
    (local $at i32)
    (local $left i32)
    (local.set $rest (local.get $count))
    (local.set $at (global.get $NEWLINE_AT))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 6)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; 20 digits are all a 64-bit count has.
      (call $assert_fail (i32.const 7))
      unreachable)
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
