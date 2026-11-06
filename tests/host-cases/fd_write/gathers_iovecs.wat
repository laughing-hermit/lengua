;; fd_write gathers its iovecs in order, an empty one among them, and says how many bytes it
;; took; descriptor 2 is standard error.
(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2048) "abcd\n")
  (data (i32.const 2056) "oops\n")
  ;; Three iovecs over "abcd\n" at 2048: "ab", nothing, then "cd\n".
  (data (i32.const 3072)
    "\00\08\00\00\02\00\00\00"
    "\02\08\00\00\00\00\00\00"
    "\02\08\00\00\03\00\00\00")
  ;; One iovec over "oops\n" at 2056.
  (data (i32.const 3096) "\08\08\00\00\05\00\00\00")
  ;; Exits with 150 plus the errno when a step that must succeed fails.
  (func $need (param $errno i32)
    (if (local.get $errno)
      (then (call $proc_exit (i32.add (i32.const 150) (local.get $errno))))))
  ;; Writes $len bytes at $at to $fd, all of them, in as many calls as it takes.
  (func $put (param $fd i32) (param $at i32) (param $len i32)
    (local $n i32)
    (block $done
      (loop $more
        (br_if $done (i32.eqz (local.get $len)))
        (i32.store (i32.const 1024) (local.get $at))
        (i32.store (i32.const 1028) (local.get $len))
        (call $need
          (call $fd_write (local.get $fd) (i32.const 1024) (i32.const 1) (i32.const 1032)))
        (local.set $n (i32.load (i32.const 1032)))
        (if (i32.eqz (local.get $n))
          (then (call $proc_exit (i32.const 102))))
        (if (i32.gt_u (local.get $n) (local.get $len))
          (then (call $proc_exit (i32.const 103))))
        (local.set $at (i32.add (local.get $at) (local.get $n)))
        (local.set $len (i32.sub (local.get $len) (local.get $n)))
        (br $more))))
  (func $main (export "_start")
    (local $n i32)
    (i32.store (i32.const 1064) (i32.const 99))
    (call $need
      (call $fd_write (i32.const 1) (i32.const 3072) (i32.const 3) (i32.const 1064)))
    ;; A write may take fewer bytes than it was given: the rest follow from where it stopped,
    ;; so the output is whole only when the count it gave is true.
    (local.set $n (i32.load (i32.const 1064)))
    (if (i32.gt_u (local.get $n) (i32.const 5))
      (then (call $proc_exit (i32.const 103))))
    (call $put (i32.const 1) (i32.add (i32.const 2048) (local.get $n))
      (i32.sub (i32.const 5) (local.get $n)))
    (i32.store (i32.const 1064) (i32.const 99))
    (call $need
      (call $fd_write (i32.const 2) (i32.const 3096) (i32.const 1) (i32.const 1064)))
    (local.set $n (i32.load (i32.const 1064)))
    (if (i32.gt_u (local.get $n) (i32.const 5))
      (then (call $proc_exit (i32.const 103))))
    (call $put (i32.const 2) (i32.add (i32.const 2056) (local.get $n))
      (i32.sub (i32.const 5) (local.get $n)))
    (call $proc_exit (i32.const 0))))
