;; path_rename of a folder onto a folder that holds a file gives NOTEMPTY, 55.
(module
  (import "wasi_snapshot_preview1" "path_rename"
    (func $path_rename (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "a")
  (data (i32.const 2056) "b")
  (func $main (export "_start")
    (call $proc_exit
      (call $path_rename (i32.const 3) (i32.const 2048) (i32.const 1)
        (i32.const 3) (i32.const 2056) (i32.const 1)))))
