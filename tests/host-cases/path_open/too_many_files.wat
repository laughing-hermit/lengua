;; path_open past the most files a program may hold open gives MFILE, 33.
(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "f.txt")
  (func $main (export "_start")
    (local $i i32)
    (local $errno i32)
    ;; f.txt opened again and again, every descriptor kept, until a call fails.
    (block $done
      (loop $next
        (local.set $errno
          (call $path_open (i32.const 3) (i32.const 0) (i32.const 2048) (i32.const 5)
            (i32.const 0) (i64.const 2113574) (i64.const 0) (i32.const 0) (i32.const 1056)))
        (br_if $done (local.get $errno))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br_if $next (i32.lt_u (local.get $i) (i32.const 100000)))))
    ;; The errno that ended it, or 99 when none did.
    (if (i32.eqz (local.get $errno))
      (then (call $proc_exit (i32.const 99))))
    (call $proc_exit (local.get $errno))))
