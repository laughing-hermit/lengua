;; path_open of a folder with the right to write gives ISDIR, 31.
(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "dir")
  (func $main (export "_start")
    ;; No oflags, the rights to write.
    (call $proc_exit
      (call $path_open (i32.const 3) (i32.const 0) (i32.const 2048) (i32.const 3)
        (i32.const 0) (i64.const 2097252) (i64.const 0) (i32.const 0) (i32.const 1056)))))
