;; path_remove_directory removes an empty folder.
(module
  (import "wasi_snapshot_preview1" "path_remove_directory"
    (func $path_remove_directory (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "empty")
  ;; Exits with 150 plus the errno when a step that must succeed fails.
  (func $need (param $errno i32)
    (if (local.get $errno)
      (then (call $proc_exit (i32.add (i32.const 150) (local.get $errno))))))
  (func $main (export "_start")
    (call $need (call $path_remove_directory (i32.const 3) (i32.const 2048) (i32.const 5)))
    (call $proc_exit (i32.const 0))))
