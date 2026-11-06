;; path_create_directory of a device name one system reserves in every folder, here "com1",
;; gives INVAL, 28, on every system.
(module
  (import "wasi_snapshot_preview1" "path_create_directory"
    (func $path_create_directory (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "com1")
  (func $main (export "_start")
    (call $proc_exit
      (call $path_create_directory (i32.const 3) (i32.const 2048) (i32.const 4)))))
