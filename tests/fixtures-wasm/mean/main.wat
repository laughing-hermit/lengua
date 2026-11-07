;; mean: prints the mean of the two decimal numbers in its arguments, with two decimal places. The
;; WebAssembly counterpart of examples/mean, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $assert_fail and $write_all.
;; The numbers are read exactly, in whole cents, and their mean rounds a half cent away from
;; zero, so no float takes part.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; Five pages: lengua's first 1024 bytes, the messages and the small regions, then the
  ;; arguments' text from 4096, and the message that shows a wrong argument after it.
  (memory (export "memory") 5 5)

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
  ;; Sizes and limits: a number's whole part is at most MAX_WHOLE, and the number at most
  ;; MAX_CENTS cents either way.
  (global $ARGC_EXPECTED i32 (i32.const 3))
  (global $MAX_WHOLE i64 (i64.const 1000000))
  (global $MAX_CENTS i64 (i64.const 100000000))
  (global $CENTS_PER_UNIT i64 (i64.const 100))
  (global $TEN i64 (i64.const 10))
  ;; A half cent rounds away from zero: a cent more that way, then half, rounded toward zero.
  (global $CENT_UP i64 (i64.const 1))
  (global $CENT_DOWN i64 (i64.const -1))
  (global $TWO i64 (i64.const 2))
  ;; Output: a sign, up to 19 whole digits, the point at POINT_AT, the tens of cents at TENS_AT,
  ;; the cents at CENTS_AT and the newline at NEWLINE_AT.
  (global $MAX_DIGITS i32 (i32.const 19))
  (global $POINT_AT i32 (i32.const 20))
  (global $TENS_AT i32 (i32.const 21))
  (global $CENTS_AT i32 (i32.const 22))
  (global $NEWLINE_AT i32 (i32.const 23))
  ;; A message's start, before the argument it shows; the last place of the message the argument
  ;; may fill, which keeps room for the newline.
  (global $PREFIX_LEN i32 (i32.const 20))
  (global $LAST_SHOWN i32 (i32.const 131103))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))
  (global $MINUS i32 (i32.const 45))
  (global $POINT i32 (i32.const 46))
  (global $ZERO i32 (i32.const 48))
  (global $NINE i32 (i32.const 9))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then what the host writes, the
  ;; number read and the output, then the arguments' text and the message that shows one.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 20))
  (data (i32.const 1024) "usage: mean <a> <b>\0a")
  (global $nan i32 (i32.const 1056))
  (global $nan.rosize i32 (i32.const 20))
  (data (i32.const 1056) "mean: not a number: ")
  (global $range i32 (i32.const 1088))
  (global $range.rosize i32 (i32.const 20))
  (data (i32.const 1088) "mean: out of range: ")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 2048))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then a and b.
  (global $argv i32 (i32.const 2064))
  (global $argv.size i32 (i32.const 12))
  ;; The number read, in cents, which main lends parse_cents to write.
  (global $number i32 (i32.const 2080))
  (global $number.size i32 (i32.const 8))
  ;; The output: a sign, up to 19 whole digits, a point, two decimals and a newline.
  (global $out i32 (i32.const 2096))
  (global $out.size i32 (i32.const 24))
  ;; The arguments' text, each ended by a NUL: 128 KiB, more than any test's arguments take.
  (global $args i32 (i32.const 4096))  ;; shared: main has the host fill it; the readers read it
  (global $args.size i32 (i32.const 131072))
  ;; A message's start, a wrong argument and a newline, which report writes at once.
  (global $message i32 (i32.const 135168))
  (global $message.size i32 (i32.const 131104))

  ;; Reads the two numbers and prints their mean, or says which argument is wrong and why. Never
  ;; returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $size i32)
    (local $at i32)
    (local $sum i64)
    (block $write_failed
      (block $bad
        (block $usage
          (local.set $err
            (call $args_sizes_get (global.get $sizes) (i32.add (global.get $sizes) (i32.const 4))))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 1)) unreachable))
          (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
          ;; Text longer than $args holds isn't two numbers the program takes.
          (local.set $size (i32.load offset=4 (global.get $sizes)))
          (br_if $usage (i32.gt_u (local.get $size) (global.get $args.size)))
          (local.set $err (call $args_get (global.get $argv) (global.get $args)))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 2)) unreachable))
          ;; Each number starts where its argv entry points: an offset into $args, which the
          ;; readers check. The first that isn't a number in range is the one reported.
          (local.set $at (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args)))
          (local.set $err (call $parse_cents (local.get $at) (global.get $number)))
          (br_if $bad (local.get $err))
          (local.set $sum (i64.load (global.get $number)))
          (local.set $at (i32.sub (i32.load offset=8 (global.get $argv)) (global.get $args)))
          (local.set $err (call $parse_cents (local.get $at) (global.get $number)))
          (br_if $bad (local.get $err))
          (local.set $sum (i64.add (local.get $sum) (i64.load (global.get $number))))
          ;; Half the sum: a cent more away from zero first, and the division rounds toward zero.
          (local.set $sum
            (i64.add (local.get $sum)
              (select (global.get $CENT_DOWN) (global.get $CENT_UP)
                (i64.lt_s (local.get $sum) (i64.const 0)))))
          (br_if $write_failed
            (call $print_cents (i64.div_s (local.get $sum) (global.get $TWO))))
          (call $proc_exit (global.get $EXIT_OK))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
        (call $proc_exit (global.get $EXIT_USAGE))
        unreachable)
      ;; The argument at $at is out of range, or isn't a number.
      (if (i32.eq (local.get $err) (global.get $ERROR_RANGE))
        (then
          (br_if $write_failed (call $report (global.get $range) (local.get $at)))
          (call $proc_exit (global.get $EXIT_OUT_OF_RANGE))
          unreachable))
      (br_if $write_failed (call $report (global.get $nan) (local.get $at)))
      (call $proc_exit (global.get $EXIT_NOT_A_NUMBER))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Reads the number that starts at byte $at of $args: perhaps a minus sign, one or more digits,
  ;; then perhaps a point and one or two digits, ended by the argument's NUL. Writes it in cents
  ;; at $into, 8 bytes, which it lends parse_fraction first. Gives 0, ERROR_INVALID when the text
  ;; isn't such a number, or ERROR_RANGE when it is one outside -1000000.00 to 1000000.00.
  (func $parse_cents (param $at i32) (param $into i32) (result i32)  ;; returns: result
    (local $i i32)
    (local $c i32)
    (local $start i32)
    (local $whole i64)
    (local $cents i64)
    (local $left i32)
    (local $status i32)
    (local.set $status (global.get $ERROR_INVALID))
    (local.set $i (local.get $at))
    (if (i32.ge_u (local.get $i) (global.get $args.size))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (if (i32.eq (i32.load8_u (i32.add (global.get $args) (local.get $i))) (global.get $MINUS))
      (then (local.set $i (i32.add (local.get $i) (i32.const 1)))))
    (local.set $start (local.get $i))
    (block $rejected
      ;; The whole digits, up to the first byte that isn't one.
      (block $whole_read
        (local.set $left (global.get $args.size))
        (loop $digit  ;; bound: 131072
          (if (i32.ge_u (local.get $i) (global.get $args.size))
            (then (call $assert_fail (i32.const 4)) unreachable))
          ;; Below '0' wraps to a large number, so one unsigned test catches both sides.
          (local.set $c
            (i32.sub (i32.load8_u (i32.add (global.get $args) (local.get $i))) (global.get $ZERO)))
          (br_if $whole_read (i32.gt_u (local.get $c) (global.get $NINE)))
          ;; Past MAX_WHOLE the number is out of range, and its digits are only read.
          (if (i64.le_u (local.get $whole) (global.get $MAX_WHOLE))
            (then
              (local.set $whole
                (i64.add (i64.mul (local.get $whole) (global.get $TEN))
                  (i64.extend_i32_u (local.get $c))))))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $digit (local.get $left)))
        ;; Every argument ends with its NUL inside $args.
        (call $assert_fail (i32.const 5))
        unreachable)
      ;; A number has a whole digit, then its NUL or its decimals.
      (br_if $rejected (i32.eq (local.get $i) (local.get $start)))
      (local.set $status (call $parse_fraction (local.get $i) (local.get $into)))
      (br_if $rejected (local.get $status))
      (local.set $status (global.get $ERROR_RANGE))
      (local.set $cents
        (i64.add (i64.mul (local.get $whole) (global.get $CENTS_PER_UNIT))
          (i64.load (local.get $into))))
      (br_if $rejected (i64.gt_u (local.get $cents) (global.get $MAX_CENTS)))
      (if (i32.ne (local.get $start) (local.get $at))
        (then (local.set $cents (i64.sub (i64.const 0) (local.get $cents)))))
      (i64.store (local.get $into) (local.get $cents))
      (local.set $status (global.get $NO_ERROR)))
    (local.get $status))

  ;; Reads what follows a number's whole digits, from byte $at of $args: the argument's NUL, or a
  ;; point, one or two digits and the NUL. Writes the cents the decimals make, 0 to 99, at $into,
  ;; 8 bytes. Gives 0, or ERROR_INVALID when it is anything else.
  (func $parse_fraction (param $at i32) (param $into i32) (result i32)  ;; returns: result
    (local $i i32)
    (local $c i32)
    (local $cents i64)
    (local $status i32)
    (local.set $status (global.get $ERROR_INVALID))
    (block $rejected
      (local.set $i (local.get $at))
      (if (i32.ge_u (local.get $i) (global.get $args.size))
        (then (call $assert_fail (i32.const 6)) unreachable))
      (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $i))))
      (block $read
        (br_if $read (i32.eqz (local.get $c)))
        (br_if $rejected (i32.ne (local.get $c) (global.get $POINT)))
        ;; The first decimal: the tens of cents.
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (if (i32.ge_u (local.get $i) (global.get $args.size))
          (then (call $assert_fail (i32.const 7)) unreachable))
        (local.set $c
          (i32.sub (i32.load8_u (i32.add (global.get $args) (local.get $i))) (global.get $ZERO)))
        (br_if $rejected (i32.gt_u (local.get $c) (global.get $NINE)))
        (local.set $cents (i64.mul (i64.extend_i32_u (local.get $c)) (global.get $TEN)))
        ;; Then the NUL, or the second decimal and the NUL.
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (if (i32.ge_u (local.get $i) (global.get $args.size))
          (then (call $assert_fail (i32.const 8)) unreachable))
        (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $i))))
        (br_if $read (i32.eqz (local.get $c)))
        (local.set $c (i32.sub (local.get $c) (global.get $ZERO)))
        (br_if $rejected (i32.gt_u (local.get $c) (global.get $NINE)))
        (local.set $cents (i64.add (local.get $cents) (i64.extend_i32_u (local.get $c))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (if (i32.ge_u (local.get $i) (global.get $args.size))
          (then (call $assert_fail (i32.const 9)) unreachable))
        (br_if $rejected (i32.load8_u (i32.add (global.get $args) (local.get $i)))))
      (i64.store (local.get $into) (local.get $cents))
      (local.set $status (global.get $NO_ERROR)))
    (local.get $status))

  ;; Writes $value cents as a decimal with two places, then a newline, to standard output: the
  ;; cents and the newline at the end of $out, then the whole digits from the last, at least
  ;; one, then the sign. Gives 0, or an error number.
  (func $print_cents (param $value i64) (result i32)  ;; returns: result
    (local $magnitude i64)
    (local $at i32)
    (local $left i32)
    (local.set $magnitude (local.get $value))
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then (local.set $magnitude (i64.sub (i64.const 0) (local.get $value)))))
    ;; The most negative number has no magnitude in 64 bits.
    (if (i64.lt_s (local.get $magnitude) (i64.const 0))
      (then (call $assert_fail (i32.const 10)) unreachable))
    (i32.store8 (i32.add (global.get $out) (global.get $NEWLINE_AT)) (global.get $NEWLINE))
    (i32.store8 (i32.add (global.get $out) (global.get $CENTS_AT))
      (i32.add (global.get $ZERO)
        (i32.wrap_i64 (i64.rem_u (local.get $magnitude) (global.get $TEN)))))
    (i32.store8 (i32.add (global.get $out) (global.get $TENS_AT))
      (i32.add (global.get $ZERO)
        (i32.wrap_i64
          (i64.rem_u (i64.div_u (local.get $magnitude) (global.get $TEN)) (global.get $TEN)))))
    (i32.store8 (i32.add (global.get $out) (global.get $POINT_AT)) (global.get $POINT))
    (local.set $magnitude (i64.div_u (local.get $magnitude) (global.get $CENTS_PER_UNIT)))
    (local.set $at (global.get $POINT_AT))
    (block $sign
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 19
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 11)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $magnitude) (global.get $TEN)))))
        (local.set $magnitude (i64.div_u (local.get $magnitude) (global.get $TEN)))
        (br_if $sign (i64.eqz (local.get $magnitude)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; 19 digits are all the whole part of a magnitude has.
      (call $assert_fail (i32.const 12))
      unreachable)
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 13)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $MINUS))))
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))

  ;; Writes the message start at $prefix, PREFIX_LEN bytes, which main lends it, then the argument
  ;; that starts at byte $at of $args, then a newline, to standard error in one write. Gives 0, or
  ;; an error number.
  (func $report (param $prefix i32) (param $at i32) (result i32)  ;; returns: result
    (local $k i32)
    (local $from i32)
    (local $to i32)
    (local $c i32)
    (local $left i32)
    (memory.copy (global.get $message) (local.get $prefix) (global.get $PREFIX_LEN))
    (local.set $to (global.get $PREFIX_LEN))
    (block $copied
      (local.set $left (global.get $args.size))
      (loop $byte  ;; bound: 131072
        (local.set $from (i32.add (local.get $at) (local.get $k)))
        (if (i32.ge_u (local.get $from) (global.get $args.size))
          (then (call $assert_fail (i32.const 14)) unreachable))
        (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $from))))
        (br_if $copied (i32.eqz (local.get $c)))
        (if (i32.ge_u (local.get $to) (global.get $LAST_SHOWN))
          (then (call $assert_fail (i32.const 15)) unreachable))
        (i32.store8 (i32.add (global.get $message) (local.get $to)) (local.get $c))
        (local.set $to (i32.add (local.get $to) (i32.const 1)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left)))
      ;; Every argument ends with its NUL inside $args.
      (call $assert_fail (i32.const 16))
      unreachable)
    (if (i32.ge_u (local.get $to) (global.get $message.size))
      (then (call $assert_fail (i32.const 17)) unreachable))
    (i32.store8 (i32.add (global.get $message) (local.get $to)) (global.get $NEWLINE))
    (call $write_all (global.get $STDERR) (global.get $message)
      (i32.add (local.get $to) (i32.const 1))))
)
