;; fd_tell on standard output, which has no places, gives SPIPE, 70.
(module
  (import "wasi_snapshot_preview1" "fd_tell"
    (func $fd_tell (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (func $main (export "_start")
    (call $proc_exit (call $fd_tell (i32.const 1) (i32.const 1072)))))
