;; path_create_directory inside a folder that isn't there gives NOENT, 44.
(module
  (import "wasi_snapshot_preview1" "path_create_directory"
    (func $path_create_directory (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "nope/sub")
  (func $main (export "_start")
    (call $proc_exit
      (call $path_create_directory (i32.const 3) (i32.const 2048) (i32.const 8)))))
