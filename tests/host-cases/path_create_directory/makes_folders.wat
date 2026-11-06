;; path_create_directory makes a folder, and then a folder inside it.
(module
  (import "wasi_snapshot_preview1" "path_create_directory"
    (func $path_create_directory (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  ;; "new/sub"; its first three bytes are "new".
  (data (i32.const 2048) "new/sub")
  ;; Exits with 150 plus the errno when a step that must succeed fails.
  (func $need (param $errno i32)
    (if (local.get $errno)
      (then (call $proc_exit (i32.add (i32.const 150) (local.get $errno))))))
  (func $main (export "_start")
    (call $need (call $path_create_directory (i32.const 3) (i32.const 2048) (i32.const 3)))
    (call $need (call $path_create_directory (i32.const 3) (i32.const 2048) (i32.const 7)))
    (call $proc_exit (i32.const 0))))
