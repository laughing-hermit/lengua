;; negate: prints the negative of the whole number in its argument: the compile skill's main
;; template, in the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; every rule passes, and lengua's library gives $assert_fail and $write_all.
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
  ;; What the functions marked "returns: result" give.
  (global $NO_ERROR i32 (i32.const 0))
  (global $ERROR_INVALID i32 (i32.const 28))
  ;; Sizes and limits.
  (global $ARGC_EXPECTED i32 (i32.const 2))
  (global $MAX_CHARS i32 (i32.const 20))
  (global $MAX_DIGITS i32 (i32.const 19))
  (global $NEWLINE_AT i32 (i32.const 20))
  (global $MAX_TENTH i64 (i64.const 922337203685477580))
  (global $MAX_MAGNITUDE i64 (i64.const 9223372036854775807))
  (global $TEN i64 (i64.const 10))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))
  (global $MINUS i32 (i32.const 45))
  (global $ZERO i32 (i32.const 48))
  (global $NINE i32 (i32.const 9))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then what the host writes,
  ;; the number read and the output.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 18))
  (data (i32.const 1024) "usage: negate <n>\0a")
  (global $nan i32 (i32.const 1056))
  (global $nan.rosize i32 (i32.const 20))
  (data (i32.const 1056) "error: not a number\0a")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 2048))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then the number.
  (global $argv i32 (i32.const 2064))
  (global $argv.size i32 (i32.const 8))
  ;; The arguments' text, each ended by a NUL.
  (global $args i32 (i32.const 2080))  ;; shared: main has the host fill it; parse_i64 reads it
  (global $args.size i32 (i32.const 64))
  ;; The number read, which main lends parse_i64 to write.
  (global $number i32 (i32.const 2144))
  (global $number.size i32 (i32.const 8))
  ;; The output: a sign, 19 digits and a newline.
  (global $out i32 (i32.const 2160))
  (global $out.size i32 (i32.const 21))

  ;; Reads the argument, parses it and prints its negative, or says why it can't. Never
  ;; returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $size i32)
    (local $at i32)
    (local $n i64)
    (block $write_failed
      (block $not_a_number
        (block $usage
          (local.set $err
            (call $args_sizes_get (global.get $sizes) (i32.add (global.get $sizes) (i32.const 4))))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 1)) unreachable))
          (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
          ;; Text longer than $args holds isn't a number.
          (local.set $size (i32.load offset=4 (global.get $sizes)))
          (br_if $not_a_number (i32.gt_u (local.get $size) (global.get $args.size)))
          (local.set $err (call $args_get (global.get $argv) (global.get $args)))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 2)) unreachable))
          ;; The number starts where argv[1] points: an offset into $args, which parse_i64 checks.
          (local.set $at (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args)))
          (local.set $err (call $parse_i64 (local.get $at) (global.get $number)))
          (br_if $not_a_number (local.get $err))
          ;; parse_i64 never gives the most negative number, so its negative fits.
          (local.set $n (i64.load (global.get $number)))
          (br_if $write_failed (call $write_i64 (i64.sub (i64.const 0) (local.get $n))))
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

  ;; Parses the decimal whole number that starts at byte $at of $args: an optional minus sign,
  ;; then at most 19 digits, ended by the argument's NUL. Writes it at $into, 8 bytes.
  ;; Gives 0, or ERROR_INVALID when the text isn't a number from -9223372036854775807 to
  ;; 9223372036854775807.
  (func $parse_i64 (param $at i32) (param $into i32) (result i32)  ;; returns: result
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
    (block $invalid
      (block $end
        (local.set $left (global.get $MAX_CHARS))
        (loop $digit  ;; bound: 20
          (if (i32.ge_u (local.get $i) (global.get $args.size))
            (then (call $assert_fail (i32.const 4)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $i))))
          (br_if $end (i32.eqz (local.get $c)))
          ;; Below '0' wraps to a large number, so one unsigned test catches both sides.
          (local.set $c (i32.sub (local.get $c) (global.get $ZERO)))
          (br_if $invalid (i32.gt_u (local.get $c) (global.get $NINE)))
          (br_if $invalid (i64.gt_u (local.get $n) (global.get $MAX_TENTH)))
          (local.set $n (i64.mul (local.get $n) (global.get $TEN)))
          (br_if $invalid
            (i64.gt_u (local.get $n)
              (i64.sub (global.get $MAX_MAGNITUDE) (i64.extend_i32_u (local.get $c)))))
          (local.set $n (i64.add (local.get $n) (i64.extend_i32_u (local.get $c))))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $digit (local.get $left)))
        ;; More characters than a number has.
        (br $invalid))
      ;; The NUL: a number has at least one digit before it.
      (br_if $invalid (i32.eq (local.get $i) (local.get $start)))
      (if (i32.ne (local.get $start) (local.get $at))
        (then (local.set $n (i64.sub (i64.const 0) (local.get $n)))))
      (i64.store (local.get $into) (local.get $n))
      (local.set $status (global.get $NO_ERROR)))
    (local.get $status))

  ;; Writes $value in decimal, then a newline, to standard output. Gives 0, or an error number.
  (func $write_i64 (param $value i64) (result i32)  ;; returns: result
    (local $magnitude i64)
    (local $at i32)
    (local $left i32)
    (local.set $magnitude (local.get $value))
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then (local.set $magnitude (i64.sub (i64.const 0) (local.get $value)))))
    ;; The most negative number has no magnitude in 64 bits.
    (if (i64.lt_s (local.get $magnitude) (i64.const 0))
      (then (call $assert_fail (i32.const 5)) unreachable))
    ;; The newline last, the digits before it from the last, then the sign.
    (local.set $at (global.get $NEWLINE_AT))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (block $sign
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 19
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 6)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $magnitude) (global.get $TEN)))))
        (local.set $magnitude (i64.div_u (local.get $magnitude) (global.get $TEN)))
        (br_if $sign (i64.eqz (local.get $magnitude)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; 19 digits are all a magnitude has.
      (call $assert_fail (i32.const 7))
      unreachable)
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 8)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $MINUS))))
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at)))))
