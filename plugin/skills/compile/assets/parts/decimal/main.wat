;; decimal: a part of lengua's library. Its idea's contract offers two functions to the programs
;; that build on it: $decimal_parse reads a whole number from decimal text, and $decimal_format
;; writes one. Each works only through the addresses it is lent and reads only the $DECIMAL_
;; constants below, so it means in another program what it means here. The part's own program
;; prints the negative of the whole number in its argument, so its tests and its proofs go
;; through both functions. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md).
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_NOT_A_NUMBER i32 (i32.const 2))
  (global $EXIT_WRITE_FAILED i32 (i32.const 3))
  ;; Sizes and limits: the program's name and the number; the most text the arguments may have;
  ;; and the bytes $text_length looks at, one more than a number's text has.
  (global $ARGC_EXPECTED i32 (i32.const 2))
  (global $MAX_ARGS_TEXT i32 (i32.const 256))
  (global $SCAN i32 (i32.const 21))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))

  ;; The part's constants, which its two functions read. A program that takes a function
  ;; defines each constant the function reads as it is here.
  ;; What $decimal_parse gives.
  (global $DECIMAL_OK i32 (i32.const 0))
  (global $DECIMAL_INVALID i32 (i32.const 28))
  ;; The most bytes a number's text has, a minus sign and 19 digits, and the most digits.
  (global $DECIMAL_TEXT i32 (i32.const 20))
  (global $DECIMAL_DIGITS i32 (i32.const 19))
  ;; The largest magnitude, and the largest that still takes one more digit.
  (global $DECIMAL_MAX i64 (i64.const 9223372036854775807))
  (global $DECIMAL_MAX_TENTH i64 (i64.const 922337203685477580))
  (global $DECIMAL_TEN i64 (i64.const 10))
  ;; Characters: the minus sign, the digit 0, and how far the digit 9 is from it.
  (global $DECIMAL_MINUS i32 (i32.const 45))
  (global $DECIMAL_ZERO i32 (i32.const 48))
  (global $DECIMAL_NINE i32 (i32.const 9))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then what the host writes,
  ;; the number read and the output.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 19))
  (data (i32.const 1024) "usage: decimal <n>\0a")
  (global $nan i32 (i32.const 1056))
  (global $nan.rosize i32 (i32.const 20))
  (data (i32.const 1056) "error: not a number\0a")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 2048))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then the number.
  (global $argv i32 (i32.const 2064))
  (global $argv.size i32 (i32.const 8))
  ;; The arguments' text, each ended by a NUL: at most MAX_ARGS_TEXT bytes, and SCAN bytes of
  ;; room after the last place a number can start, so main can lend any of those places.
  (global $args i32 (i32.const 2080))
  (global $args.size i32 (i32.const 276))
  ;; The number read, which main lends $decimal_parse to write.
  (global $number i32 (i32.const 2368))
  (global $number.size i32 (i32.const 8))
  ;; The output, which main lends $decimal_format to write: a number's text, then a newline.
  (global $out i32 (i32.const 2384))
  (global $out.size i32 (i32.const 21))

  ;; Reads the argument, parses it and prints its negative, or says why it can't. Never
  ;; returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $size i32)
    (local $at i32)
    (local $length i32)
    (local $n i64)
    (block $write_failed
      (block $not_a_number
        (block $usage
          (local.set $err
            (call $args_sizes_get (global.get $sizes) (i32.add (global.get $sizes) (i32.const 4))))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 1)) unreachable))
          (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
          ;; Text longer than MAX_ARGS_TEXT isn't a number.
          (local.set $size (i32.load offset=4 (global.get $sizes)))
          (br_if $not_a_number (i32.gt_u (local.get $size) (global.get $MAX_ARGS_TEXT)))
          (local.set $err (call $args_get (global.get $argv) (global.get $args)))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 2)) unreachable))
          ;; The number starts where argv[1] points, inside the text args_get wrote.
          (local.set $at (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args)))
          (if (i32.ge_u (local.get $at) (global.get $MAX_ARGS_TEXT))
            (then (call $assert_fail (i32.const 3)) unreachable))
          (local.set $length (call $text_length (i32.add (global.get $args) (local.get $at))))
          ;; More bytes than a number's text has.
          (br_if $not_a_number (i32.gt_u (local.get $length) (global.get $DECIMAL_TEXT)))
          (local.set $err
            (call $decimal_parse (i32.add (global.get $args) (local.get $at)) (local.get $length)
              (global.get $number)))
          (br_if $not_a_number (local.get $err))
          ;; $decimal_parse never gives the most negative number, so its negative fits.
          (local.set $n (i64.load (global.get $number)))
          (local.set $length
            (call $decimal_format (i64.sub (i64.const 0) (local.get $n)) (global.get $out)))
          (if (i32.gt_u (local.get $length) (global.get $DECIMAL_TEXT))
            (then (call $assert_fail (i32.const 4)) unreachable))
          (i32.store8 (i32.add (global.get $out) (local.get $length)) (global.get $NEWLINE))
          (br_if $write_failed
            (call $write_all (global.get $STDOUT) (global.get $out)
              (i32.add (local.get $length) (i32.const 1))))
          (call $proc_exit (global.get $EXIT_OK))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
        (call $proc_exit (global.get $EXIT_USAGE))
        unreachable)
      (br_if $write_failed
        (call $write_all (global.get $STDERR) (global.get $nan) (global.get $nan.rosize)))
      (call $proc_exit (global.get $EXIT_NOT_A_NUMBER))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Gives how many bytes of the text at $text come before its NUL, or SCAN when its first SCAN
  ;; bytes hold none. main lends $text with room for SCAN bytes.
  (func $text_length (param $text i32) (result i32)
    (local $i i32)
    (local $left i32)
    (block $ended
      (local.set $left (global.get $SCAN))
      (loop $byte  ;; bound: 21
        (if (i32.ge_u (local.get $i) (global.get $SCAN))
          (then (call $assert_fail (i32.const 5)) unreachable))
        (br_if $ended (i32.eqz (i32.load8_u (i32.add (local.get $text) (local.get $i)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $i))

  ;; Reads the whole number that the first $length bytes at $at write in decimal: an optional
  ;; minus sign, then 1 to DECIMAL_DIGITS digits, and nothing else. Writes it at $into, 8 bytes,
  ;; and gives DECIMAL_OK; or gives DECIMAL_INVALID, and writes nothing, when the text isn't a
  ;; number from -9223372036854775807 to 9223372036854775807. Its caller lends $at with room for
  ;; DECIMAL_TEXT bytes, whatever $length is.
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

  ;; Writes $value in decimal at $into: a minus sign when it is negative, then its digits, with
  ;; no zero at the start but for 0 itself; at most DECIMAL_TEXT bytes. Gives how many bytes it
  ;; wrote. $value is never the most negative number, which has no magnitude in 64 bits.
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
