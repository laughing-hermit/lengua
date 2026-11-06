;; fd_read from a descriptor not in use gives BADF, 8.
(module
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  ;; One iovec over 64 bytes at 4096.
  (data (i32.const 3072) "\00\10\00\00\40\00\00\00")
  (func $main (export "_start")
    (call $proc_exit
      (call $fd_read (i32.const 9) (i32.const 3072) (i32.const 1) (i32.const 1064)))))
