;; fd_prestat_dir_name with a buffer shorter than the name gives NAMETOOLONG, 37.
(module
  (import "wasi_snapshot_preview1" "fd_prestat_dir_name"
    (func $fd_prestat_dir_name (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (func $main (export "_start")
    (call $proc_exit
      (call $fd_prestat_dir_name (i32.const 3) (i32.const 4096) (i32.const 0)))))
