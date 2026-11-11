;; sum: adds the whole numbers on the lines of standard input and prints the sum. It is made
;; from two parts of lengua's library, each function copied word for word under its
;; `;; from:` line, with the constants it reads: $lines_read and $lines_end from the part lines,
;; and $decimal_parse and $decimal_format from the part decimal. check holds each copy to its
;; part. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md).
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_NOT_A_NUMBER i32 (i32.const 2))
  (global $EXIT_TOO_LONG i32 (i32.const 3))
  (global $EXIT_READ_FAILED i32 (i32.const 4))
  (global $EXIT_WRITE_FAILED i32 (i32.const 5))
  ;; The lines a text of LINES_TEXT bytes has at most, and one more turn to see its end.
  (global $MAX_LINES i32 (i32.const 4097))
  ;; The numbers a line may hold.
  (global $MAX_NUMBER i64 (i64.const 1000000000))
  (global $MIN_NUMBER i64 (i64.const -1000000000))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))

  ;; The constants of the part lines, as that part defines them.
  (global $LINES_TEXT i32 (i32.const 4096))
  (global $LINES_ROOM i32 (i32.const 4097))
  (global $LINES_TO_END i32 (i32.const -1))
  (global $LINES_NEWLINE i32 (i32.const 10))

  ;; The constants of the part decimal, as that part defines them.
  (global $DECIMAL_OK i32 (i32.const 0))
  (global $DECIMAL_INVALID i32 (i32.const 28))
  (global $DECIMAL_TEXT i32 (i32.const 20))
  (global $DECIMAL_DIGITS i32 (i32.const 19))
  (global $DECIMAL_MAX i64 (i64.const 9223372036854775807))
  (global $DECIMAL_MAX_TENTH i64 (i64.const 922337203685477580))
  (global $DECIMAL_TEN i64 (i64.const 10))
  (global $DECIMAL_MINUS i32 (i32.const 45))
  (global $DECIMAL_ZERO i32 (i32.const 48))
  (global $DECIMAL_NINE i32 (i32.const 9))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then the text, the number read
  ;; and the output.
  (global $nan i32 (i32.const 1024))
  (global $nan.rosize i32 (i32.const 20))
  (data (i32.const 1024) "error: not a number\0a")
  (global $too_long i32 (i32.const 1056))
  (global $too_long.rosize i32 (i32.const 22))
  (data (i32.const 1056) "error: input too long\0a")
  ;; Standard input, which main lends $lines_read to write, and $lines_end and $decimal_parse
  ;; to read: LINES_ROOM bytes, and DECIMAL_TEXT bytes of room after the last place a line can
  ;; start, rounded up.
  (global $text i32 (i32.const 2048))
  (global $text.size i32 (i32.const 4160))
  ;; The number read, which main lends $decimal_parse to write.
  (global $number i32 (i32.const 6208))
  (global $number.size i32 (i32.const 8))
  ;; The output, which print_sum lends $decimal_format to write: a number's text, then a newline.
  (global $out i32 (i32.const 6224))
  (global $out.size i32 (i32.const 21))

  ;; Reads standard input, adds the number on each of its lines and prints the sum, or says why
  ;; it can't. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $from i32)
    (local $end i32)
    (local $length i32)
    (local $left i32)
    (local $err i32)
    (local $n i64)
    (local $sum i64)
    (block $write_failed
      (block $read_failed
        (block $too_long
          (block $not_a_number
            (local.set $got (call $lines_read (global.get $STDIN) (global.get $text)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $LINES_ROOM))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (br_if $too_long (i32.gt_u (local.get $got) (global.get $LINES_TEXT)))
            (block $done
              (local.set $left (global.get $MAX_LINES))
              (loop $line  ;; bound: 4097
                ;; No byte is left: the line before was the last.
                (br_if $done (i32.ge_u (local.get $from) (local.get $got)))
                ;; A line starts inside the text, so the DECIMAL_TEXT bytes $decimal_parse may
                ;; read from there are inside $text.
                (if (i32.ge_u (local.get $from) (global.get $LINES_TEXT))
                  (then (call $assert_fail (i32.const 2)) unreachable))
                (local.set $end
                  (call $lines_end (global.get $text) (local.get $from) (local.get $got)))
                (if (i32.gt_u (local.get $end) (global.get $LINES_TEXT))
                  (then (call $assert_fail (i32.const 3)) unreachable))
                (local.set $length (i32.sub (local.get $end) (local.get $from)))
                ;; A line longer than a number's text is no number.
                (br_if $not_a_number (i32.gt_u (local.get $length) (global.get $DECIMAL_TEXT)))
                (local.set $err
                  (call $decimal_parse (i32.add (global.get $text) (local.get $from))
                    (local.get $length) (global.get $number)))
                (br_if $not_a_number (local.get $err))
                (local.set $n (i64.load (global.get $number)))
                (br_if $not_a_number (i64.gt_s (local.get $n) (global.get $MAX_NUMBER)))
                (br_if $not_a_number (i64.lt_s (local.get $n) (global.get $MIN_NUMBER)))
                ;; At most MAX_LINES numbers of at most MAX_NUMBER: the sum fits in 64 bits.
                (local.set $sum (i64.add (local.get $sum) (local.get $n)))
                ;; The next line starts after this one's newline.
                (local.set $from (i32.add (local.get $end) (i32.const 1)))
                (local.set $left (i32.sub (local.get $left) (i32.const 1)))
                (br_if $line (local.get $left))))
            (br_if $write_failed (call $print_sum (local.get $sum)))
            (call $proc_exit (global.get $EXIT_OK))
            unreachable)
          (br_if $write_failed
            (call $write_all (global.get $STDERR) (global.get $nan) (global.get $nan.rosize)))
          (call $proc_exit (global.get $EXIT_NOT_A_NUMBER))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $too_long)
            (global.get $too_long.rosize)))
        (call $proc_exit (global.get $EXIT_TOO_LONG))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Writes $sum in decimal, then a newline, to standard output. Gives 0, or an error number.
  (func $print_sum (param $sum i64) (result i32)  ;; returns: result
    (local $length i32)
    (local.set $length (call $decimal_format (local.get $sum) (global.get $out)))
    (if (i32.gt_u (local.get $length) (global.get $DECIMAL_TEXT))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (i32.store8 (i32.add (global.get $out) (local.get $length)) (global.get $NEWLINE))
    (call $write_all (global.get $STDOUT) (global.get $out)
      (i32.add (local.get $length) (i32.const 1))))

  ;; from: ../lines
  (func $lines_read (param $fd i32) (param $text i32) (result i32)
    (local $got i32)
    (block $failed
      (local.set $got
        (call $fill (local.get $fd) (local.get $text) (global.get $LINES_ROOM)
          (global.get $LINES_TO_END)))
      ;; A negative count is an error number, which the caller gets as it is.
      (br_if $failed (i32.lt_s (local.get $got) (i32.const 0)))
      (if (i32.gt_u (local.get $got) (global.get $LINES_ROOM))
        (then (call $assert_fail (i32.const 824)) unreachable)))
    (local.get $got))

  ;; from: ../lines
  (func $lines_end (param $text i32) (param $from i32) (param $length i32) (result i32)
    (local $i i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $LINES_TEXT))
      (then (call $assert_fail (i32.const 821)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $length))
      (then (call $assert_fail (i32.const 822)) unreachable))
    (local.set $i (local.get $from))
    (block $found
      (local.set $left (global.get $LINES_TEXT))
      (loop $byte  ;; bound: 4096
        (br_if $found (i32.ge_u (local.get $i) (local.get $length)))
        (if (i32.ge_u (local.get $i) (global.get $LINES_TEXT))
          (then (call $assert_fail (i32.const 823)) unreachable))
        (br_if $found
          (i32.eq (i32.load8_u (i32.add (local.get $text) (local.get $i)))
            (global.get $LINES_NEWLINE)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $i))

  ;; from: ../decimal
  (func $decimal_parse (param $at i32) (param $length i32) (param $into i32) (result i32)  ;; returns: result
    (local $i i32)
    (local $c i32)
    (local $start i32)
    (local $left i32)
    (local $n i64)
    (local $status i32)
    (local.set $status (global.get $DECIMAL_INVALID))
    (block $invalid
      (br_if $invalid (i32.eqz (local.get $length)))
      (br_if $invalid (i32.gt_u (local.get $length) (global.get $DECIMAL_TEXT)))
      (if (i32.eq (i32.load8_u (local.get $at)) (global.get $DECIMAL_MINUS))
        (then (local.set $i (i32.add (local.get $i) (i32.const 1)))))
      ;; Where the digits start: 1 after a minus sign, else 0.
      (local.set $start (local.get $i))
      ;; A sign alone is no number, and a number has at most DECIMAL_DIGITS digits.
      (br_if $invalid (i32.ge_u (local.get $i) (local.get $length)))
      (br_if $invalid
        (i32.gt_u (i32.sub (local.get $length) (local.get $i)) (global.get $DECIMAL_DIGITS)))
      (block $end
        (local.set $left (global.get $DECIMAL_DIGITS))
        (loop $digit  ;; bound: 19
          (br_if $end (i32.ge_u (local.get $i) (local.get $length)))
          (if (i32.ge_u (local.get $i) (global.get $DECIMAL_TEXT))
            (then (call $assert_fail (i32.const 801)) unreachable))
          ;; Below '0' wraps to a large number, so one unsigned test catches both sides.
          (local.set $c
            (i32.sub (i32.load8_u (i32.add (local.get $at) (local.get $i)))
              (global.get $DECIMAL_ZERO)))
          (br_if $invalid (i32.gt_u (local.get $c) (global.get $DECIMAL_NINE)))
          (br_if $invalid (i64.gt_u (local.get $n) (global.get $DECIMAL_MAX_TENTH)))
          (local.set $n (i64.mul (local.get $n) (global.get $DECIMAL_TEN)))
          (br_if $invalid
            (i64.gt_u (local.get $n)
              (i64.sub (global.get $DECIMAL_MAX) (i64.extend_i32_u (local.get $c)))))
          (local.set $n (i64.add (local.get $n) (i64.extend_i32_u (local.get $c))))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $digit (local.get $left))))
      (if (local.get $start)
        (then (local.set $n (i64.sub (i64.const 0) (local.get $n)))))
      (i64.store (local.get $into) (local.get $n))
      (local.set $status (global.get $DECIMAL_OK)))
    (local.get $status))

  ;; from: ../decimal
  (func $decimal_format (param $value i64) (param $into i32) (result i32)
    (local $magnitude i64)
    (local $rest i64)
    (local $length i32)
    (local $at i32)
    (local $left i32)
    (local.set $magnitude (local.get $value))
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then
        (local.set $magnitude (i64.sub (i64.const 0) (local.get $value)))
        ;; The minus sign's byte.
        (local.set $length (i32.add (local.get $length) (i32.const 1)))))
    (if (i64.lt_s (local.get $magnitude) (i64.const 0))
      (then (call $assert_fail (i32.const 802)) unreachable))
    ;; How many digits: one, and one more for each time the magnitude divides by ten.
    (local.set $rest (local.get $magnitude))
    (block $counted
      (local.set $left (global.get $DECIMAL_DIGITS))
      (loop $count  ;; bound: 19
        (local.set $length (i32.add (local.get $length) (i32.const 1)))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $DECIMAL_TEN)))
        (br_if $counted (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $count (local.get $left)))
      ;; A magnitude below 2 to the 63rd has at most 19 digits.
      (call $assert_fail (i32.const 803))
      unreachable)
    ;; The digits, from the last to the first.
    (local.set $at (local.get $length))
    (block $written
      (local.set $left (global.get $DECIMAL_DIGITS))
      (loop $digit  ;; bound: 19
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $DECIMAL_TEXT))
          (then (call $assert_fail (i32.const 804)) unreachable))
        (i32.store8 (i32.add (local.get $into) (local.get $at))
          (i32.add (global.get $DECIMAL_ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $magnitude) (global.get $DECIMAL_TEN)))))
        (local.set $magnitude (i64.div_u (local.get $magnitude) (global.get $DECIMAL_TEN)))
        (br_if $written (i64.eqz (local.get $magnitude)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; The loop above counted these digits.
      (call $assert_fail (i32.const 805))
      unreachable)
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then (i32.store8 (local.get $into) (global.get $DECIMAL_MINUS))))
    (local.get $length))
)
