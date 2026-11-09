;; words-stream: counts the lines and words of standard input of any length, as `wc -lw` does. The
;; WebAssembly port of the native words, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $fill and $write_all. It reads
;; a buffer-full of 4096 bytes at a time, as stream-template.wat does, and looks at each byte in
;; turn, where the native twin takes 16 at a time with SSE2; what crosses from one buffer-full to
;; the next is whether its last byte is whitespace.
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
  ;; The buffer-fulls it takes at most: 2147483647 of 4 KiB, 8 TiB.
  (global $MAX_PIECES i32 (i32.const 2147483647))
  ;; Before the first byte it is as if whitespace came, so a byte that isn't starts a word.
  (global $BLANK i32 (i32.const 1))
  ;; Bytes that matter. Tab to carriage return are the 5 bytes from 9 to 13.
  (global $NEWLINE i32 (i32.const 10))
  (global $SPACE i32 (i32.const 32))
  (global $TAB i32 (i32.const 9))
  (global $TAB_TO_CR i32 (i32.const 4))
  (global $ZERO i32 (i32.const 48))
  ;; Printing: two counts of up to 20 digits each, a space between them and a newline after.
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $NEWLINE_AT i32 (i32.const 41))
  (global $TEN i64 (i64.const 10))

  ;; Regions (rule 3, M1), from 1024 up: a buffer-full of standard input; what count_piece says
  ;; of one, its newlines, the words that start in it and whether its last byte is whitespace, 4
  ;; bytes each; and the line it prints.
  (global $buffer i32 (i32.const 1024))
  (global $buffer.size i32 (i32.const 4096))
  (global $tally i32 (i32.const 5120))
  (global $tally.size i32 (i32.const 12))
  (global $out i32 (i32.const 5136))
  (global $out.size i32 (i32.const 42))

  ;; Counts the lines and words of standard input a buffer-full at a time, until the input ends,
  ;; then prints both. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $blank i32)
    (local $lines i64)
    (local $words i64)
    (local $left i32)
    (local.set $blank (global.get $BLANK))
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
            (call $count_piece (global.get $buffer) (local.get $got) (local.get $blank)
              (global.get $tally))
            (local.set $lines (i64.add (local.get $lines) (i64.load32_u (global.get $tally))))
            (local.set $words
              (i64.add (local.get $words) (i64.load32_u offset=4 (global.get $tally))))
            (local.set $blank (i32.load offset=8 (global.get $tally)))
            ;; A buffer-full that isn't full is the input's last.
            (br_if $done (i32.lt_u (local.get $got) (global.get $buffer.size)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $piece (local.get $left)))
          ;; 8 TiB is all it takes.
          (call $assert_fail (i32.const 2))
          unreachable)
        (br_if $write_failed (call $write_counts (local.get $lines) (local.get $words)))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Counts the newlines among the $len bytes at $at, which main lends it, and the words that
  ;; start there: a byte that isn't whitespace after one that is. $blank is 1 when the byte
  ;; before them is whitespace, or there is none, else 0. Writes at $into, which main lends it,
  ;; the newlines, the words and whether the last byte is whitespace. Each byte's change is
  ;; arithmetic, not a branch.
  (func $count_piece (param $at i32) (param $len i32) (param $blank i32) (param $into i32)
    (local $i i32)
    (local $c i32)
    (local $space i32)
    (local $lines i32)
    (local $words i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (if (i32.gt_u (local.get $blank) (i32.const 1))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (block $done
      (local.set $left (global.get $buffer.size))
      (loop $byte  ;; bound: 4096
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.ge_u (local.get $i) (global.get $buffer.size))
          (then (call $assert_fail (i32.const 5)) unreachable))
        (local.set $c (i32.load8_u (i32.add (local.get $at) (local.get $i))))
        (local.set $lines
          (i32.add (local.get $lines) (i32.eq (local.get $c) (global.get $NEWLINE))))
        ;; A space, or tab to carriage return: below a tab wraps to a large number, so one
        ;; unsigned test finds those five.
        (local.set $space
          (i32.or (i32.eq (local.get $c) (global.get $SPACE))
            (i32.le_u (i32.sub (local.get $c) (global.get $TAB)) (global.get $TAB_TO_CR))))
        ;; A word starts where a byte that isn't whitespace comes after one that is.
        (local.set $words
          (i32.add (local.get $words)
            (i32.and (local.get $blank) (i32.xor (local.get $space) (i32.const 1)))))
        (local.set $blank (local.get $space))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (i32.store (local.get $into) (local.get $lines))
    (i32.store offset=4 (local.get $into) (local.get $words))
    (i32.store offset=8 (local.get $into) (local.get $blank)))

  ;; Writes $lines, a space, $words and a newline to standard output, each count in decimal.
  ;; Gives 0, or an error number.
  (func $write_counts (param $lines i64) (param $words i64) (result i32)  ;; returns: result
    (local $at i32)
    (i32.store8 (i32.add (global.get $out) (global.get $NEWLINE_AT)) (global.get $NEWLINE))
    (local.set $at (call $digits (global.get $out) (local.get $words) (global.get $NEWLINE_AT)))
    ;; The space before the words.
    (local.set $at (i32.sub (local.get $at) (i32.const 1)))
    (if (i32.ge_u (local.get $at) (global.get $out.size))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $SPACE))
    (local.set $at (call $digits (global.get $out) (local.get $lines) (local.get $at)))
    (if (i32.ge_u (local.get $at) (global.get $out.size))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))

  ;; Writes $number's decimal digits into $into, which write_counts lends it with room for
  ;; NEWLINE_AT bytes, the last digit just before byte $end, and gives where the first one is.
  (func $digits (param $into i32) (param $number i64) (param $end i32) (result i32)
    (local $at i32)
    (local $rest i64)
    (local $left i32)
    (if (i32.gt_u (local.get $end) (global.get $NEWLINE_AT))
      (then (call $assert_fail (i32.const 8)) unreachable))
    (local.set $at (local.get $end))
    (local.set $rest (local.get $number))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $NEWLINE_AT))
          (then (call $assert_fail (i32.const 9)) unreachable))
        (i32.store8 (i32.add (local.get $into) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; 20 digits are all a 64-bit number has.
      (call $assert_fail (i32.const 10))
      unreachable)
    (local.get $at))
)
