;; histogram-split: counts how many times each byte value occurs in standard input of any length
;; and prints the counts, with the work split into pieces that run at once. In the WebAssembly
;; dialect 1 (references/power-of-10-webassembly.md): standard input a buffer-full of 8 MiB at a
;; time, each split into eight pieces. A piece is one more run of this module, with its share of
;; the bytes as its standard input and a memory of its own: it counts them, eight bytes a step,
;; each byte loaded on its own and counted in one of four tables, and prints its counts as the
;; program prints the input's. The program adds what the eight pieces printed to its first
;; table. `piece` says which of the two a run is, and both read in the one loop.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (import "lengua_split_1" "split" (func $split (param i32 i32 i32 i32 i32) (result i32)))
  (import "lengua_split_1" "piece" (func $piece (result i32)))
  ;; 130 pages: lengua's first 1024 bytes, then the regions below, which end at 8471552.
  (memory (export "memory") 130 130)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_READ_FAILED i32 (i32.const 2))
  (global $EXIT_WRITE_FAILED i32 (i32.const 3))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; What `piece` answers in the run that is the whole program: NOENT.
  (global $NOT_A_PIECE i32 (i32.const 44))
  ;; The pieces a buffer-full is split into, as the idea's line grants.
  (global $PIECES i32 (i32.const 8))
  ;; The buffer-fulls it takes at most: 2147483647 of 8 MiB.
  (global $MAX_FULLS i32 (i32.const 2147483647))
  ;; A buffer-full's steps of eight bytes, and the bytes after them.
  (global $STEP i32 (i32.const 8))
  (global $STEPS i32 (i32.const 1048576))
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
  ;; A piece's answer, read back: where the last answer starts among the answers, a value's
  ;; digits with the space after them, a count's digits with the newline after them, and ten.
  (global $LAST_ANSWER i32 (i32.const 57344))
  (global $VALUE_BYTES i32 (i32.const 4))
  (global $COUNT_BYTES i32 (i32.const 21))
  (global $TEN_32 i32 (i32.const 10))

  ;; Regions (rule 3, M1), from 1024 up: a buffer-full of standard input, the four tables, the
  ;; lines it prints, and the pieces' answers.
  (global $buffer i32 (i32.const 1024))  ;; shared: main fills it; count_piece and count_tail read it
  (global $buffer.size i32 (i32.const 8388608))
  ;; The four tables of 256 counters, one after the other.
  (global $first i32 (i32.const 8389632))  ;; shared: main lends it; count_tail, add_answer and emit use it
  (global $first.size i32 (i32.const 2048))
  (global $second i32 (i32.const 8391680))  ;; shared: main lends it; emit reads it
  (global $second.size i32 (i32.const 2048))
  (global $third i32 (i32.const 8393728))  ;; shared: main lends it; emit reads it
  (global $third.size i32 (i32.const 2048))
  (global $fourth i32 (i32.const 8395776))  ;; shared: main lends it; emit reads it
  (global $fourth.size i32 (i32.const 2048))
  ;; The lines it prints: 256 lines of 25 bytes at most, and room to spare.
  (global $out i32 (i32.const 8397824))  ;; shared: put_number writes the numbers, emit the rest
  (global $out.size i32 (i32.const 8192))
  ;; What the eight pieces of a buffer-full printed, each at the start of its 8192 bytes, with
  ;; zeros after it.
  (global $answers i32 (i32.const 8406016))  ;; shared: split_full has them written; answer_byte reads them
  (global $answers.size i32 (i32.const 65536))

  ;; Asks whether this run is a piece, then counts the byte values of standard input a
  ;; buffer-full at a time, a piece by itself and the whole program through its pieces, and
  ;; prints them. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $left i32)
    (local $whole i32)
    ;; 0 in a piece of the work, NOT_A_PIECE in the whole program, and nothing else.
    (local.set $whole (call $piece))
    (if (local.get $whole)
      (then
        (if (i32.ne (local.get $whole) (global.get $NOT_A_PIECE))
          (then (call $assert_fail (i32.const 15)) unreachable))))
    (block $write_failed
      (block $read_failed
        (block $counted
          (local.set $left (global.get $MAX_FULLS))
          (loop $full  ;; bound: 2147483647
            (local.set $got
              (call $fill (global.get $STDIN) (global.get $buffer) (global.get $buffer.size)
                (global.get $READ_TO_END)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $buffer.size))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (if (local.get $whole)
              (then (call $split_full (local.get $got)))
              (else
                (call $count_piece (local.get $got) (global.get $first) (global.get $second)
                  (global.get $third) (global.get $fourth))))
            ;; A buffer-full that isn't full is the input's last.
            (br_if $counted (i32.lt_u (local.get $got) (global.get $buffer.size)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $full (local.get $left)))
          ;; 16 PiB is all it takes.
          (call $assert_fail (i32.const 2))
          unreachable)
        (br_if $write_failed (call $emit))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Has the first $len bytes of the buffer counted by PIECES pieces, each a run of this module
  ;; on its share of the bytes, at once where the computer has the cores, then adds what each
  ;; counted to the first table. No bytes are no work.
  (func $split_full (param $len i32)
    (local $err i32)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 16)) unreachable))
    (block $none
      (br_if $none (i32.eqz (local.get $len)))
      (local.set $err
        (call $split (global.get $buffer) (local.get $len) (global.get $PIECES)
          (global.get $answers) (global.get $answers.size)))
      ;; The idea's line covers the call, and a piece that fails ends the run, so it never fails.
      (if (local.get $err)
        (then (call $assert_fail (i32.const 17)) unreachable))
      (call $add_answers)))

  ;; Adds what each of the PIECES pieces printed, one after the other in the pieces' order.
  (func $add_answers
    (local $index i32)
    (local $left i32)
    (local.set $left (global.get $PIECES))
    (loop $next  ;; bound: 8
      (if (i32.ge_u (local.get $index) (global.get $PIECES))
        (then (call $assert_fail (i32.const 18)) unreachable))
      (call $add_answer (i32.mul (local.get $index) (global.get $out.size)))
      (local.set $index (i32.add (local.get $index) (i32.const 1)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $next (local.get $left))))

  ;; The byte of the answers at $at.
  (func $answer_byte (param $at i32) (result i32)
    (if (i32.ge_u (local.get $at) (global.get $answers.size))
      (then (call $assert_fail (i32.const 19)) unreachable))
    (i32.load8_u (i32.add (global.get $answers) (local.get $at))))

  ;; Adds the answer that starts at byte $from of the answers to the first table: its lines, as
  ;; emit prints them, each a value, a space, a count and a newline, until a zero where a line
  ;; would start, or the answer's end.
  (func $add_answer (param $from i32)
    (local $i i32)
    (local $end i32)
    (local $byte i32)
    (local $value i32)
    (local $slot i32)
    (local $count i64)
    (local $lines i32)
    (local $left i32)
    (if (i32.gt_u (local.get $from) (global.get $LAST_ANSWER))
      (then (call $assert_fail (i32.const 20)) unreachable))
    (local.set $i (local.get $from))
    (local.set $end (i32.add (local.get $from) (global.get $out.size)))
    (block $added
      (local.set $lines (global.get $VALUES))
      (loop $line  ;; bound: 256
        (br_if $added (i32.ge_u (local.get $i) (local.get $end)))
        (br_if $added (i32.eqz (call $answer_byte (local.get $i))))
        ;; The value: digits, then a space.
        (local.set $value (i32.const 0))
        (block $valued
          (local.set $left (global.get $VALUE_BYTES))
          (loop $value_digit  ;; bound: 4
            (local.set $byte (call $answer_byte (local.get $i)))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br_if $valued (i32.eq (local.get $byte) (global.get $SPACE)))
            (local.set $value
              (i32.add (i32.mul (local.get $value) (global.get $TEN_32))
                (i32.sub (local.get $byte) (global.get $ZERO))))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $value_digit (local.get $left)))
          ;; A value has 3 digits at most.
          (call $assert_fail (i32.const 21))
          unreachable)
        ;; The count: digits, then a newline.
        (local.set $count (i64.const 0))
        (block $numbered
          (local.set $left (global.get $COUNT_BYTES))
          (loop $count_digit  ;; bound: 21
            (local.set $byte (call $answer_byte (local.get $i)))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br_if $numbered (i32.eq (local.get $byte) (global.get $NEWLINE)))
            (local.set $count
              (i64.add (i64.mul (local.get $count) (global.get $TEN))
                (i64.extend_i32_u (i32.sub (local.get $byte) (global.get $ZERO)))))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $count_digit (local.get $left)))
          ;; A count has 20 digits at most.
          (call $assert_fail (i32.const 22))
          unreachable)
        (if (i32.ge_u (local.get $value) (global.get $VALUES))
          (then (call $assert_fail (i32.const 23)) unreachable))
        (local.set $slot (i32.shl (local.get $value) (global.get $SLOT_SHIFT)))
        (i64.store (i32.add (global.get $first) (local.get $slot))
          (i64.add (i64.load (i32.add (global.get $first) (local.get $slot)))
            (local.get $count)))
        (local.set $lines (i32.sub (local.get $lines) (i32.const 1)))
        (br_if $line (local.get $lines)))))

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
      (loop $steps  ;; bound: 1048576
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
