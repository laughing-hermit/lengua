;; path_open of a file with the directory flag gives NOTDIR, 54.
(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "f.txt")
  (func $main (export "_start")
    ;; The directory oflag, the rights to read.
    (call $proc_exit
      (call $path_open (i32.const 3) (i32.const 0) (i32.const 2048) (i32.const 5)
        (i32.const 2) (i64.const 2113574) (i64.const 0) (i32.const 0) (i32.const 1056)))))
