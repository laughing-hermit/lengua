;; histogram: counts how many times each byte value occurs in standard input of any length and
;; prints the counts. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; standard input a 64 KiB buffer-full at a time, eight bytes a step, each byte loaded on its own
;; and counted in one of four tables, the first and fifth of a step in the first, the second and
;; sixth in the second, and so on, so that a run of one byte never waits on the counter it just
;; added to. The four tables, 8 KiB in all, stay in a core's first-level cache; main lends them to
;; the counting as four addresses, which stay in registers through the loop. A byte's counter is
;; found from the byte alone, 0 to 255, so it needs no check. The lines go out in one write.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; Two pages: lengua's first 1024 bytes, then the regions below, which end at 82944.
  (memory (export "memory") 2 2)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_READ_FAILED i32 (i32.const 2))
  (global $EXIT_WRITE_FAILED i32 (i32.const 3))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The buffer-fulls it takes at most: 2147483647 of 64 KiB, 128 TiB.
  (global $MAX_PIECES i32 (i32.const 2147483647))
  ;; A buffer-full's steps of eight bytes, and the bytes after them.
  (global $STEP i32 (i32.const 8))
  (global $STEPS i32 (i32.const 8192))
  (global $TAIL_MAX i32 (i32.const 7))
  ;; A table: a counter of 8 bytes for each of the 256 byte values, 2048 bytes. A value's counter
  ;; is the value moved left by SLOT_SHIFT.
  (global $VALUES i32 (i32.const 256))
  (global $SLOT_SHIFT i32 (i32.const 3))
  ;; Printing: a line is a value of up to 3 digits, a space, a count of up to 20 digits and a
  ;; newline, 25 bytes; a line starts at LAST_LINE or before.
  (global $LAST_LINE i32 (i32.const 8167))
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $TEN i64 (i64.const 10))
  (global $SPACE i32 (i32.const 32))
  (global $NEWLINE i32 (i32.const 10))
  (global $ZERO i32 (i32.const 48))

  ;; Regions (rule 3, M1), from 1024 up: a buffer-full of standard input, the four tables and the
  ;; lines it prints.
  (global $buffer i32 (i32.const 1024))  ;; shared: main fills it; count_piece and count_tail read it
  (global $buffer.size i32 (i32.const 65536))
  ;; The four tables of 256 counters, one after the other.
  (global $first i32 (i32.const 66560))  ;; shared: main lends it; count_tail and emit use it
  (global $first.size i32 (i32.const 2048))
  (global $second i32 (i32.const 68608))  ;; shared: main lends it; emit reads it
  (global $second.size i32 (i32.const 2048))
  (global $third i32 (i32.const 70656))  ;; shared: main lends it; emit reads it
  (global $third.size i32 (i32.const 2048))
  (global $fourth i32 (i32.const 72704))  ;; shared: main lends it; emit reads it
  (global $fourth.size i32 (i32.const 2048))
  ;; The lines it prints: 256 lines of 25 bytes at most, and room to spare.
  (global $out i32 (i32.const 74752))  ;; shared: put_number writes the numbers, emit the rest
  (global $out.size i32 (i32.const 8192))

  ;; Counts the byte values of standard input a buffer-full at a time, then prints them. Never
  ;; returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
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
            (call $count_piece (local.get $got) (global.get $first) (global.get $second)
              (global.get $third) (global.get $fourth))
            ;; A buffer-full that isn't full is the input's last.
            (br_if $counted (i32.lt_u (local.get $got) (global.get $buffer.size)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $piece (local.get $left)))
          ;; 128 TiB is all it takes.
          (call $assert_fail (i32.const 2))
          unreachable)
        (br_if $write_failed (call $emit))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Counts the first $len bytes of the buffer, eight a step, each byte loaded on its own: the
  ;; first and fifth of a step into the table at $a, the second and sixth into $b's, the third and
  ;; seventh into $c's, the fourth and eighth into $d's; count_tail counts the last bytes. A step
  ;; starts at $last or before, so all eight of its bytes are in the buffer.
  (func $count_piece (param $len i32) (param $a i32) (param $b i32) (param $c i32) (param $d i32)
    (local $i i32)
    (local $p i32)
    (local $at i32)
    (local $last i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (block $tail
      ;; Fewer than eight bytes: no step.
      (br_if $tail (i32.lt_u (local.get $len) (global.get $STEP)))
      (local.set $last (i32.sub (local.get $len) (global.get $STEP)))
      (local.set $left (global.get $STEPS))
      (loop $steps  ;; bound: 8192
        (br_if $tail (i32.gt_u (local.get $i) (local.get $last)))
        (local.set $p (i32.add (global.get $buffer) (local.get $i)))
        (i64.store
          (local.tee $at
            (i32.add (local.get $a)
              (i32.shl (i32.load8_u (local.get $p)) (global.get $SLOT_SHIFT))))
          (i64.add (i64.load (local.get $at)) (i64.const 1)))
        (i64.store
          (local.tee $at
            (i32.add (local.get $b)
              (i32.shl (i32.load8_u offset=1 (local.get $p)) (global.get $SLOT_SHIFT))))
          (i64.add (i64.load (local.get $at)) (i64.const 1)))
        (i64.store
          (local.tee $at
            (i32.add (local.get $c)
              (i32.shl (i32.load8_u offset=2 (local.get $p)) (global.get $SLOT_SHIFT))))
          (i64.add (i64.load (local.get $at)) (i64.const 1)))
        (i64.store
          (local.tee $at
            (i32.add (local.get $d)
              (i32.shl (i32.load8_u offset=3 (local.get $p)) (global.get $SLOT_SHIFT))))
          (i64.add (i64.load (local.get $at)) (i64.const 1)))
        (i64.store
          (local.tee $at
            (i32.add (local.get $a)
              (i32.shl (i32.load8_u offset=4 (local.get $p)) (global.get $SLOT_SHIFT))))
          (i64.add (i64.load (local.get $at)) (i64.const 1)))
        (i64.store
          (local.tee $at
            (i32.add (local.get $b)
              (i32.shl (i32.load8_u offset=5 (local.get $p)) (global.get $SLOT_SHIFT))))
          (i64.add (i64.load (local.get $at)) (i64.const 1)))
        (i64.store
          (local.tee $at
            (i32.add (local.get $c)
              (i32.shl (i32.load8_u offset=6 (local.get $p)) (global.get $SLOT_SHIFT))))
          (i64.add (i64.load (local.get $at)) (i64.const 1)))
        (i64.store
          (local.tee $at
            (i32.add (local.get $d)
              (i32.shl (i32.load8_u offset=7 (local.get $p)) (global.get $SLOT_SHIFT))))
          (i64.add (i64.load (local.get $at)) (i64.const 1)))
        (local.set $i (i32.add (local.get $i) (global.get $STEP)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $steps (local.get $left))))
    (call $count_tail (local.get $i) (local.get $len)))

  ;; Counts the bytes of the buffer from $from to $len, fewer than eight, into the first table.
  (func $count_tail (param $from i32) (param $len i32)
    (local $i i32)
    (local $at i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $len))
      (then (call $assert_fail (i32.const 5)) unreachable))
    (local.set $i (local.get $from))
    (block $done
      (local.set $left (global.get $TAIL_MAX))
      (loop $bytes  ;; bound: 7
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.ge_u (local.get $i) (global.get $buffer.size))
          (then (call $assert_fail (i32.const 6)) unreachable))
        (local.set $at
          (i32.add (global.get $first)
            (i32.shl (i32.load8_u (i32.add (global.get $buffer) (local.get $i)))
              (global.get $SLOT_SHIFT))))
        (i64.store (local.get $at) (i64.add (i64.load (local.get $at)) (i64.const 1)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $bytes (local.get $left)))))

  ;; Prints a line for each byte value that occurs, in the order of the values: the value, a space,
  ;; its count in all four tables and a newline, all of it in one write. Gives 0, or an error
  ;; number.
  (func $emit (result i32)  ;; returns: result
    (local $value i32)
    (local $slot i32)
    (local $to i32)
    (local $total i64)
    (local $left i32)
    (block $listed
      (local.set $left (global.get $VALUES))
      (loop $next  ;; bound: 256
        (if (i32.ge_u (local.get $value) (global.get $VALUES))
          (then (call $assert_fail (i32.const 7)) unreachable))
        (local.set $slot (i32.shl (local.get $value) (global.get $SLOT_SHIFT)))
        (local.set $total
          (i64.add
            (i64.add (i64.load (i32.add (global.get $first) (local.get $slot)))
              (i64.load (i32.add (global.get $second) (local.get $slot))))
            (i64.add (i64.load (i32.add (global.get $third) (local.get $slot)))
              (i64.load (i32.add (global.get $fourth) (local.get $slot))))))
        (if (i64.ne (local.get $total) (i64.const 0))
          (then
            ;; A line takes 25 bytes at most, which the lines have room for from here.
            (if (i32.gt_u (local.get $to) (global.get $LAST_LINE))
              (then (call $assert_fail (i32.const 8)) unreachable))
            (local.set $to (call $put_number (i64.extend_i32_u (local.get $value)) (local.get $to)))
            (if (i32.ge_u (local.get $to) (global.get $out.size))
              (then (call $assert_fail (i32.const 9)) unreachable))
            (i32.store8 (i32.add (global.get $out) (local.get $to)) (global.get $SPACE))
            (local.set $to
              (call $put_number (local.get $total) (i32.add (local.get $to) (i32.const 1))))
            (if (i32.ge_u (local.get $to) (global.get $out.size))
              (then (call $assert_fail (i32.const 10)) unreachable))
            (i32.store8 (i32.add (global.get $out) (local.get $to)) (global.get $NEWLINE))
            (local.set $to (i32.add (local.get $to) (i32.const 1)))))
        (local.set $value (i32.add (local.get $value) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $next (local.get $left))))
    (if (i32.gt_u (local.get $to) (global.get $out.size))
      (then (call $assert_fail (i32.const 11)) unreachable))
    (call $write_all (global.get $STDOUT) (global.get $out) (local.get $to)))

  ;; Writes $number's decimal digits into the lines from byte $at on, and gives where they end.
  (func $put_number (param $number i64) (param $at i32) (result i32)
    (local $digits i32)
    (local $rest i64)
    (local $to i32)
    (local $left i32)
    ;; How many digits: one, and one more for each time ten goes into what is left.
    (local.set $rest (local.get $number))
    (block $counted
      (local.set $left (global.get $MAX_DIGITS))
      (loop $count  ;; bound: 20
        (local.set $digits (i32.add (local.get $digits) (i32.const 1)))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $counted (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $count (local.get $left)))
      ;; A 64-bit number has at most 20 digits.
      (call $assert_fail (i32.const 12))
      unreachable)
    ;; The digits from the last.
    (local.set $to (i32.add (local.get $at) (local.get $digits)))
    (local.set $rest (local.get $number))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $to (i32.sub (local.get $to) (i32.const 1)))
        (if (i32.ge_u (local.get $to) (global.get $out.size))
          (then (call $assert_fail (i32.const 13)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $to))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      (call $assert_fail (i32.const 14))
      unreachable)
    (i32.add (local.get $at) (local.get $digits)))
)
