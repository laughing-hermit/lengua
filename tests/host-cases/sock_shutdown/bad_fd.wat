;; sock_shutdown of a descriptor not in use gives BADF, 8.
(module
  (import "wasi_snapshot_preview1" "sock_shutdown"
    (func $sock_shutdown (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (func $main (export "_start")
    ;; How: wr, 2.
    (call $proc_exit (call $sock_shutdown (i32.const 9) (i32.const 2)))))
