;; upcase-stdin: copies standard input of any length to standard output with every ASCII letter in
;; capitals, a buffer-full at a time, in constant memory. In the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): the stream template's loop of buffer-fulls, read with
;; $fill and written with $write_all, and the vector template's change of 16 bytes at once, here
;; four vectors a step while 64 bytes are left, so the loop's own work is shared by 64 bytes; then
;; 16 bytes at a time, and the last bytes one at a time.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; Two pages: lengua's first 1024 bytes, then the buffer.
  (memory (export "memory") 2 2)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The buffer-fulls it takes at most: 2147483647 of 64 KiB, 128 TiB.
  (global $MAX_PIECES i32 (i32.const 2147483647))
  ;; A buffer-full's steps: blocks of 64 bytes and where the last of them starts; vectors of 16
  ;; bytes, at most three after the last block, and where the last of them starts; and the bytes
  ;; after them.
  (global $BLOCK i32 (i32.const 64))
  (global $BLOCKS i32 (i32.const 1024))
  (global $LAST_BLOCK i32 (i32.const 65472))
  (global $CHUNK i32 (i32.const 16))
  (global $CHUNKS_LEFT i32 (i32.const 3))
  (global $LAST_CHUNK i32 (i32.const 65520))
  (global $TAIL_MAX i32 (i32.const 15))
  ;; Characters: a to z are the 26 bytes from 97, and each capital is 32 below its letter.
  (global $SMALL_A i32 (i32.const 97))
  (global $LETTERS i32 (i32.const 26))
  (global $CASE_BIT i32 (i32.const 32))

  ;; The one region (rule 3, M1): a buffer-full of standard input, capitalized in place, then
  ;; written. 64 KiB is sixteen of the pieces a pipe hands over, so each write sends many, and it
  ;; stays in a core's second-level cache.
  (global $buffer i32 (i32.const 1024))
  (global $buffer.size i32 (i32.const 65536))

  ;; Copies standard input to standard output a buffer-full at a time, until the input ends.
  ;; Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $left i32)
    (block $write_failed
      (block $read_failed
        (block $done
          (local.set $left (global.get $MAX_PIECES))
          (loop $piece  ;; bound: 2147483647
            (local.set $got
              (call $fill (global.get $STDIN) (global.get $buffer) (global.get $buffer.size)
                (global.get $READ_TO_END)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            ;; The input has ended.
            (br_if $done (i32.eqz (local.get $got)))
            (if (i32.gt_u (local.get $got) (global.get $buffer.size))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (call $shout (global.get $buffer) (local.get $got))
            (br_if $write_failed
              (call $write_all (global.get $STDOUT) (global.get $buffer) (local.get $got)))
            ;; A buffer-full that isn't full is the input's last.
            (br_if $done (i32.lt_u (local.get $got) (global.get $buffer.size)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $piece (local.get $left)))
          ;; 128 TiB is all it takes.
          (call $assert_fail (i32.const 2))
          unreachable)
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Capitalizes the $len bytes at $at, at most a buffer-full, which main lends it: 64 bytes a step,
  ;; four vectors of 16, while 64 are left; shout_rest does the rest. In each vector, a lane of all
  ;; ones where the byte is a to z (below `a` wraps to a large number, so one unsigned compare
  ;; finds them), masked to the case bit, is what comes off the lane.
  (func $shout (param $at i32) (param $len i32)
    (local $i i32)
    (local $p i32)
    (local $left i32)
    (local $chunk v128)
    (local $small_a v128)
    (local $letters v128)
    (local $case_bit v128)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 3)) unreachable))
    ;; The constants spread over every lane, once, before the loop.
    (local.set $small_a (i8x16.splat (global.get $SMALL_A)))
    (local.set $letters (i8x16.splat (global.get $LETTERS)))
    (local.set $case_bit (i8x16.splat (global.get $CASE_BIT)))
    (block $rest
      (local.set $left (global.get $BLOCKS))
      (loop $blocks  ;; bound: 1024
        (br_if $rest (i32.gt_u (i32.add (local.get $i) (global.get $BLOCK)) (local.get $len)))
        ;; 64 bytes are left, so the block starts at LAST_BLOCK or before.
        (if (i32.gt_u (local.get $i) (global.get $LAST_BLOCK))
          (then (call $assert_fail (i32.const 4)) unreachable))
        (local.set $p (i32.add (local.get $at) (local.get $i)))
        (v128.store (local.get $p)
          (i8x16.sub (local.tee $chunk (v128.load (local.get $p)))
            (v128.and (local.get $case_bit)
              (i8x16.lt_u (i8x16.sub (local.get $chunk) (local.get $small_a))
                (local.get $letters)))))
        (v128.store offset=16 (local.get $p)
          (i8x16.sub (local.tee $chunk (v128.load offset=16 (local.get $p)))
            (v128.and (local.get $case_bit)
              (i8x16.lt_u (i8x16.sub (local.get $chunk) (local.get $small_a))
                (local.get $letters)))))
        (v128.store offset=32 (local.get $p)
          (i8x16.sub (local.tee $chunk (v128.load offset=32 (local.get $p)))
            (v128.and (local.get $case_bit)
              (i8x16.lt_u (i8x16.sub (local.get $chunk) (local.get $small_a))
                (local.get $letters)))))
        (v128.store offset=48 (local.get $p)
          (i8x16.sub (local.tee $chunk (v128.load offset=48 (local.get $p)))
            (v128.and (local.get $case_bit)
              (i8x16.lt_u (i8x16.sub (local.get $chunk) (local.get $small_a))
                (local.get $letters)))))
        (local.set $i (i32.add (local.get $i) (global.get $BLOCK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $blocks (local.get $left))))
    (call $shout_rest (local.get $at) (local.get $i) (local.get $len)))

  ;; Capitalizes the bytes from $from to $len of the $at that shout lends it, fewer than 64 and
  ;; within a buffer-full: 16 at a time while 16 are left, then the rest one at a time, with the
  ;; same change in i32 arithmetic, where the test's 1 times the case bit is what comes off.
  (func $shout_rest (param $at i32) (param $from i32) (param $len i32)
    (local $i i32)
    (local $c i32)
    (local $left i32)
    (local $chunk v128)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 5)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $len))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (local.set $i (local.get $from))
    (block $tail
      (local.set $left (global.get $CHUNKS_LEFT))
      (loop $chunks  ;; bound: 3
        (br_if $tail (i32.gt_u (i32.add (local.get $i) (global.get $CHUNK)) (local.get $len)))
        ;; 16 bytes are left, so the vector starts at LAST_CHUNK or before.
        (if (i32.gt_u (local.get $i) (global.get $LAST_CHUNK))
          (then (call $assert_fail (i32.const 7)) unreachable))
        (local.set $chunk (v128.load (i32.add (local.get $at) (local.get $i))))
        (v128.store (i32.add (local.get $at) (local.get $i))
          (i8x16.sub (local.get $chunk)
            (v128.and (i8x16.splat (global.get $CASE_BIT))
              (i8x16.lt_u (i8x16.sub (local.get $chunk) (i8x16.splat (global.get $SMALL_A)))
                (i8x16.splat (global.get $LETTERS))))))
        (local.set $i (i32.add (local.get $i) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $chunks (local.get $left))))
    ;; The scalar tail: fewer than 16 bytes are left, one at a time.
    (block $done
      (local.set $left (global.get $TAIL_MAX))
      (loop $bytes  ;; bound: 15
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.ge_u (local.get $i) (global.get $buffer.size))
          (then (call $assert_fail (i32.const 8)) unreachable))
        (local.set $c (i32.load8_u (i32.add (local.get $at) (local.get $i))))
        (i32.store8 (i32.add (local.get $at) (local.get $i))
          (i32.sub (local.get $c)
            (i32.mul (global.get $CASE_BIT)
              (i32.lt_u (i32.sub (local.get $c) (global.get $SMALL_A)) (global.get $LETTERS)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $bytes (local.get $left)))))
)
