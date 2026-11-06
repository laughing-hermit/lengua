;; sock_accept on standard output, which isn't a socket, gives NOTSOCK, 57.
(module
  (import "wasi_snapshot_preview1" "sock_accept"
    (func $sock_accept (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (func $main (export "_start")
    (call $proc_exit (call $sock_accept (i32.const 1) (i32.const 0) (i32.const 1064)))))
