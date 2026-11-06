;; fd_prestat_get of descriptor 4, past the one preopen, gives BADF, 8: where wasi-libc stops
;; looking for preopens.
(module
  (import "wasi_snapshot_preview1" "fd_prestat_get"
    (func $fd_prestat_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (func $main (export "_start")
    (call $proc_exit (call $fd_prestat_get (i32.const 4) (i32.const 1152)))))
