;; fd_fdstat_get of a descriptor not in use gives BADF, 8.
(module
  (import "wasi_snapshot_preview1" "fd_fdstat_get"
    (func $fd_fdstat_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (func $main (export "_start")
    (call $proc_exit (call $fd_fdstat_get (i32.const 9) (i32.const 1152)))))
