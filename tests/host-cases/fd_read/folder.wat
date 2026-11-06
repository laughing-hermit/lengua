;; fd_read from descriptor 3, the working folder, gives ISDIR, 31: a folder is listed, not
;; read.
(module
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  ;; One iovec over 64 bytes at 4096.
  (data (i32.const 3072) "\00\10\00\00\40\00\00\00")
  (func $main (export "_start")
    (call $proc_exit
      (call $fd_read (i32.const 3) (i32.const 3072) (i32.const 1) (i32.const 1064)))))
