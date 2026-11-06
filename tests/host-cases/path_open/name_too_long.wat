;; path_open making a name of 256 bytes, past every system's 255, gives NAMETOOLONG, 37.
(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (func $main (export "_start")
    ;; 256 bytes of "a" at 4096.
    (memory.fill (i32.const 4096) (i32.const 97) (i32.const 256))
    ;; creat, to write.
    (call $proc_exit
      (call $path_open (i32.const 3) (i32.const 0) (i32.const 4096) (i32.const 256)
        (i32.const 1) (i64.const 2097252) (i64.const 0) (i32.const 0) (i32.const 1056)))))
