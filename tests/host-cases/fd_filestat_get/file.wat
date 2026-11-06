;; fd_filestat_get gives an open file's type and size, and nothing else: every other field is 0.
(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_filestat_get"
    (func $fd_filestat_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 2040) " \n")
  (data (i32.const 2048) "f.txt")
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
  ;; Writes a space, or a newline, to standard output.
  (func $sp (call $say (i32.const 2040) (i32.const 1)))
  (func $nl (call $say (i32.const 2041) (i32.const 1)))
  ;; Opens the $len-byte path at $at beneath the working folder, descriptor 3: the errno, and
  ;; the new descriptor at 1056.
  (func $open (param $at i32) (param $len i32) (param $oflags i32) (param $rights i64)
    (param $fdflags i32) (result i32)
    (call $path_open (i32.const 3) (i32.const 0) (local.get $at) (local.get $len)
      (local.get $oflags) (local.get $rights) (i64.const 0) (local.get $fdflags)
      (i32.const 1056)))
  ;; Writes the filestat at $at as its type, its size, and 1 if any other field (dev, ino,
  ;; nlink, atim, mtim, ctim) is set or 0 if none is, then a newline.
  (func $say_stat (param $at i32)
    (call $say_num (i64.extend_i32_u (i32.load8_u offset=16 (local.get $at))))
    (call $sp)
    (call $say_num (i64.load offset=32 (local.get $at)))
    (call $sp)
    (call $say_num (i64.extend_i32_u (i64.ne (i64.const 0)
      (i64.or
        (i64.or (i64.or (i64.load (local.get $at)) (i64.load offset=8 (local.get $at)))
          (i64.or (i64.load offset=24 (local.get $at)) (i64.load offset=40 (local.get $at))))
        (i64.or (i64.load offset=48 (local.get $at)) (i64.load offset=56 (local.get $at)))))))
    (call $nl))
  (func $main (export "_start")
    (call $need (call $open (i32.const 2048) (i32.const 5) (i32.const 0)
      (i64.const 2113574) (i32.const 0)))
    (memory.fill (i32.const 1152) (i32.const 255) (i32.const 64))
    (call $need (call $fd_filestat_get (i32.load (i32.const 1056)) (i32.const 1152)))
    (call $say_stat (i32.const 1152))
    (call $proc_exit (i32.const 0))))
