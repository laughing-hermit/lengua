;; fd_write to a descriptor not in use gives BADF, 8.
(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "x")
  ;; One iovec over "x" at 2048.
  (data (i32.const 3072) "\00\08\00\00\01\00\00\00")
  (func $main (export "_start")
    (call $proc_exit
      (call $fd_write (i32.const 9) (i32.const 3072) (i32.const 1) (i32.const 1064)))))
