;; utc: prints the current UTC time of day as HH:MM:SS. The WebAssembly counterpart of
;; tests/fixtures/clock, in the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; lengua's library gives $assert_fail and $write_all. The clock reads in nanoseconds, of which
;; the program keeps the whole seconds since the most recent midnight.
(module
  (import "wasi_snapshot_preview1" "clock_time_get"
    (func $clock_time_get (param i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors and clocks: the time of day, read to the microsecond.
  (global $STDOUT i32 (i32.const 1))
  (global $REALTIME i32 (i32.const 0))
  (global $PRECISION i64 (i64.const 1000))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_WRITE_FAILED i32 (i32.const 2))
  (global $EXIT_CLOCK_FAILED i32 (i32.const 3))
  ;; Time.
  (global $NANOSECONDS i64 (i64.const 1000000000))
  (global $SECONDS_PER_DAY i64 (i64.const 86400))
  (global $SECONDS_PER_HOUR i64 (i64.const 3600))
  (global $SECONDS_PER_MINUTE i64 (i64.const 60))
  (global $HOURS_PER_DAY i64 (i64.const 24))
  (global $MINUTES_PER_HOUR i64 (i64.const 60))
  (global $HUNDRED i64 (i64.const 100))
  (global $TEN i64 (i64.const 10))
  ;; The line, HH:MM:SS and a newline: where the minutes, the seconds, each colon and the newline
  ;; go; the hours are at its start.
  (global $FIRST_COLON_AT i32 (i32.const 2))
  (global $MINUTES_AT i32 (i32.const 3))
  (global $SECOND_COLON_AT i32 (i32.const 5))
  (global $SECONDS_AT i32 (i32.const 6))
  (global $NEWLINE_AT i32 (i32.const 8))
  ;; Characters.
  (global $ZERO i32 (i32.const 48))
  (global $COLON i32 (i32.const 58))
  (global $NEWLINE i32 (i32.const 10))

  ;; Regions (rule 3, M1), from 1024 up: what the clock writes, then the line printed, which main
  ;; lends put_two a part at a time.
  (global $time i32 (i32.const 1024))
  (global $time.size i32 (i32.const 8))
  (global $line i32 (i32.const 1040))
  (global $line.size i32 (i32.const 9))

  ;; Reads the clock and prints the time of day in UTC. Never returns: every path ends in
  ;; proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $day i64)
    (local $hours i64)
    (local $minutes i64)
    (block $write_failed
      (block $clock_failed
        (local.set $err
          (call $clock_time_get (global.get $REALTIME) (global.get $PRECISION)
            (global.get $time)))
        (br_if $clock_failed (local.get $err))
        ;; The whole seconds since the most recent UTC midnight.
        (local.set $day
          (i64.rem_u (i64.div_u (i64.load (global.get $time)) (global.get $NANOSECONDS))
            (global.get $SECONDS_PER_DAY)))
        (local.set $hours (i64.div_u (local.get $day) (global.get $SECONDS_PER_HOUR)))
        (if (i64.ge_u (local.get $hours) (global.get $HOURS_PER_DAY))
          (then (call $assert_fail (i32.const 1)) unreachable))
        (local.set $minutes
          (i64.div_u (i64.rem_u (local.get $day) (global.get $SECONDS_PER_HOUR))
            (global.get $SECONDS_PER_MINUTE)))
        (if (i64.ge_u (local.get $minutes) (global.get $MINUTES_PER_HOUR))
          (then (call $assert_fail (i32.const 2)) unreachable))
        (call $put_two (local.get $hours) (global.get $line))
        (i32.store8 (i32.add (global.get $line) (global.get $FIRST_COLON_AT)) (global.get $COLON))
        (call $put_two (local.get $minutes) (i32.add (global.get $line) (global.get $MINUTES_AT)))
        (i32.store8 (i32.add (global.get $line) (global.get $SECOND_COLON_AT)) (global.get $COLON))
        (call $put_two (i64.rem_u (local.get $day) (global.get $SECONDS_PER_MINUTE))
          (i32.add (global.get $line) (global.get $SECONDS_AT)))
        (i32.store8 (i32.add (global.get $line) (global.get $NEWLINE_AT)) (global.get $NEWLINE))
        (br_if $write_failed
          (call $write_all (global.get $STDOUT) (global.get $line) (global.get $line.size)))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_CLOCK_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Writes $value, from 0 to 99, as two decimal digits at $at, which main lends it: the tens,
  ;; then the ones, so a value below ten has a zero first.
  (func $put_two (param $value i64) (param $at i32)
    (local $tens i64)
    (if (i64.ge_u (local.get $value) (global.get $HUNDRED))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (local.set $tens (i64.div_u (local.get $value) (global.get $TEN)))
    (if (i64.ge_u (local.get $tens) (global.get $TEN))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (i32.store8 (local.get $at) (i32.add (global.get $ZERO) (i32.wrap_i64 (local.get $tens))))
    (i32.store8 offset=1 (local.get $at)
      (i32.add (global.get $ZERO)
        (i32.wrap_i64 (i64.rem_u (local.get $value) (global.get $TEN))))))
)
