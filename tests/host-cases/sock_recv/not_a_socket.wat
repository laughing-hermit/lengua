;; sock_recv on standard output, which isn't a socket, gives NOTSOCK, 57.
(module
  (import "wasi_snapshot_preview1" "sock_recv"
    (func $sock_recv (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  ;; One iovec over 64 bytes at 4096.
  (data (i32.const 3072) "\00\10\00\00\40\00\00\00")
  (func $main (export "_start")
    (call $proc_exit
      (call $sock_recv (i32.const 1) (i32.const 3072) (i32.const 1) (i32.const 0)
        (i32.const 1064) (i32.const 1068)))))
