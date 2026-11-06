;; path_create_directory inside a file, as if it were a folder, gives NOTDIR, 54.
(module
  (import "wasi_snapshot_preview1" "path_create_directory"
    (func $path_create_directory (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "f.txt/sub")
  (func $main (export "_start")
    (call $proc_exit
      (call $path_create_directory (i32.const 3) (i32.const 2048) (i32.const 9)))))
