;; hypotenuse: prints the length of the hypotenuse of a right triangle from the lengths of its two
;; sides, with three decimals. The WebAssembly port of the native hypotenuse, in the
;; WebAssembly dialect 1 (references/power-of-10-webassembly.md): lengua's library gives
;; $assert_fail and $write_all. Each side is read exactly, in thousandths; the square root is a
;; double's, as the idea's tests compute it, and its thousandths round to the nearest, ties to
;; even. Every float comes from a whole number, so none is NaN.
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
  ;; Sizes and limits: a side's whole part is at most MAX_WHOLE and its decimals at most
  ;; MAX_PLACES, and it is from 1 to MAX_UNITS thousandths.
  (global $ARGC_EXPECTED i32 (i32.const 3))
  (global $MAX_PLACES i32 (i32.const 3))
  (global $MAX_WHOLE i64 (i64.const 1000000))
  (global $UNITS i64 (i64.const 1000))
  (global $MAX_UNITS i64 (i64.const 1000000000))
  ;; What a mark gives once it's seen: a point, or a whole digit.
  (global $SEEN i32 (i32.const 1))
  ;; Output: up to 16 whole digits, a point, 3 decimals and a newline.
  (global $MAX_WHOLE_DIGITS i32 (i32.const 16))
  (global $NEWLINE_AT i32 (i32.const 20))
  (global $TEN i64 (i64.const 10))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))
  (global $POINT i32 (i32.const 46))
  (global $ZERO i32 (i32.const 48))
  (global $NINE i32 (i32.const 9))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then what the host writes, the
  ;; side read and the output, then the arguments' text.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 26))
  (data (i32.const 1024) "usage: hypotenuse <a> <b>\0a")
  (global $nan i32 (i32.const 1056))
  (global $nan.rosize i32 (i32.const 28))
  (data (i32.const 1056) "error: side is not a number\0a")
  (global $range i32 (i32.const 1088))
  (global $range.rosize i32 (i32.const 25))
  (data (i32.const 1088) "error: side out of range\0a")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 2048))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then a and b.
  (global $argv i32 (i32.const 2064))
  (global $argv.size i32 (i32.const 12))
  ;; The side read, in thousandths, which main lends parse_side to write.
  (global $number i32 (i32.const 2080))
  (global $number.size i32 (i32.const 8))
  ;; The output: up to 16 whole digits, a point, 3 decimals and a newline.
  (global $out i32 (i32.const 2096))
  (global $out.size i32 (i32.const 21))
  ;; The arguments' text, each ended by a NUL: 128 KiB, more than any test's arguments take.
  (global $args i32 (i32.const 4096))  ;; shared: main has the host fill it; parse_side reads it
  (global $args.size i32 (i32.const 131072))

  ;; Reads the two sides and prints the hypotenuse, or says why it can't. Never returns: every
  ;; path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $size i32)
    (local $a i64)
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
            ;; Text longer than $args holds isn't two sides the program takes.
            (local.set $size (i32.load offset=4 (global.get $sizes)))
            (br_if $usage (i32.gt_u (local.get $size) (global.get $args.size)))
            (local.set $err (call $args_get (global.get $argv) (global.get $args)))
            (if (local.get $err)
              (then (call $assert_fail (i32.const 2)) unreachable))
            ;; Each side starts where its argv entry points: an offset into $args, which
            ;; parse_side checks. The first that isn't a side in range is the one reported.
            (local.set $err
              (call $parse_side
                (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args))
                (global.get $number)))
            (br_if $not_a_number (i32.eq (local.get $err) (global.get $ERROR_INVALID)))
            (br_if $out_of_range (local.get $err))
            (local.set $a (i64.load (global.get $number)))
            (local.set $err
              (call $parse_side
                (i32.sub (i32.load offset=8 (global.get $argv)) (global.get $args))
                (global.get $number)))
            (br_if $not_a_number (i32.eq (local.get $err) (global.get $ERROR_INVALID)))
            (br_if $out_of_range (local.get $err))
            (br_if $write_failed
              (call $write_fixed3
                (call $hypotenuse (local.get $a) (i64.load (global.get $number)))))
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

  ;; Reads the side that starts at byte $at of $args: digits, then perhaps a point and one to
  ;; three digits, ended by the argument's NUL. Writes it in thousandths at $into, 8 bytes. Gives
  ;; 0, ERROR_INVALID when the text isn't such a number, or ERROR_RANGE when it is one outside
  ;; 0.001 to 1000000.000.
  (func $parse_side (param $at i32) (param $into i32) (result i32)  ;; returns: result
    (local $i i32)
    (local $c i32)
    (local $digits i32)
    (local $point i32)
    (local $places i32)
    (local $whole i64)
    (local $fraction i64)
    (local $left i32)
    (local $status i32)
    (local.set $status (global.get $ERROR_INVALID))
    (local.set $i (local.get $at))
    (block $rejected
      (block $end
        (local.set $left (global.get $args.size))
        (loop $char  ;; bound: 131072
          (if (i32.ge_u (local.get $i) (global.get $args.size))
            (then (call $assert_fail (i32.const 3)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $i))))
          (br_if $end (i32.eqz (local.get $c)))
          (block $next
            ;; One point, between the whole part and the decimals.
            (if (i32.eq (local.get $c) (global.get $POINT))
              (then
                (br_if $rejected (local.get $point))
                (local.set $point (global.get $SEEN))
                (br $next)))
            ;; Below '0' wraps to a large number, so one unsigned test catches both sides.
            (local.set $c (i32.sub (local.get $c) (global.get $ZERO)))
            (br_if $rejected (i32.gt_u (local.get $c) (global.get $NINE)))
            (if (local.get $point)
              (then
                (local.set $places (i32.add (local.get $places) (i32.const 1)))
                (br_if $rejected (i32.gt_u (local.get $places) (global.get $MAX_PLACES)))
                (local.set $fraction
                  (i64.add (i64.mul (local.get $fraction) (global.get $TEN))
                    (i64.extend_i32_u (local.get $c))))
                (br $next)))
            (local.set $digits (global.get $SEEN))
            ;; Past MAX_WHOLE the side is out of range, and its digits are only checked.
            (if (i64.le_u (local.get $whole) (global.get $MAX_WHOLE))
              (then
                (local.set $whole
                  (i64.add (i64.mul (local.get $whole) (global.get $TEN))
                    (i64.extend_i32_u (local.get $c)))))))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $char (local.get $left)))
        ;; Every argument ends with its NUL inside $args.
        (call $assert_fail (i32.const 4))
        unreachable)
      ;; The NUL: a side has whole digits, and a point has decimals after it.
      (br_if $rejected (i32.eqz (local.get $digits)))
      (br_if $rejected (i32.and (local.get $point) (i32.eqz (local.get $places))))
      ;; The decimals in thousandths: padded to three places.
      (block $padded
        (local.set $left (global.get $MAX_PLACES))
        (loop $pad  ;; bound: 3
          (br_if $padded (i32.ge_u (local.get $places) (global.get $MAX_PLACES)))
          (local.set $fraction (i64.mul (local.get $fraction) (global.get $TEN)))
          (local.set $places (i32.add (local.get $places) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $pad (local.get $left))))
      (local.set $status (global.get $ERROR_RANGE))
      (br_if $rejected (i64.gt_u (local.get $whole) (global.get $MAX_WHOLE)))
      (local.set $whole
        (i64.add (i64.mul (local.get $whole) (global.get $UNITS)) (local.get $fraction)))
      (br_if $rejected (i64.eqz (local.get $whole)))
      (br_if $rejected (i64.gt_u (local.get $whole) (global.get $MAX_UNITS)))
      (i64.store (local.get $into) (local.get $whole))
      (local.set $status (global.get $NO_ERROR)))
    (local.get $status))

  ;; Gives the hypotenuse of the right triangle whose sides are $a and $b thousandths, each from
  ;; 1 to MAX_UNITS, in thousandths: each side a double, divided by UNITS, then the square root
  ;; of the sum of their squares, times UNITS, rounded to the nearest whole number, ties to even.
  (func $hypotenuse (param $a i64) (param $b i64) (result i64)
    (local $units f64)
    (local $x f64)
    (local $y f64)
    (if (i64.eqz (local.get $a))
      (then (call $assert_fail (i32.const 5)) unreachable))
    (if (i64.eqz (local.get $b))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (local.set $units (f64.convert_i64_s (global.get $UNITS)))
    (local.set $x (f64.div (f64.convert_i64_s (local.get $a)) (local.get $units)))
    (local.set $y (f64.div (f64.convert_i64_s (local.get $b)) (local.get $units)))
    (i64.trunc_sat_f64_s
      (f64.nearest
        (f64.mul
          (f64.sqrt
            (f64.add (f64.mul (local.get $x) (local.get $x))
              (f64.mul (local.get $y) (local.get $y))))
          (local.get $units)))))

  ;; Writes $value thousandths, at least 0, with three decimals, then a newline, to standard
  ;; output: the decimals from the last, a point, then the whole digits from the last, at least
  ;; one. Gives 0, or an error number.
  (func $write_fixed3 (param $value i64) (result i32)  ;; returns: result
    (local $rest i64)
    (local $at i32)
    (local $left i32)
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (local.set $rest (local.get $value))
    (local.set $at (global.get $NEWLINE_AT))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (local.set $left (global.get $MAX_PLACES))
    (loop $place  ;; bound: 3
      (local.set $at (i32.sub (local.get $at) (i32.const 1)))
      (if (i32.ge_u (local.get $at) (global.get $out.size))
        (then (call $assert_fail (i32.const 8)) unreachable))
      (i32.store8 (i32.add (global.get $out) (local.get $at))
        (i32.add (global.get $ZERO)
          (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
      (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $place (local.get $left)))
    (local.set $at (i32.sub (local.get $at) (i32.const 1)))
    (if (i32.ge_u (local.get $at) (global.get $out.size))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $POINT))
    (block $written
      (local.set $left (global.get $MAX_WHOLE_DIGITS))
      (loop $digit  ;; bound: 16
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 10)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; A value below 2^63 has at most 16 whole digits before its three decimals.
      (call $assert_fail (i32.const 11))
      unreachable)
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
