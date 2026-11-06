;; fd_readdir with a buffer too small for the whole listing fills it, cutting the last entry
;; short, and goes on from a cookie: five entries read 40 bytes at a time are each seen once.
(module
  (import "wasi_snapshot_preview1" "fd_readdir"
    (func $fd_readdir (param i32 i32 i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2040) " \n")
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
  ;; Writes $n to standard output in decimal.
  (func $say_num (param $n i64)
    (local $at i32)
    (local.set $at (i32.const 1112))
    (loop $digit
      (local.set $at (i32.sub (local.get $at) (i32.const 1)))
      (i32.store8 (local.get $at)
        (i32.add (i32.const 48)
          (i32.wrap_i64 (i64.rem_u (local.get $n) (i64.const 10)))))
      (local.set $n (i64.div_u (local.get $n) (i64.const 10)))
      (br_if $digit (i64.ne (local.get $n) (i64.const 0))))
    (call $say (local.get $at) (i32.sub (i32.const 1112) (local.get $at))))
  ;; Writes a newline to standard output.
  (func $nl (call $say (i32.const 2041) (i32.const 1)))
  ;; Reads the whole listing of the folder $fd with a $cap-byte buffer at 4096, following the
  ;; cookies as wasi-libc does: the entries at 1112, the files among them at 1116, the folders
  ;; at 1120.
  (func $list (param $fd i32) (param $cap i32)
    (local $cookie i64)
    (local $used i32)
    (local $at i32)
    (local $size i32)
    (local $type i32)
    (local $whole i32)
    (i32.store (i32.const 1112) (i32.const 0))
    (i32.store (i32.const 1116) (i32.const 0))
    (i32.store (i32.const 1120) (i32.const 0))
    (block $end
      (loop $call
        (i32.store (i32.const 1064) (i32.const 0))
        (call $need (call $fd_readdir (local.get $fd) (i32.const 4096) (local.get $cap)
          (local.get $cookie) (i32.const 1064)))
        (local.set $used (i32.load (i32.const 1064)))
        (if (i32.gt_u (local.get $used) (local.get $cap))
          (then (call $proc_exit (i32.const 103))))
        (local.set $at (i32.const 0))
        (local.set $whole (i32.const 0))
        (block $parsed
          (loop $entry
            ;; An entry counts when its 24-byte head and its name are both in the buffer.
            (br_if $parsed
              (i32.gt_u (i32.add (local.get $at) (i32.const 24)) (local.get $used)))
            (local.set $size
              (i32.add (i32.const 24) (i32.load offset=4112 (local.get $at))))
            (br_if $parsed
              (i32.gt_u (i32.add (local.get $at) (local.get $size)) (local.get $used)))
            (local.set $cookie (i64.load offset=4096 (local.get $at)))
            (local.set $type (i32.load8_u offset=4116 (local.get $at)))
            (i32.store (i32.const 1112) (i32.add (i32.load (i32.const 1112)) (i32.const 1)))
            (if (i32.eq (local.get $type) (i32.const 4))
              (then (i32.store (i32.const 1116)
                (i32.add (i32.load (i32.const 1116)) (i32.const 1)))))
            (if (i32.eq (local.get $type) (i32.const 3))
              (then (i32.store (i32.const 1120)
                (i32.add (i32.load (i32.const 1120)) (i32.const 1)))))
            (local.set $at (i32.add (local.get $at) (local.get $size)))
            (local.set $whole (i32.const 1))
            (br $entry)))
        ;; A buffer the host didn't fill held the listing's end.
        (br_if $end (i32.lt_u (local.get $used) (local.get $cap)))
        ;; A full buffer without one whole entry can't go on.
        (if (i32.eqz (local.get $whole))
          (then (call $proc_exit (i32.const 99))))
        (br $call))))
  (func $main (export "_start")
    ;; Each entry is 26 bytes: a 24-byte head and a two-byte name.
    (call $list (i32.const 3) (i32.const 40))
    (call $say_num (i64.extend_i32_u (i32.load (i32.const 1112))))
    (call $nl)
    (call $proc_exit (i32.const 0))))
