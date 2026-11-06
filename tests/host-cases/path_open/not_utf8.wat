;; path_open making a name that isn't UTF-8 gives ILSEQ, 25.
(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  ;; The byte 0xff, then ".txt": five bytes.
  (data (i32.const 2048) "\ff.txt")
  (func $main (export "_start")
    ;; creat, to write.
    (call $proc_exit
      (call $path_open (i32.const 3) (i32.const 0) (i32.const 2048) (i32.const 5)
        (i32.const 1) (i64.const 2097252) (i64.const 0) (i32.const 0) (i32.const 1056)))))
