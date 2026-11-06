;; path_create_directory of a path of 1,024 bytes, past the 1,023 every system takes, gives
;; NAMETOOLONG, 37, though each name in it is short.
(module
  (import "wasi_snapshot_preview1" "path_create_directory"
    (func $path_create_directory (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (func $main (export "_start")
    (local $i i32)
    ;; At 4096: "a/" 511 times, then "aa", 1,024 bytes in all.
    (memory.fill (i32.const 4096) (i32.const 97) (i32.const 1024))
    (local.set $i (i32.const 1))
    (block $done
      (loop $next
        (br_if $done (i32.gt_u (local.get $i) (i32.const 1021)))
        (i32.store8 (i32.add (i32.const 4096) (local.get $i)) (i32.const 47))
        (local.set $i (i32.add (local.get $i) (i32.const 2)))
        (br $next)))
    (call $proc_exit
      (call $path_create_directory (i32.const 3) (i32.const 4096) (i32.const 1024)))))
