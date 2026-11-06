;; path_rename to a name that differs only in case from another file in the folder gives
;; EXIST, 20, on every system.
(module
  (import "wasi_snapshot_preview1" "path_rename"
    (func $path_rename (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "a.txt")
  (data (i32.const 2056) "OTHER.TXT")
  (func $main (export "_start")
    (call $proc_exit
      (call $path_rename (i32.const 3) (i32.const 2048) (i32.const 5)
        (i32.const 3) (i32.const 2056) (i32.const 9)))))
