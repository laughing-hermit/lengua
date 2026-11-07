;; absdiff: prints the absolute difference of the two whole numbers in its arguments. The
;; WebAssembly counterpart of examples/absdiff, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $assert_fail and $write_all. A
;; number's digits past its limit are only checked, so a number of any length is out of range,
;; never a number that overflows.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; Three pages: lengua's first 1024 bytes, the messages and the small regions, then the
  ;; arguments' text from 4096.
  (memory (export "memory") 3 3)

  ;; Descriptors.
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_NOT_A_NUMBER i32 (i32.const 2))
  (global $EXIT_OUT_OF_RANGE i32 (i32.const 3))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  ;; What the functions marked "returns: result" give.
  (global $NO_ERROR i32 (i32.const 0))
  (global $ERROR_INVALID i32 (i32.const 28))
  (global $ERROR_RANGE i32 (i32.const 68))
  ;; Sizes and limits.
  (global $ARGC_EXPECTED i32 (i32.const 3))
  (global $NUM_LIMIT i64 (i64.const 1000000))
  (global $MAX_DIGITS i32 (i32.const 19))
  (global $NEWLINE_AT i32 (i32.const 19))
  (global $TEN i64 (i64.const 10))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))
  (global $MINUS i32 (i32.const 45))
  (global $ZERO i32 (i32.const 48))
  (global $NINE i32 (i32.const 9))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then what the host writes, the
  ;; number read and the output, then the arguments' text.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 23))
  (data (i32.const 1024) "usage: absdiff <a> <b>\0a")
  (global $nan i32 (i32.const 1056))
  (global $nan.rosize i32 (i32.const 28))
  (data (i32.const 1056) "absdiff: not a whole number\0a")
  (global $range i32 (i32.const 1088))
  (global $range.rosize i32 (i32.const 29))
  (data (i32.const 1088) "absdiff: number out of range\0a")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 2048))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then a and b.
  (global $argv i32 (i32.const 2064))
  (global $argv.size i32 (i32.const 12))
  ;; The number read, which main lends parse_bounded to write.
  (global $number i32 (i32.const 2080))
  (global $number.size i32 (i32.const 8))
  ;; The output: up to 19 digits and a newline.
  (global $out i32 (i32.const 2096))
  (global $out.size i32 (i32.const 20))
  ;; The arguments' text, each ended by a NUL: 128 KiB, more than any test's arguments take.
  (global $args i32 (i32.const 4096))  ;; shared: main has the host fill it; parse_bounded reads it
  (global $args.size i32 (i32.const 131072))

  ;; Reads the two arguments, parses them and prints how far apart they are, or says why it
  ;; can't. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $size i32)
    (local $a i64)
    (local $difference i64)
    (block $write_failed
      (block $out_of_range
        (block $not_a_number
          (block $usage
            (local.set $err
              (call $args_sizes_get (global.get $sizes)
                (i32.add (global.get $sizes) (i32.const 4))))
            (if (local.get $err)
              (then (call $assert_fail (i32.const 1)) unreachable))
            (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
            ;; Text longer than $args holds isn't two numbers the program takes.
            (local.set $size (i32.load offset=4 (global.get $sizes)))
            (br_if $usage (i32.gt_u (local.get $size) (global.get $args.size)))
            (local.set $err (call $args_get (global.get $argv) (global.get $args)))
            (if (local.get $err)
              (then (call $assert_fail (i32.const 2)) unreachable))
            ;; Each number starts where its argv entry points: an offset into $args, which
            ;; parse_bounded checks. The first that isn't a number in range is the one reported.
            (local.set $err
              (call $parse_bounded
                (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args))
                (global.get $number)))
            (br_if $not_a_number (i32.eq (local.get $err) (global.get $ERROR_INVALID)))
            (br_if $out_of_range (local.get $err))
            (local.set $a (i64.load (global.get $number)))
            (local.set $err
              (call $parse_bounded
                (i32.sub (i32.load offset=8 (global.get $argv)) (global.get $args))
                (global.get $number)))
            (br_if $not_a_number (i32.eq (local.get $err) (global.get $ERROR_INVALID)))
            (br_if $out_of_range (local.get $err))
            ;; Both numbers are from -1000000 to 1000000, so their difference fits.
            (local.set $difference (i64.sub (local.get $a) (i64.load (global.get $number))))
            (if (i64.lt_s (local.get $difference) (i64.const 0))
              (then (local.set $difference (i64.sub (i64.const 0) (local.get $difference)))))
            (br_if $write_failed (call $write_u64 (local.get $difference)))
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
      (br_if $write_failed
        (call $write_all (global.get $STDERR) (global.get $range) (global.get $range.rosize)))
      (call $proc_exit (global.get $EXIT_OUT_OF_RANGE))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Parses the decimal whole number that starts at byte $at of $args: an optional minus sign,
  ;; then one or more digits, ended by the argument's NUL. Writes it at $into, 8 bytes. Gives 0,
  ;; ERROR_INVALID when the text isn't a whole number, or ERROR_RANGE when it is one outside
  ;; -1000000 to 1000000.
  (func $parse_bounded (param $at i32) (param $into i32) (result i32)  ;; returns: result
    (local $i i32)
    (local $c i32)
    (local $start i32)
    (local $left i32)
    (local $n i64)
    (local $status i32)
    (local.set $status (global.get $ERROR_INVALID))
    (local.set $i (local.get $at))
    (if (i32.ge_u (local.get $i) (global.get $args.size))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (if (i32.eq (i32.load8_u (i32.add (global.get $args) (local.get $i))) (global.get $MINUS))
      (then (local.set $i (i32.add (local.get $i) (i32.const 1)))))
    (local.set $start (local.get $i))
    (block $rejected
      (block $end
        (local.set $left (global.get $args.size))
        (loop $digit  ;; bound: 131072
          (if (i32.ge_u (local.get $i) (global.get $args.size))
            (then (call $assert_fail (i32.const 4)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $i))))
          (br_if $end (i32.eqz (local.get $c)))
          ;; Below '0' wraps to a large number, so one unsigned test catches both sides.
          (local.set $c (i32.sub (local.get $c) (global.get $ZERO)))
          (br_if $rejected (i32.gt_u (local.get $c) (global.get $NINE)))
          ;; Past the limit the number is out of range, and its digits are only checked.
          (if (i64.le_u (local.get $n) (global.get $NUM_LIMIT))
            (then
              (local.set $n
                (i64.add (i64.mul (local.get $n) (global.get $TEN))
                  (i64.extend_i32_u (local.get $c))))))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $digit (local.get $left)))
        ;; Every argument ends with its NUL inside $args.
        (call $assert_fail (i32.const 5))
        unreachable)
      ;; The NUL: a number has at least one digit before it.
      (br_if $rejected (i32.eq (local.get $i) (local.get $start)))
      (local.set $status (global.get $ERROR_RANGE))
      (br_if $rejected (i64.gt_u (local.get $n) (global.get $NUM_LIMIT)))
      (if (i32.ne (local.get $start) (local.get $at))
        (then (local.set $n (i64.sub (i64.const 0) (local.get $n)))))
      (i64.store (local.get $into) (local.get $n))
      (local.set $status (global.get $NO_ERROR)))
    (local.get $status))

  ;; Writes $value, at least 0, in decimal, then a newline, to standard output: the newline last,
  ;; and the digits before it from the last. Gives 0, or an error number.
  (func $write_u64 (param $value i64) (result i32)  ;; returns: result
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
      ;; 19 digits are all a number below 2^63 has.
      (call $assert_fail (i32.const 8))
      unreachable)
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
