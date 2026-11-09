;; shout: reads a line from standard input and prints it in capital letters, 16 bytes at a time
;; with 128-bit vectors. The compile skill's vector template, in the WebAssembly
;; dialect 1 (references/power-of-10-webassembly.md): $find_newline compares 16 bytes at once and
;; takes the first newline's place from a bitmask, and $shout capitalizes 16 bytes at once while
;; 16 are left, then the rest one at a time, a scalar tail.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_NO_INPUT i32 (i32.const 1))
  (global $EXIT_TOO_LONG i32 (i32.const 2))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  ;; Sizes and limits: a line of at most MAX_LINE bytes and its newline, READ_MAX, read into
  ;; $line, whose 5 vectors of 16 bytes hold them.
  (global $MAX_LINE i32 (i32.const 64))
  (global $READ_MAX i32 (i32.const 65))
  (global $CHUNK i32 (i32.const 16))
  (global $LINE_CHUNKS i32 (i32.const 5))
  ;; Where $line's last vector starts.
  (global $LAST_CHUNK i32 (i32.const 64))
  ;; A line's whole vectors, where the last of them starts, and the bytes after them.
  (global $SHOUT_CHUNKS i32 (i32.const 4))
  (global $LAST_SHOUT i32 (i32.const 48))
  (global $TAIL_MAX i32 (i32.const 15))
  ;; Characters: a to z are the 26 bytes from 97, and each capital is 32 below its letter.
  (global $NEWLINE i32 (i32.const 10))
  (global $SMALL_A i32 (i32.const 97))
  (global $LETTERS i32 (i32.const 26))
  (global $CASE_BIT i32 (i32.const 32))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then the line.
  (global $no_input i32 (i32.const 1024))
  (global $no_input.rosize i32 (i32.const 16))
  (data (i32.const 1024) "error: no input\0a")
  (global $too_long i32 (i32.const 1056))
  (global $too_long.rosize i32 (i32.const 21))
  (data (i32.const 1056) "error: line too long\0a")
  ;; READ_MAX bytes of input, rounded up to whole vectors so that every vector load stays inside;
  ;; main lends it to the functions that work on it.
  (global $line i32 (i32.const 1088))
  (global $line.size i32 (i32.const 80))

  ;; Reads a line, prints it in capital letters, or says why it can't. Never returns: every path
  ;; ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $len i32)
    (block $write_failed
      (block $read_failed
        (block $too_long
          (block $no_input
            ;; Until a newline arrives, the input ends or READ_MAX bytes are in.
            (local.set $got
              (call $fill (global.get $STDIN) (global.get $line) (global.get $READ_MAX)
                (global.get $NEWLINE)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $READ_MAX))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (local.set $len (call $find_newline (global.get $line) (local.get $got)))
            (br_if $too_long (i32.gt_u (local.get $len) (global.get $MAX_LINE)))
            (br_if $no_input (i32.eqz (local.get $len)))
            (call $shout (global.get $line) (local.get $len))
            ;; The newline after the line, where its own may not be.
            (i32.store8 (i32.add (global.get $line) (local.get $len)) (global.get $NEWLINE))
            (br_if $write_failed
              (call $write_all (global.get $STDOUT) (global.get $line)
                (i32.add (local.get $len) (i32.const 1))))
            (call $proc_exit (global.get $EXIT_OK))
            unreachable)
          (br_if $write_failed
            (call $write_all (global.get $STDERR) (global.get $no_input)
              (global.get $no_input.rosize)))
          (call $proc_exit (global.get $EXIT_NO_INPUT))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $too_long)
            (global.get $too_long.rosize)))
        (call $proc_exit (global.get $EXIT_TOO_LONG))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Finds where the line in the $got bytes at $at ends, which main lends it with room for 80
  ;; bytes: at its first newline, or after the bytes when they hold none. It compares 16 bytes
  ;; at once with a vector of newlines, and the comparison's bitmask has a bit for each lane, so
  ;; its trailing zeros are the first newline's place. Bytes past $got are the zeros memory starts
  ;; with, which no newline is.
  (func $find_newline (param $at i32) (param $got i32) (result i32)
    (local $i i32)
    (local $found i32)
    (local $mask i32)
    (local $left i32)
    (local $newlines v128)
    (if (i32.gt_u (local.get $got) (global.get $READ_MAX))
      (then (call $assert_fail (i32.const 2)) unreachable))
    (local.set $found (local.get $got))
    (local.set $newlines (i8x16.splat (global.get $NEWLINE)))
    (block $done
      (local.set $left (global.get $LINE_CHUNKS))
      (loop $chunk  ;; bound: 5
        (if (i32.gt_u (local.get $i) (global.get $LAST_CHUNK))
          (then (call $assert_fail (i32.const 3)) unreachable))
        (local.set $mask
          (i8x16.bitmask
            (i8x16.eq (v128.load (i32.add (local.get $at) (local.get $i))) (local.get $newlines))))
        (if (local.get $mask)
          (then
            (local.set $found (i32.add (local.get $i) (i32.ctz (local.get $mask))))
            (br $done)))
        (local.set $i (i32.add (local.get $i) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $chunk (local.get $left))))
    ;; A newline past what was read is none.
    (select (local.get $found) (local.get $got)
      (i32.lt_u (local.get $found) (local.get $got))))

  ;; Capitalizes the $len bytes at $at, at most MAX_LINE, which main lends it: 16 bytes at a time
  ;; while 16 are left, then the rest one at a time. a to z become A to Z, and the other bytes
  ;; stay as they are.
  (func $shout (param $at i32) (param $len i32)
    (local $i i32)
    (local $c i32)
    (local $left i32)
    (local $chunk v128)
    (local $small_a v128)
    (local $letters v128)
    (local $case_bit v128)
    (if (i32.gt_u (local.get $len) (global.get $MAX_LINE))
      (then (call $assert_fail (i32.const 4)) unreachable))
    ;; The constants spread over every lane, once, before the loop.
    (local.set $small_a (i8x16.splat (global.get $SMALL_A)))
    (local.set $letters (i8x16.splat (global.get $LETTERS)))
    (local.set $case_bit (i8x16.splat (global.get $CASE_BIT)))
    (block $tail
      (local.set $left (global.get $SHOUT_CHUNKS))
      (loop $chunks  ;; bound: 4
        (br_if $tail (i32.gt_u (i32.add (local.get $i) (global.get $CHUNK)) (local.get $len)))
        ;; 16 bytes are left, so the vector starts at LAST_SHOUT or before.
        (if (i32.gt_u (local.get $i) (global.get $LAST_SHOUT))
          (then (call $assert_fail (i32.const 5)) unreachable))
        (local.set $chunk (v128.load (i32.add (local.get $at) (local.get $i))))
        ;; A lane of all ones where the byte is a to z: below 'a' wraps to a large number, so one
        ;; unsigned compare finds them. Masked to the case bit, it is what comes off each lane.
        (v128.store (i32.add (local.get $at) (local.get $i))
          (i8x16.sub (local.get $chunk)
            (v128.and (local.get $case_bit)
              (i8x16.lt_u (i8x16.sub (local.get $chunk) (local.get $small_a))
                (local.get $letters)))))
        (local.set $i (i32.add (local.get $i) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $chunks (local.get $left))))
    ;; The scalar tail: fewer than 16 bytes are left, one at a time.
    (block $done
      (local.set $left (global.get $TAIL_MAX))
      (loop $bytes  ;; bound: 15
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.ge_u (local.get $i) (global.get $MAX_LINE))
          (then (call $assert_fail (i32.const 6)) unreachable))
        (local.set $c (i32.load8_u (i32.add (local.get $at) (local.get $i))))
        (i32.store8 (i32.add (local.get $at) (local.get $i))
          (i32.sub (local.get $c)
            (i32.mul (global.get $CASE_BIT)
              (i32.lt_u (i32.sub (local.get $c) (global.get $SMALL_A)) (global.get $LETTERS)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $bytes (local.get $left))))))
