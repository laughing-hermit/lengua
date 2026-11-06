;; lengua's library for WebAssembly: the functions a main.wat may call without defining them.
;; src/wasm/text.rs merges into a module the functions its main.wat calls and doesn't define,
;; with what they need, after main.wat's own functions, so main.wat's code keeps its places.
;;
;; The library's imports carry names of its own, $lengua.*, so they never meet main.wat's; a
;; module may import the same function twice. Bytes 0 to 15 of memory are the library's while a
;; call of it lasts: an iovec at 0 and a count at 8. A buffer given to $fill or $write_all starts
;; at 16 or later, or the call fails an assertion. $assert_fail, which ends the program, writes
;; its message at 16 as well.
(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $lengua.fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_read"
    (func $lengua.fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $lengua.proc_exit (param i32)))

  ;; $assert_fail(n): writes "lengua: assertion NNN failed\n" to standard error, NNN being the
  ;; assertion's number, 1 to 999, and ends the program with code 134.
  (func $assert_fail (param $n i32)
    ;; A module with no memory yet can't print: it only ends.
    (if (i32.eqz (memory.size))
      (then (call $lengua.proc_exit (i32.const 134))))
    (memory.init $lengua.assertion (i32.const 16) (i32.const 0) (i32.const 29))
    (i32.store8 (i32.const 34)
      (i32.add (i32.const 48)
        (i32.rem_u (i32.div_u (local.get $n) (i32.const 100)) (i32.const 10))))
    (i32.store8 (i32.const 35)
      (i32.add (i32.const 48)
        (i32.rem_u (i32.div_u (local.get $n) (i32.const 10)) (i32.const 10))))
    (i32.store8 (i32.const 36)
      (i32.add (i32.const 48) (i32.rem_u (local.get $n) (i32.const 10))))
    (i32.store (i32.const 0) (i32.const 16))
    (i32.store (i32.const 4) (i32.const 29))
    (drop (call $lengua.fd_write (i32.const 2) (i32.const 0) (i32.const 1) (i32.const 8)))
    (call $lengua.proc_exit (i32.const 134))
    (unreachable))

  ;; $fill(fd, ptr, cap, stop): reads from fd into the cap bytes at ptr until they are full, the
  ;; input ends, or the last byte read is stop (a byte, 0 to 255; -1 reads to the end). However
  ;; the input arrives, it reads the same. Gives how many bytes it read, or the negative of a
  ;; WASI error. Assertions: 990, a buffer in the library's bytes; 991, no room; 992, a read of
  ;; more than was asked.
  (func $fill (param $fd i32) (param $ptr i32) (param $cap i32) (param $stop i32) (result i32)
    (local $got i32)
    (local $n i32)
    (local $errno i32)
    (if (i32.lt_u (local.get $ptr) (i32.const 16))
      (then (call $assert_fail (i32.const 990))))
    (if (i32.eqz (local.get $cap))
      (then (call $assert_fail (i32.const 991))))
    (block $done
      (loop $more
        (i32.store (i32.const 0) (i32.add (local.get $ptr) (local.get $got)))
        (i32.store (i32.const 4) (i32.sub (local.get $cap) (local.get $got)))
        (local.set $errno
          (call $lengua.fd_read (local.get $fd) (i32.const 0) (i32.const 1) (i32.const 8)))
        (if (local.get $errno)
          (then (return (i32.sub (i32.const 0) (local.get $errno)))))
        (local.set $n (i32.load (i32.const 8)))
        ;; The end of the input.
        (br_if $done (i32.eqz (local.get $n)))
        (if (i32.gt_u (local.get $n) (i32.sub (local.get $cap) (local.get $got)))
          (then (call $assert_fail (i32.const 992))))
        (local.set $got (i32.add (local.get $got) (local.get $n)))
        (br_if $done (i32.eq (local.get $got) (local.get $cap)))
        (br_if $done
          (i32.eq
            (i32.load8_u (i32.sub (i32.add (local.get $ptr) (local.get $got)) (i32.const 1)))
            (local.get $stop)))
        (br $more)))
    (local.get $got))

  ;; $write_all(fd, ptr, len): writes the len bytes at ptr to fd, however many each write takes.
  ;; Gives 0, or the negative of a WASI error. Assertion 993: a buffer in the library's bytes.
  (func $write_all (param $fd i32) (param $ptr i32) (param $len i32) (result i32)
    (local $n i32)
    (local $errno i32)
    (if (i32.lt_u (local.get $ptr) (i32.const 16))
      (then (call $assert_fail (i32.const 993))))
    (block $done
      (loop $more
        (br_if $done (i32.eqz (local.get $len)))
        (i32.store (i32.const 0) (local.get $ptr))
        (i32.store (i32.const 4) (local.get $len))
        (local.set $errno
          (call $lengua.fd_write (local.get $fd) (i32.const 0) (i32.const 1) (i32.const 8)))
        (if (local.get $errno)
          (then (return (i32.sub (i32.const 0) (local.get $errno)))))
        (local.set $n (i32.load (i32.const 8)))
        ;; A write that takes nothing would never end: an input or output error.
        (if (i32.eqz (local.get $n))
          (then (return (i32.const -29))))
        (local.set $ptr (i32.add (local.get $ptr) (local.get $n)))
        (local.set $len (i32.sub (local.get $len) (local.get $n)))
        (br $more)))
    (i32.const 0))

  (data $lengua.assertion "lengua: assertion 000 failed\n"))
