;; fd_readdir of a descriptor not in use gives BADF, 8.
(module
  (import "wasi_snapshot_preview1" "fd_readdir"
    (func $fd_readdir (param i32 i32 i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (func $main (export "_start")
    (call $proc_exit
      (call $fd_readdir (i32.const 9) (i32.const 4096) (i32.const 4096) (i64.const 0)
        (i32.const 1064)))))
