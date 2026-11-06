;; poll_oneoff with no subscriptions, which would wait for nothing, gives INVAL, 28.
(module
  (import "wasi_snapshot_preview1" "poll_oneoff"
    (func $poll_oneoff (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (func $main (export "_start")
    (call $proc_exit
      (call $poll_oneoff (i32.const 1280) (i32.const 1216) (i32.const 0) (i32.const 1064)))))
