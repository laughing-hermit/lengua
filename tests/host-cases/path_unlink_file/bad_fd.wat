;; path_unlink_file beneath a descriptor not in use gives BADF, 8.
(module
  (import "wasi_snapshot_preview1" "path_unlink_file"
    (func $path_unlink_file (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "f.txt")
  (func $main (export "_start")
    (call $proc_exit
      (call $path_unlink_file (i32.const 9) (i32.const 2048) (i32.const 5)))))
