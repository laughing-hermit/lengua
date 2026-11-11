;; vowels: counts the vowels a, e, i, o and u, small or capital, in standard input of any length,
;; a buffer-full at a time in constant memory, and prints the count and a newline. In the
;; WebAssembly dialect 1: lengua's library reads with $fill and writes with $write_all.
;; $vowels_in looks at 16 bytes at a time with 128-bit vectors, then at the rest one at a time.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; Two pages: lengua's first 1024 bytes, then the buffer and the output.
  (memory (export "memory") 2 2)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_WRITE_FAILED i32 (i32.const 2))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The buffer-fulls it takes at most: 2147483647 of 64 KiB, 128 TiB.
  (global $MAX_PIECES i32 (i32.const 2147483647))
  ;; A buffer-full's whole vectors, where the last of them starts, and the bytes after them.
  (global $CHUNK i32 (i32.const 16))
  (global $CHUNKS i32 (i32.const 4096))
  (global $LAST_CHUNK i32 (i32.const 65520))
  (global $TAIL_MAX i32 (i32.const 15))
  ;; Characters: the case bit makes a capital letter small, then the five small vowels.
  (global $CASE_BIT i32 (i32.const 32))
  (global $SMALL_A i32 (i32.const 97))
  (global $SMALL_E i32 (i32.const 101))
  (global $SMALL_I i32 (i32.const 105))
  (global $SMALL_O i32 (i32.const 111))
  (global $SMALL_U i32 (i32.const 117))
  (global $NEWLINE i32 (i32.const 10))
  (global $ZERO i32 (i32.const 48))
  ;; The output: at most 20 digits, as many as a 64-bit number has, then the newline.
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $NEWLINE_AT i32 (i32.const 20))
  (global $TEN i64 (i64.const 10))

  ;; Regions (rule 3, M1), from 1024 up: a buffer-full of standard input, then the output.
  (global $buffer i32 (i32.const 1024))
  (global $buffer.size i32 (i32.const 65536))
  (global $out i32 (i32.const 66560))
  (global $out.size i32 (i32.const 21))

  ;; Counts the vowels of standard input a buffer-full at a time, until the input ends, then
  ;; prints the count. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $left i32)
    (local $count i64)
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
            ;; At most 65536 a buffer-full and 2147483647 buffer-fulls: the count fits in 47 bits.
            (local.set $count
              (i64.add (local.get $count)
                (i64.extend_i32_u (call $vowels_in (global.get $buffer) (local.get $got)))))
            ;; A buffer-full that isn't full is the input's last.
            (br_if $done (i32.lt_u (local.get $got) (global.get $buffer.size)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $piece (local.get $left)))
          ;; 128 TiB is all it takes.
          (call $assert_fail (i32.const 2))
          unreachable)
        (br_if $write_failed (call $write_u64 (local.get $count)))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Counts the vowels in the $len bytes at $at, which main lends it: a, e, i, o and u, small or
  ;; capital. It takes 16 bytes at a time while 16 are left: the case bit on each lane, a
  ;; comparison with each vowel, and the bitmask's set bits are the vowels. Then it takes the
  ;; rest one at a time. No branch depends on a byte.
  (func $vowels_in (param $at i32) (param $len i32) (result i32)
    (local $i i32)
    (local $c i32)
    (local $left i32)
    (local $n i32)
    (local $chunk v128)
    (local $case_bit v128)
    (local $a v128)
    (local $e v128)
    (local $small_i v128)
    (local $o v128)
    (local $u v128)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 3)) unreachable))
    ;; The constants spread over every lane, once, before the loop.
    (local.set $case_bit (i8x16.splat (global.get $CASE_BIT)))
    (local.set $a (i8x16.splat (global.get $SMALL_A)))
    (local.set $e (i8x16.splat (global.get $SMALL_E)))
    (local.set $small_i (i8x16.splat (global.get $SMALL_I)))
    (local.set $o (i8x16.splat (global.get $SMALL_O)))
    (local.set $u (i8x16.splat (global.get $SMALL_U)))
    (block $tail
      (local.set $left (global.get $CHUNKS))
      (loop $chunks  ;; bound: 4096
        (br_if $tail (i32.gt_u (i32.add (local.get $i) (global.get $CHUNK)) (local.get $len)))
        ;; 16 bytes are left, so the vector starts at LAST_CHUNK or before.
        (if (i32.gt_u (local.get $i) (global.get $LAST_CHUNK))
          (then (call $assert_fail (i32.const 4)) unreachable))
        (local.set $chunk
          (v128.or (v128.load (i32.add (local.get $at) (local.get $i))) (local.get $case_bit)))
        (local.set $n
          (i32.add (local.get $n)
            (i32.popcnt
              (i8x16.bitmask
                (v128.or
                  (v128.or
                    (i8x16.eq (local.get $chunk) (local.get $a))
                    (i8x16.eq (local.get $chunk) (local.get $e)))
                  (v128.or
                    (v128.or
                      (i8x16.eq (local.get $chunk) (local.get $small_i))
                      (i8x16.eq (local.get $chunk) (local.get $o)))
                    (i8x16.eq (local.get $chunk) (local.get $u))))))))
        (local.set $i (i32.add (local.get $i) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $chunks (local.get $left))))
    ;; The scalar tail: fewer than 16 bytes are left, one at a time.
    (block $done
      (local.set $left (global.get $TAIL_MAX))
      (loop $bytes  ;; bound: 15
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.ge_u (local.get $i) (global.get $buffer.size))
          (then (call $assert_fail (i32.const 5)) unreachable))
        ;; The case bit makes A to Z small, and makes no other byte a small vowel.
        (local.set $c
          (i32.or (i32.load8_u (i32.add (local.get $at) (local.get $i))) (global.get $CASE_BIT)))
        (local.set $n
          (i32.add (local.get $n)
            (i32.add
              (i32.add
                (i32.eq (local.get $c) (global.get $SMALL_A))
                (i32.eq (local.get $c) (global.get $SMALL_E)))
              (i32.add
                (i32.add
                  (i32.eq (local.get $c) (global.get $SMALL_I))
                  (i32.eq (local.get $c) (global.get $SMALL_O)))
                (i32.eq (local.get $c) (global.get $SMALL_U))))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $bytes (local.get $left))))
    ;; At most one vowel a byte.
    (if (i32.gt_u (local.get $n) (local.get $len))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (local.get $n))

  ;; Writes $value in decimal, then a newline, to standard output. Gives 0, or an error number.
  (func $write_u64 (param $value i64) (result i32)  ;; returns: result
    (local $at i32)
    (local $left i32)
    (local $rest i64)
    (local.set $rest (local.get $value))
    ;; The newline last, the digits before it from the last.
    (local.set $at (global.get $NEWLINE_AT))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 7)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; 20 digits are all a 64-bit number has.
      (call $assert_fail (i32.const 8))
      unreachable)
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at)))))
