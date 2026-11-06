;; path_unlink_file of a path that climbs out of the working folder gives NOTCAPABLE, 76.
(module
  (import "wasi_snapshot_preview1" "path_unlink_file"
    (func $path_unlink_file (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "../f.txt")
  (func $main (export "_start")
    (call $proc_exit
      (call $path_unlink_file (i32.const 3) (i32.const 2048) (i32.const 8)))))
