;; count: counts the lines and words of standard input and prints the two counts, as wc -lw
;; does. The WebAssembly port of the native count, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): standard input of any length a 64 KiB buffer-full at
;; a time, each counted 16 bytes a step with 128-bit vectors. A compare gives each lane all ones
;; where it holds, its bitmask a bit for each lane, and the bitmask's bits are counted; the lanes
;; past the input's end are masked off, so the last step needs no code of its own.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; Two pages: lengua's first 1024 bytes, then the regions below, which end at 66666.
  (memory (export "memory") 2 2)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_READ_FAILED i32 (i32.const 2))
  (global $EXIT_WRITE_FAILED i32 (i32.const 3))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The argument count of a run with no arguments: the program's name alone.
  (global $ARGC_EXPECTED i32 (i32.const 1))
  ;; The size of a count args_sizes_get writes.
  (global $WORD i32 (i32.const 4))
  ;; The buffer-fulls it takes at most: 2147483647 of 64 KiB, 128 TiB.
  (global $MAX_PIECES i32 (i32.const 2147483647))
  ;; A buffer-full's steps of 16 bytes, and where the last of them starts.
  (global $CHUNK i32 (i32.const 16))
  (global $CHUNKS i32 (i32.const 4096))
  (global $LAST_CHUNK i32 (i32.const 65520))
  ;; A step's 16 lanes as the bits of a bitmask, and one of them.
  (global $ALL_LANES i32 (i32.const 65535))
  (global $ONE i32 (i32.const 1))
  ;; Printing: two counts of up to 20 digits, a space between them and a newline after, the
  ;; newline at the output's last byte; the second count starts at FIRST_ROOM or later.
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $NEWLINE_AT i32 (i32.const 41))
  (global $FIRST_ROOM i32 (i32.const 21))
  (global $TEN i64 (i64.const 10))
  ;; Bytes that matter. Tab to carriage return are the 5 bytes from 9 to 13.
  (global $NEWLINE i32 (i32.const 10))
  (global $SPACE i32 (i32.const 32))
  (global $TAB i32 (i32.const 9))
  (global $TAB_TO_CR i32 (i32.const 4))
  (global $ZERO i32 (i32.const 48))

  ;; Regions (rule 3, M1), from 1024 up: the usage line, read-only, then what args_sizes_get
  ;; writes, a buffer-full of standard input and the output.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 21))
  (data (i32.const 1024) "usage: count < input\0a")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 1056))
  (global $sizes.size i32 (i32.const 8))
  ;; One buffer-full of standard input, 64 KiB: a piece that stays in a core's cache.
  (global $buffer i32 (i32.const 1088))  ;; shared: main fills it, and count_piece reads it
  (global $buffer.size i32 (i32.const 65536))
  ;; The line it prints: two counts of up to 20 digits, a space and a newline.
  (global $out i32 (i32.const 66624))  ;; shared: put_digits writes digits, print_counts the rest
  (global $out.size i32 (i32.const 42))

  ;; Checks there are no arguments, then counts the newlines and the words of standard input a
  ;; buffer-full at a time, and prints both. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $got i32)
    (local $before i32)
    (local $piece_lines i32)
    (local $piece_words i32)
    (local $lines i64)
    (local $words i64)
    (local $left i32)
    (block $write_failed
      (block $read_failed
        (block $usage
          (local.set $err
            (call $args_sizes_get (global.get $sizes)
              (i32.add (global.get $sizes) (global.get $WORD))))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 1)) unreachable))
          (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
          ;; The byte before the input is none, which counts as whitespace.
          (local.set $before (global.get $ONE))
          (block $counted
            (local.set $left (global.get $MAX_PIECES))
            (loop $piece  ;; bound: 2147483647
              (local.set $got
                (call $fill (global.get $STDIN) (global.get $buffer) (global.get $buffer.size)
                  (global.get $READ_TO_END)))
              (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
              (if (i32.gt_u (local.get $got) (global.get $buffer.size))
                (then (call $assert_fail (i32.const 2)) unreachable))
              ;; The piece's newlines, the words that start in it, and whether its last byte is
              ;; whitespace, for the next.
              (call $count_piece (local.get $got) (local.get $before))
              (local.set $before)
              (local.set $piece_words)
              (local.set $piece_lines)
              (local.set $lines
                (i64.add (local.get $lines) (i64.extend_i32_u (local.get $piece_lines))))
              (local.set $words
                (i64.add (local.get $words) (i64.extend_i32_u (local.get $piece_words))))
              ;; A buffer-full that isn't full is the input's last.
              (br_if $counted (i32.lt_u (local.get $got) (global.get $buffer.size)))
              (local.set $left (i32.sub (local.get $left) (i32.const 1)))
              (br_if $piece (local.get $left)))
            ;; 128 TiB is all it takes.
            (call $assert_fail (i32.const 3))
            unreachable)
          (br_if $write_failed (call $print_counts (local.get $lines) (local.get $words)))
          (call $proc_exit (global.get $EXIT_OK))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
        (call $proc_exit (global.get $EXIT_USAGE))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Counts the newlines, and the words that start, among the first $len bytes of the buffer, 16
  ;; bytes a step. The lanes before $len are the low bits of $valid, so the bytes past the end
  ;; count for nothing. A word starts at a byte that isn't whitespace where the byte before it
  ;; is: $before is 1 when the byte before the piece is whitespace, or there is none. Gives the
  ;; newlines, the words that start, and 1 when the piece's last byte is whitespace, else 0.
  (func $count_piece (param $len i32) (param $before i32) (result i32 i32 i32)
    (local $i i32)
    (local $lines i32)
    (local $words i32)
    (local $rest i32)
    (local $valid i32)
    (local $space i32)
    (local $left i32)
    (local $chunk v128)
    (local $newlines v128)
    (local $spaces v128)
    (local $tab v128)
    (local $tab_to_cr v128)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (if (i32.gt_u (local.get $before) (global.get $ONE))
      (then (call $assert_fail (i32.const 5)) unreachable))
    ;; The bytes it compares with, spread over every lane, once, before the loop.
    (local.set $newlines (i8x16.splat (global.get $NEWLINE)))
    (local.set $spaces (i8x16.splat (global.get $SPACE)))
    (local.set $tab (i8x16.splat (global.get $TAB)))
    (local.set $tab_to_cr (i8x16.splat (global.get $TAB_TO_CR)))
    (block $done
      (local.set $left (global.get $CHUNKS))
      (loop $chunks  ;; bound: 4096
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        ;; A step starts before $len, so at LAST_CHUNK or before.
        (if (i32.gt_u (local.get $i) (global.get $LAST_CHUNK))
          (then (call $assert_fail (i32.const 6)) unreachable))
        ;; The bytes left, at most 16, as that many low bits.
        (local.set $rest (i32.sub (local.get $len) (local.get $i)))
        (local.set $rest
          (select (local.get $rest) (global.get $CHUNK)
            (i32.lt_u (local.get $rest) (global.get $CHUNK))))
        (local.set $valid
          (i32.sub (i32.shl (i32.const 1) (local.get $rest)) (i32.const 1)))
        (local.set $chunk (v128.load (i32.add (global.get $buffer) (local.get $i))))
        (local.set $lines
          (i32.add (local.get $lines)
            (i32.popcnt
              (i32.and (local.get $valid)
                (i8x16.bitmask (i8x16.eq (local.get $chunk) (local.get $newlines)))))))
        ;; Whitespace: a space, or a byte from tab to carriage return. Below a tab wraps to a
        ;; large number, so one unsigned compare finds those five.
        (local.set $space
          (i8x16.bitmask
            (v128.or (i8x16.eq (local.get $chunk) (local.get $spaces))
              (i8x16.le_u (i8x16.sub (local.get $chunk) (local.get $tab))
                (local.get $tab_to_cr)))))
        ;; A word starts at a byte that isn't whitespace where the one before is: the mask moved
        ;; up a place, with the byte before the step as its first bit.
        (local.set $words
          (i32.add (local.get $words)
            (i32.popcnt
              (i32.and
                (i32.and (local.get $valid) (i32.xor (local.get $space) (global.get $ALL_LANES)))
                (i32.or (i32.shl (local.get $space) (i32.const 1)) (local.get $before))))))
        ;; Whether the step's last byte before $len is whitespace, for the next step.
        (local.set $before
          (i32.and
            (i32.shr_u (local.get $space) (i32.sub (local.get $rest) (i32.const 1)))
            (i32.const 1)))
        (local.set $i (i32.add (local.get $i) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $chunks (local.get $left))))
    (local.get $lines)
    (local.get $words)
    (local.get $before))

  ;; Writes $value's decimal digits into the output, the last just before byte $end, and gives
  ;; where the first is.
  (func $put_digits (param $value i64) (param $end i32) (result i32)
    (local $at i32)
    (local $rest i64)
    (local $left i32)
    (if (i32.gt_u (local.get $end) (global.get $out.size))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (local.set $at (local.get $end))
    (local.set $rest (local.get $value))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 8)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; A 64-bit number has at most 20 digits.
      (call $assert_fail (i32.const 9))
      unreachable)
    (local.get $at))

  ;; Prints $lines, a space, $words and a newline to standard output: the newline last, the
  ;; words' digits before it, then the space and the lines' digits. Gives 0, or an error number.
  (func $print_counts (param $lines i64) (param $words i64) (result i32)  ;; returns: result
    (local $at i32)
    (i32.store8 (i32.add (global.get $out) (global.get $NEWLINE_AT)) (global.get $NEWLINE))
    (local.set $at (call $put_digits (local.get $words) (global.get $NEWLINE_AT)))
    ;; Room before the words' digits for the space and the lines' 20 digits at most.
    (if (i32.lt_u (local.get $at) (global.get $FIRST_ROOM))
      (then (call $assert_fail (i32.const 10)) unreachable))
    (local.set $at (i32.sub (local.get $at) (i32.const 1)))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $SPACE))
    (local.set $at (call $put_digits (local.get $lines) (local.get $at)))
    (if (i32.ge_u (local.get $at) (global.get $out.size))
      (then (call $assert_fail (i32.const 11)) unreachable))
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
