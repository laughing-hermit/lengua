;; environ_get gives exactly the variables the test sets, each as NAME=value with its NUL; a
;; value may hold an equals sign.
(module
  (import "wasi_snapshot_preview1" "environ_sizes_get"
    (func $environ_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "environ_get"
    (func $environ_get (param i32 i32) (result i32)))
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
  ;; Writes a newline to standard output.
  (func $nl (call $say (i32.const 2041) (i32.const 1)))
  ;; The length of the NUL-terminated string at $at.
  (func $strlen (param $at i32) (result i32)
    (local $n i32)
    (block $end
      (loop $byte
        (br_if $end
          (i32.eqz (i32.load8_u (i32.add (local.get $at) (local.get $n)))))
        (local.set $n (i32.add (local.get $n) (i32.const 1)))
        (br $byte)))
    (local.get $n))
  ;; Writes the NUL-terminated string at $at to standard output.
  (func $say_str (param $at i32)
    (call $say (local.get $at) (call $strlen (local.get $at))))
  (func $main (export "_start")
    (local $i i32)
    (call $need (call $environ_sizes_get (i32.const 1064) (i32.const 1068)))
    (call $need (call $environ_get (i32.const 4096) (i32.const 8192)))
    ;; Every variable the host gives, one to a line.
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (i32.load (i32.const 1064))))
        (call $say_str
          (i32.load (i32.add (i32.const 4096) (i32.shl (local.get $i) (i32.const 2)))))
        (call $nl)
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (call $proc_exit (i32.const 0))))
