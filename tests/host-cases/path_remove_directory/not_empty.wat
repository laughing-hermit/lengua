;; path_remove_directory of a folder with a file in it gives NOTEMPTY, 55, and removes nothing.
(module
  (import "wasi_snapshot_preview1" "path_remove_directory"
    (func $path_remove_directory (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "dir")
  (func $main (export "_start")
    (call $proc_exit
      (call $path_remove_directory (i32.const 3) (i32.const 2048) (i32.const 3)))))
