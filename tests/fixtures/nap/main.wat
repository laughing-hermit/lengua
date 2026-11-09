;; nap: sleeps for half a minute and then says that it is awake. The WebAssembly port of the
;; native nap, in the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; lengua's library gives $assert_fail and $write_all. It waits with poll_oneoff on one time, 30
;; seconds on the clock that only moves forward; in a test, the wait ends at once.
(module
  (import "wasi_snapshot_preview1" "poll_oneoff"
    (func $poll_oneoff (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_SLEEP_FAILED i32 (i32.const 2))
  (global $EXIT_WRITE_FAILED i32 (i32.const 3))
  ;; poll_oneoff's records: a subscription of 48 bytes, its own number first, its kind at 8, 0 for
  ;; a time, then the clock at 16, the wait in nanoseconds at 24, the precision at 32 and the
  ;; flags at 40, 0 for a wait from now; an event of 32 bytes, its subscription's number first
  ;; and its error at 8.
  (global $ONE_SUBSCRIPTION i32 (i32.const 1))
  (global $NAP_NUMBER i64 (i64.const 1))
  (global $CLOCK_TIME i32 (i32.const 0))
  (global $MONOTONIC i32 (i32.const 1))
  (global $NAP_NANOSECONDS i64 (i64.const 30000000000))
  (global $PRECISION i64 (i64.const 1000))

  ;; Regions (rule 3, M1), from 1024 up: the message, read-only, then what poll_oneoff waits on,
  ;; what it says happened, and how many events it wrote.
  (global $awake i32 (i32.const 1024))
  (global $awake.rosize i32 (i32.const 6))
  (data (i32.const 1024) "awake\0a")
  (global $subscription i32 (i32.const 1040))
  (global $subscription.size i32 (i32.const 48))
  (global $event i32 (i32.const 1088))
  (global $event.size i32 (i32.const 32))
  (global $happened i32 (i32.const 1120))
  (global $happened.size i32 (i32.const 4))

  ;; Naps for 30 seconds, then says it's awake. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (block $write_failed
      (block $sleep_failed
        ;; One wait: NAP_NANOSECONDS from now, on the clock that only moves forward.
        (i64.store (global.get $subscription) (global.get $NAP_NUMBER))
        (i32.store8 offset=8 (global.get $subscription) (global.get $CLOCK_TIME))
        (i32.store offset=16 (global.get $subscription) (global.get $MONOTONIC))
        (i64.store offset=24 (global.get $subscription) (global.get $NAP_NANOSECONDS))
        (i64.store offset=32 (global.get $subscription) (global.get $PRECISION))
        (local.set $err
          (call $poll_oneoff (global.get $subscription) (global.get $event)
            (global.get $ONE_SUBSCRIPTION) (global.get $happened)))
        (br_if $sleep_failed (local.get $err))
        ;; poll_oneoff writes one event a subscription that happened, and this one's is the nap's.
        (if (i32.ne (i32.load (global.get $happened)) (global.get $ONE_SUBSCRIPTION))
          (then (call $assert_fail (i32.const 1)) unreachable))
        (if (i64.ne (i64.load (global.get $event)) (global.get $NAP_NUMBER))
          (then (call $assert_fail (i32.const 2)) unreachable))
        ;; The event's error: a wait that failed.
        (br_if $sleep_failed (i32.load16_u offset=8 (global.get $event)))
        (br_if $write_failed
          (call $write_all (global.get $STDOUT) (global.get $awake) (global.get $awake.rosize)))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_SLEEP_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)
)
