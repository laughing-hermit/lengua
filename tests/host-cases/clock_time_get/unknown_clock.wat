;; clock_time_get of a clock that isn't one gives INVAL, 28.
(module
  (import "wasi_snapshot_preview1" "clock_time_get"
    (func $clock_time_get (param i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (func $main (export "_start")
    (call $proc_exit
      (call $clock_time_get (i32.const 7) (i64.const 1) (i32.const 1072)))))
