;; poll_oneoff with one clock subscription, an hour long, ends at once in a test and gives one
;; event: the subscription's userdata, no error, the clock's type.
(module
  (import "wasi_snapshot_preview1" "poll_oneoff"
    (func $poll_oneoff (param i32 i32 i32 i32) (result i32)))
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
  ;; Writes a space, or a newline, to standard output.
  (func $sp (call $say (i32.const 2040) (i32.const 1)))
  (func $nl (call $say (i32.const 2041) (i32.const 1)))
  (func $main (export "_start")
    (memory.fill (i32.const 1216) (i32.const 255) (i32.const 32))
    ;; The subscription at 1280: userdata 42; the clock (tag 0); real time, an hour from now,
    ;; no precision, relative.
    (i64.store (i32.const 1280) (i64.const 42))
    (i32.store8 (i32.const 1288) (i32.const 0))
    (i32.store (i32.const 1296) (i32.const 0))
    (i64.store (i32.const 1304) (i64.const 3600000000000))
    (i64.store (i32.const 1312) (i64.const 0))
    (i32.store16 (i32.const 1320) (i32.const 0))
    (i32.store (i32.const 1064) (i32.const 99))
    (call $need
      (call $poll_oneoff (i32.const 1280) (i32.const 1216) (i32.const 1) (i32.const 1064)))
    ;; How many events, then the first one's userdata, error and type.
    (call $say_num (i64.extend_i32_u (i32.load (i32.const 1064))))
    (call $sp)
    (call $say_num (i64.load (i32.const 1216)))
    (call $sp)
    (call $say_num (i64.extend_i32_u (i32.load16_u (i32.const 1224))))
    (call $sp)
    (call $say_num (i64.extend_i32_u (i32.load8_u (i32.const 1226))))
    (call $nl)
    (call $proc_exit (i32.const 0))))
