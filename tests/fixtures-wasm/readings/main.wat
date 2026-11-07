;; readings: reads the clock that only moves forward as many times as its argument says, and prints
;; how many readings were not earlier than the previous one, so that a benchmark can measure what a
;; reading of the clock costs. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; lengua's library gives $write_all. Each reading is one call to clock_time_get, and the count is
;; parsed a digit at a time, as add parses its numbers.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "clock_time_get"
    (func $clock_time_get (param i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_WRITE_FAILED i32 (i32.const 2))
  (global $EXIT_CLOCK_FAILED i32 (i32.const 3))
  ;; What count_readings gives when the clock can't be read.
  (global $FAILED i32 (i32.const -1))
  ;; The clock that only moves forward, and the precision asked of it: a nanosecond.
  (global $MONOTONIC i32 (i32.const 1))
  (global $PRECISION i64 (i64.const 1))
  ;; Sizes and limits: one argument, a count from 1 to MAX_READINGS, which has at most MAX_DIGITS
  ;; digits; SCAN is those digits and the NUL after them.
  (global $ARGC_EXPECTED i32 (i32.const 2))
  (global $MAX_READINGS i32 (i32.const 1000000))
  (global $MAX_DIGITS i32 (i32.const 7))
  (global $SCAN i32 (i32.const 8))
  (global $BASE i32 (i32.const 10))
  ;; Printing: a count of up to 20 digits, then a newline.
  (global $PRINT_DIGITS i32 (i32.const 20))
  (global $NEWLINE_AT i32 (i32.const 20))
  (global $TEN i64 (i64.const 10))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))
  (global $ZERO i32 (i32.const 48))
  (global $NINE i32 (i32.const 9))

  ;; Regions (rule 3, M1), from 1024 up: the usage line, read-only, then what the host writes, the
  ;; reading of the clock and the line it prints.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 24))
  (data (i32.const 1024) "usage: readings <count>\0a")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 1056))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then the count.
  (global $argv i32 (i32.const 1072))
  (global $argv.size i32 (i32.const 8))
  ;; The arguments' text, each ended by a NUL.
  (global $args i32 (i32.const 1088))  ;; shared: main has the host fill it; parse_count reads it
  (global $args.size i32 (i32.const 64))
  ;; A reading of the clock, in nanoseconds, as clock_time_get writes it.
  (global $time i32 (i32.const 1152))
  (global $time.size i32 (i32.const 8))
  ;; The line it prints: a count of up to 20 digits, then a newline.
  (global $out i32 (i32.const 1168))
  (global $out.size i32 (i32.const 21))

  ;; Reads the count, reads the clock that many times and prints how many readings were not
  ;; earlier than the previous one, or the usage line. Never returns: every path ends in
  ;; proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $count i32)
    (local $kept i32)
    (block $write_failed
      (block $clock_failed
        (block $usage
          (local.set $err
            (call $args_sizes_get (global.get $sizes) (i32.add (global.get $sizes) (i32.const 4))))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 1)) unreachable))
          (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
          ;; Text longer than $args holds isn't a count.
          (br_if $usage
            (i32.gt_u (i32.load offset=4 (global.get $sizes)) (global.get $args.size)))
          (local.set $err (call $args_get (global.get $argv) (global.get $args)))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 2)) unreachable))
          ;; The count starts where its argv entry points: an offset into $args, which
          ;; parse_count checks.
          (local.set $count
            (call $parse_count (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args))))
          (br_if $usage (i32.eqz (local.get $count)))
          (local.set $kept (call $count_readings (local.get $count)))
          (br_if $clock_failed (i32.lt_s (local.get $kept) (i32.const 0)))
          (br_if $write_failed (call $write_count (i64.extend_i32_u (local.get $kept))))
          (call $proc_exit (global.get $EXIT_OK))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
        (call $proc_exit (global.get $EXIT_USAGE))
        unreachable)
      (call $proc_exit (global.get $EXIT_CLOCK_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Parses the count that starts at byte $at of $args: one to MAX_DIGITS digits, ended by the
  ;; argument's NUL, from 1 to MAX_READINGS. Gives it, or 0 when the text isn't such a count.
  (func $parse_count (param $at i32) (result i32)
    (local $i i32)
    (local $c i32)
    (local $n i32)
    (local $digits i32)
    (local $result i32)
    (local $left i32)
    (if (i32.ge_u (local.get $at) (global.get $args.size))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (local.set $i (local.get $at))
    (block $invalid
      (block $end
        (local.set $left (global.get $SCAN))
        (loop $digit  ;; bound: 8
          (if (i32.ge_u (local.get $i) (global.get $args.size))
            (then (call $assert_fail (i32.const 4)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $i))))
          (br_if $end (i32.eqz (local.get $c)))
          (br_if $invalid (i32.ge_u (local.get $digits) (global.get $MAX_DIGITS)))
          ;; Below '0' wraps to a large number, so one unsigned test catches both sides.
          (local.set $c (i32.sub (local.get $c) (global.get $ZERO)))
          (br_if $invalid (i32.gt_u (local.get $c) (global.get $NINE)))
          (local.set $n (i32.add (i32.mul (local.get $n) (global.get $BASE)) (local.get $c)))
          (local.set $digits (i32.add (local.get $digits) (i32.const 1)))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $digit (local.get $left)))
        ;; More characters than a count has.
        (br $invalid))
      ;; The NUL: a count has a digit at least, and is from 1 to MAX_READINGS.
      (br_if $invalid (i32.eqz (local.get $n)))
      (br_if $invalid (i32.gt_u (local.get $n) (global.get $MAX_READINGS)))
      (local.set $result (local.get $n)))
    (local.get $result))

  ;; Reads the clock that only moves forward $count times, at most MAX_READINGS, and gives how
  ;; many readings were not earlier than the previous one, or FAILED when the clock can't be read.
  ;; The first reading is never earlier: no reading comes before it.
  (func $count_readings (param $count i32) (result i32)
    (local $err i32)
    (local $now i64)
    (local $before i64)
    (local $taken i32)
    (local $kept i32)
    (local $result i32)
    (local $left i32)
    (if (i32.gt_u (local.get $count) (global.get $MAX_READINGS))
      (then (call $assert_fail (i32.const 5)) unreachable))
    (local.set $result (global.get $FAILED))
    (block $failed
      (block $done
        (local.set $left (global.get $MAX_READINGS))
        (loop $reading  ;; bound: 1000000
          (br_if $done (i32.ge_u (local.get $taken) (local.get $count)))
          (local.set $err
            (call $clock_time_get (global.get $MONOTONIC) (global.get $PRECISION)
              (global.get $time)))
          (br_if $failed (local.get $err))
          (local.set $now (i64.load (global.get $time)))
          (local.set $kept
            (i32.add (local.get $kept) (i64.ge_u (local.get $now) (local.get $before))))
          (local.set $before (local.get $now))
          (local.set $taken (i32.add (local.get $taken) (i32.const 1)))
          ;; A reading counts once at most.
          (if (i32.gt_u (local.get $kept) (local.get $taken))
            (then (call $assert_fail (i32.const 6)) unreachable))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $reading (local.get $left))))
      (local.set $result (local.get $kept)))
    (local.get $result))

  ;; Writes $count in decimal, then a newline, to standard output: the newline last, the digits
  ;; before it from the last. Gives 0, or an error number.
  (func $write_count (param $count i64) (result i32)  ;; returns: result
    (local $rest i64)
    (local $at i32)
    (local $left i32)
    (local.set $rest (local.get $count))
    (local.set $at (global.get $NEWLINE_AT))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (block $written
      (local.set $left (global.get $PRINT_DIGITS))
      (loop $digit  ;; bound: 20
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
      ;; 20 digits are all a 64-bit count has.
      (call $assert_fail (i32.const 8))
      unreachable)
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
