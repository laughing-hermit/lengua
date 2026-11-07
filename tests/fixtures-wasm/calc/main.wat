;; calc: adds, subtracts or multiplies two whole numbers from its arguments, with an operator
;; between them. The WebAssembly counterpart of tests/fixtures/calc, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $assert_fail and $write_all.
;; The operator's character picks its case through a br_table, whose default is an assertion: a
;; check before it leaves only the cases' characters.
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
  (global $EXIT_UNKNOWN_OPERATOR i32 (i32.const 3))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  ;; What the functions marked "returns: result" give.
  (global $NO_ERROR i32 (i32.const 0))
  (global $ERROR_INVALID i32 (i32.const 28))
  ;; Sizes and limits: a number is at most 20 characters after its sign, and from MIN_VALUE to
  ;; MAX_VALUE, so no result overflows.
  (global $ARGC_EXPECTED i32 (i32.const 4))
  (global $MAX_CHARS i32 (i32.const 20))
  (global $MAX_TENTH i64 (i64.const 922337203685477580))
  (global $MAX_MAGNITUDE i64 (i64.const 9223372036854775807))
  (global $MIN_VALUE i64 (i64.const -1000000000))
  (global $MAX_VALUE i64 (i64.const 1000000000))
  (global $MAX_DIGITS i32 (i32.const 19))
  (global $NEWLINE_AT i32 (i32.const 20))
  (global $TEN i64 (i64.const 10))
  ;; The operators' characters run from `*` (42) through `+` and `,` to `-`: a case each.
  (global $FIRST_OPERATOR i32 (i32.const 42))
  (global $OPERATORS i32 (i32.const 4))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))
  (global $MINUS i32 (i32.const 45))
  (global $ZERO i32 (i32.const 48))
  (global $NINE i32 (i32.const 9))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then what the host writes, the
  ;; number read and the output, then the arguments' text.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 25))
  (data (i32.const 1024) "usage: calc <a> <op> <b>\0a")
  (global $nan i32 (i32.const 1056))
  (global $nan.rosize i32 (i32.const 20))
  (data (i32.const 1056) "error: not a number\0a")
  (global $bad_operator i32 (i32.const 1088))
  (global $bad_operator.rosize i32 (i32.const 24))
  (data (i32.const 1088) "error: unknown operator\0a")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 2048))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then a, the operator
  ;; and b.
  (global $argv i32 (i32.const 2064))
  (global $argv.size i32 (i32.const 16))
  ;; A number read, then the result, which main lends parse_number and apply to write.
  (global $number i32 (i32.const 2080))
  (global $number.size i32 (i32.const 8))
  ;; The output: a sign, 19 digits and a newline.
  (global $out i32 (i32.const 2096))
  (global $out.size i32 (i32.const 21))
  ;; The arguments' text, each ended by a NUL: 128 KiB, more than any test's arguments take.
  (global $args i32 (i32.const 4096))  ;; shared: main has the host fill it; the parsers read it
  (global $args.size i32 (i32.const 131072))

  ;; Reads the three arguments, applies the operator to the two numbers and prints the result,
  ;; or says why it can't. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $size i32)
    (local $a i64)
    (local $b i64)
    (block $write_failed
      (block $unknown
        (block $not_a_number
          (block $usage
            (local.set $err
              (call $args_sizes_get (global.get $sizes)
                (i32.add (global.get $sizes) (i32.const 4))))
            (if (local.get $err)
              (then (call $assert_fail (i32.const 1)) unreachable))
            (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
            ;; Text longer than $args holds isn't two numbers and an operator.
            (local.set $size (i32.load offset=4 (global.get $sizes)))
            (br_if $usage (i32.gt_u (local.get $size) (global.get $args.size)))
            (local.set $err (call $args_get (global.get $argv) (global.get $args)))
            (if (local.get $err)
              (then (call $assert_fail (i32.const 2)) unreachable))
            ;; Each argument starts where its argv entry points: an offset into $args, which the
            ;; parsers check. The numbers come first, so a wrong number and a wrong operator
            ;; report the number.
            (local.set $err
              (call $parse_number
                (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args))
                (global.get $number)))
            (br_if $not_a_number (local.get $err))
            (local.set $a (i64.load (global.get $number)))
            (local.set $err
              (call $parse_number
                (i32.sub (i32.load offset=12 (global.get $argv)) (global.get $args))
                (global.get $number)))
            (br_if $not_a_number (local.get $err))
            (local.set $b (i64.load (global.get $number)))
            (local.set $err
              (call $apply
                (i32.sub (i32.load offset=8 (global.get $argv)) (global.get $args))
                (local.get $a) (local.get $b) (global.get $number)))
            (br_if $unknown (local.get $err))
            (br_if $write_failed (call $write_i64 (i64.load (global.get $number))))
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
        (call $write_all (global.get $STDERR) (global.get $bad_operator)
          (global.get $bad_operator.rosize)))
      (call $proc_exit (global.get $EXIT_UNKNOWN_OPERATOR))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Parses the decimal whole number that starts at byte $at of $args: an optional minus sign,
  ;; then at most 20 digits, ended by the argument's NUL. Writes it at $into, 8 bytes. Gives 0,
  ;; or ERROR_INVALID when the text isn't a number from MIN_VALUE to MAX_VALUE.
  (func $parse_number (param $at i32) (param $into i32) (result i32)  ;; returns: result
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
      ;; The NUL: a number has at least one digit before it, and lies in its range.
      (br_if $invalid (i32.eq (local.get $i) (local.get $start)))
      (if (i32.ne (local.get $start) (local.get $at))
        (then (local.set $n (i64.sub (i64.const 0) (local.get $n)))))
      (br_if $invalid (i64.lt_s (local.get $n) (global.get $MIN_VALUE)))
      (br_if $invalid (i64.gt_s (local.get $n) (global.get $MAX_VALUE)))
      (i64.store (local.get $into) (local.get $n))
      (local.set $status (global.get $NO_ERROR)))
    (local.get $status))

  ;; Applies the operator that starts at byte $at of $args, one character and the argument's NUL,
  ;; to $a and $b, each from MIN_VALUE to MAX_VALUE, and writes the result at $into, 8 bytes.
  ;; Gives 0, or ERROR_INVALID when the operator isn't one of `+`, `-` and `*`.
  (func $apply (param $at i32) (param $a i64) (param $b i64) (param $into i32)  ;; returns: result
    (result i32)
    (local $i i32)
    (local $c i32)
    (local $index i32)
    (local $result i64)
    (local $status i32)
    (local.set $status (global.get $ERROR_INVALID))
    (block $unknown
      (block $applied
        ;; The operator's one character, then its NUL.
        (local.set $i (local.get $at))
        (if (i32.ge_u (local.get $i) (global.get $args.size))
          (then (call $assert_fail (i32.const 5)) unreachable))
        (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $i))))
        (br_if $unknown (i32.eqz (local.get $c)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (if (i32.ge_u (local.get $i) (global.get $args.size))
          (then (call $assert_fail (i32.const 6)) unreachable))
        (br_if $unknown (i32.load8_u (i32.add (global.get $args) (local.get $i))))
        ;; The character's case, from FIRST_OPERATOR on: below it wraps to a large number, which
        ;; the check leaves with the characters past the cases.
        (local.set $index (i32.sub (local.get $c) (global.get $FIRST_OPERATOR)))
        (br_if $unknown (i32.ge_u (local.get $index) (global.get $OPERATORS)))
        (block $subtract
          (block $add
            (block $multiply
              (block $past
                (br_table $multiply $add $unknown $subtract $past (local.get $index)))
              ;; Every index below OPERATORS has a case of its own.
              (call $assert_fail (i32.const 7))
              unreachable)
            (local.set $result (i64.mul (local.get $a) (local.get $b)))
            (br $applied))
          (local.set $result (i64.add (local.get $a) (local.get $b)))
          (br $applied))
        (local.set $result (i64.sub (local.get $a) (local.get $b))))
      (i64.store (local.get $into) (local.get $result))
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
      (then (call $assert_fail (i32.const 8)) unreachable))
    ;; The newline last, the digits before it from the last, then the sign.
    (local.set $at (global.get $NEWLINE_AT))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (block $sign
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 19
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 9)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $magnitude) (global.get $TEN)))))
        (local.set $magnitude (i64.div_u (local.get $magnitude) (global.get $TEN)))
        (br_if $sign (i64.eqz (local.get $magnitude)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; 19 digits are all a magnitude has.
      (call $assert_fail (i32.const 10))
      unreachable)
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 11)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $MINUS))))
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
