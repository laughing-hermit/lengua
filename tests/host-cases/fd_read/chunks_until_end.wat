;; fd_read hands over standard input byte for byte, whatever the bytes are, in reads of at
;; most four bytes, until it returns 0.
(module
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
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
  ;; Writes $len bytes at $at to standard output.
  (func $say (param $at i32) (param $len i32)
    (call $put (i32.const 1) (local.get $at) (local.get $len)))
  ;; Reads up to $cap bytes from $fd into $at: the errno, and the count at 1064.
  (func $read (param $fd i32) (param $at i32) (param $cap i32) (result i32)
    (i32.store (i32.const 1040) (local.get $at))
    (i32.store (i32.const 1044) (local.get $cap))
    (call $fd_read (local.get $fd) (i32.const 1040) (i32.const 1) (i32.const 1064)))
  (func $main (export "_start")
    (block $end
      (loop $next
        (call $need (call $read (i32.const 0) (i32.const 4096) (i32.const 4)))
        (br_if $end (i32.eqz (i32.load (i32.const 1064))))
        (if (i32.gt_u (i32.load (i32.const 1064)) (i32.const 4))
          (then (call $proc_exit (i32.const 103))))
        (call $say (i32.const 4096) (i32.load (i32.const 1064)))
        (br $next)))
    (call $proc_exit (i32.const 0))))
