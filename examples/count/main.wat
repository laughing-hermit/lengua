;; count: counts the lines and words of standard input and prints the two counts, as wc -lw
;; does. The WebAssembly port of the native count, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): standard input of any length a 64 KiB buffer-full at
;; a time, each counted 16 bytes a step with 128-bit vectors and no bitmask, which ARM64 has no
;; one instruction for. A compare gives each lane all ones where it holds, and the lane's top bit,
;; moved down to its lowest, adds 1 to the lane's counter of a byte. After at most 255 steps,
;; before a byte can overflow, the bytes are widened into counters of 16 bits, and at the piece's
;; end those are added up. The last step, of fewer than 16 bytes, has the lanes past the input's
;; end made spaces, which end no line and start no word, so it is the same code.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; Two pages: lengua's first 1024 bytes, then the regions below, which end at 66730.
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
  ;; The steps a run takes at most: a lane's byte counts to 255 before it overflows.
  (global $RUN i32 (i32.const 255))
  ;; A lane's top bit, and the bits of a lane of a byte and of two.
  (global $TOP_BIT i32 (i32.const 7))
  (global $BYTE_BITS i32 (i32.const 8))
  (global $HALF_BITS i32 (i32.const 16))
  ;; The most a byte holds.
  (global $BYTE_MASK i32 (i32.const 255))
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

  ;; Regions (rule 3, M1), from 1024 up: the usage line and the lanes a last step keeps, both
  ;; read-only, then what args_sizes_get writes, a buffer-full of standard input and the output.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 21))
  (data (i32.const 1024) "usage: count < input\0a")
  ;; Sixteen bytes of all ones, then sixteen of zeros: the 16 bytes from 16 - n on keep the first
  ;; n lanes of a step.
  (global $keep i32 (i32.const 1056))
  (global $keep.rosize i32 (i32.const 32))
  (data (i32.const 1056) "\ff\ff\ff\ff\ff\ff\ff\ff\ff\ff\ff\ff\ff\ff\ff\ff"
    "\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 1088))
  (global $sizes.size i32 (i32.const 8))
  ;; One buffer-full of standard input, 64 KiB: a piece that stays in a core's cache.
  (global $buffer i32 (i32.const 1152))  ;; shared: main fills it; the counting reads it
  (global $buffer.size i32 (i32.const 65536))
  ;; The line it prints: two counts of up to 20 digits, a space and a newline.
  (global $out i32 (i32.const 66688))  ;; shared: put_digits writes digits, print_counts the rest
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
          ;; The byte before the input is none, which counts as whitespace: a space.
          (local.set $before (global.get $SPACE))
          (block $counted
            (local.set $left (global.get $MAX_PIECES))
            (loop $piece  ;; bound: 2147483647
              (local.set $got
                (call $fill (global.get $STDIN) (global.get $buffer) (global.get $buffer.size)
                  (global.get $READ_TO_END)))
              (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
              (if (i32.gt_u (local.get $got) (global.get $buffer.size))
                (then (call $assert_fail (i32.const 2)) unreachable))
              ;; The piece's newlines, the words that start in it, and its last byte, for the
              ;; next.
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
  ;; bytes a step, in runs of at most 255 steps. Each lane of $lines and $words is a byte that
  ;; counts the steps whose byte in that lane is a newline or starts a word; widen adds a run's
  ;; bytes to counters of 16 bits, and count_last counts the bytes after the last whole step. A
  ;; word starts at a byte that isn't whitespace where the byte before it is: $before is the byte
  ;; before the piece, a space when there is none. Gives the newlines, the words that start, and
  ;; the piece's last byte, or $before for an empty piece.
  (func $count_piece (param $len i32) (param $before i32) (result i32 i32 i32)
    (local $i i32)
    (local $last i32)
    (local $runs i32)
    (local $left i32)
    (local $chunk v128)
    (local $space v128)
    (local $was v128)
    (local $lines v128)
    (local $words v128)
    (local $line_halves v128)
    (local $word_halves v128)
    (local $newlines v128)
    (local $spaces v128)
    (local $tab v128)
    (local $tab_to_cr v128)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (if (i32.gt_u (local.get $before) (global.get $BYTE_MASK))
      (then (call $assert_fail (i32.const 5)) unreachable))
    ;; The bytes it compares with, spread over every lane, once, before the loop.
    (local.set $newlines (i8x16.splat (global.get $NEWLINE)))
    (local.set $spaces (i8x16.splat (global.get $SPACE)))
    (local.set $tab (i8x16.splat (global.get $TAB)))
    (local.set $tab_to_cr (i8x16.splat (global.get $TAB_TO_CR)))
    ;; The byte before the piece, as the last lane of a step before the first, its whitespace
    ;; found as every step's is.
    (local.set $was
      (v128.or (i8x16.eq (i8x16.splat (local.get $before)) (local.get $spaces))
        (i8x16.le_u (i8x16.sub (i8x16.splat (local.get $before)) (local.get $tab))
          (local.get $tab_to_cr))))
    (block $counted
      ;; Fewer than 16 bytes: no whole step.
      (br_if $counted (i32.lt_u (local.get $len) (global.get $CHUNK)))
      (local.set $last (i32.sub (local.get $len) (global.get $CHUNK)))
      ;; Each run takes at least one step, so a buffer-full takes CHUNKS runs at most.
      (local.set $runs (global.get $CHUNKS))
      (loop $run  ;; bound: 4096
        (block $ran
          (local.set $left (global.get $RUN))
          (loop $chunks  ;; bound: 255
            (br_if $ran (i32.gt_u (local.get $i) (local.get $last)))
            (local.set $chunk (v128.load (i32.add (global.get $buffer) (local.get $i))))
            ;; A newline adds 1 to its lane: the compare's all ones, moved down to the lowest bit.
            (local.set $lines
              (i8x16.add (local.get $lines)
                (i8x16.shr_u (i8x16.eq (local.get $chunk) (local.get $newlines))
                  (global.get $TOP_BIT))))
            ;; Whitespace: a space, or a byte from tab to carriage return. Below a tab wraps to a
            ;; large number, so one unsigned compare finds those five.
            (local.set $space
              (v128.or (i8x16.eq (local.get $chunk) (local.get $spaces))
                (i8x16.le_u (i8x16.sub (local.get $chunk) (local.get $tab))
                  (local.get $tab_to_cr))))
            ;; A word starts at a byte that isn't whitespace where the byte before it is: the
            ;; whitespace moved up a lane, the step before's last lane first.
            (local.set $words
              (i8x16.add (local.get $words)
                (i8x16.shr_u
                  (v128.andnot
                    (i8x16.shuffle 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30
                      (local.get $was) (local.get $space))
                    (local.get $space))
                  (global.get $TOP_BIT))))
            (local.set $was (local.get $space))
            (local.set $i (i32.add (local.get $i) (global.get $CHUNK)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $chunks (local.get $left))))
        ;; The run's bytes, widened before they can overflow, and the bytes made zero again.
        (local.set $line_halves
          (i16x8.add (local.get $line_halves) (call $widen (local.get $lines))))
        (local.set $word_halves
          (i16x8.add (local.get $word_halves) (call $widen (local.get $words))))
        (local.set $lines (i8x16.splat (i32.const 0)))
        (local.set $words (i8x16.splat (i32.const 0)))
        (br_if $counted (i32.gt_u (local.get $i) (local.get $last)))
        (local.set $runs (i32.sub (local.get $runs) (i32.const 1)))
        (br_if $run (local.get $runs)))
      ;; A run that doesn't end the steps takes 255 of them, so the runs never run out.
      (call $assert_fail (i32.const 6))
      unreachable)
    ;; The bytes after the last whole step, then the totals, and the byte before the next piece.
    (call $count_last (local.get $i) (local.get $len) (local.get $was))
    (local.set $words)
    (local.set $lines)
    (call $total (i16x8.add (local.get $line_halves) (call $widen (local.get $lines))))
    (call $total (i16x8.add (local.get $word_halves) (call $widen (local.get $words))))
    (call $last_byte (local.get $len) (local.get $before)))

  ;; The bytes of $bytes added in pairs, the first and the second, the third and the fourth, and
  ;; so on, as 8 lanes of 16 bits: a pair moved up a byte and back down is its first byte widened,
  ;; and moved down a byte its second.
  (func $widen (param $bytes v128) (result v128)
    (i16x8.add
      (i16x8.shr_u (i16x8.shl (local.get $bytes) (global.get $BYTE_BITS)) (global.get $BYTE_BITS))
      (i16x8.shr_u (local.get $bytes) (global.get $BYTE_BITS))))

  ;; The 8 lanes of 16 bits of $halves added up: widened into 4 lanes of 32 bits as widen widens
  ;; bytes, then the 4 lanes' sum.
  (func $total (param $halves v128) (result i32)
    (local $quads v128)
    (local.set $quads
      (i32x4.add
        (i32x4.shr_u (i32x4.shl (local.get $halves) (global.get $HALF_BITS))
          (global.get $HALF_BITS))
        (i32x4.shr_u (local.get $halves) (global.get $HALF_BITS))))
    (i32.add
      (i32.add (i32x4.extract_lane 0 (local.get $quads))
        (i32x4.extract_lane 1 (local.get $quads)))
      (i32.add (i32x4.extract_lane 2 (local.get $quads))
        (i32x4.extract_lane 3 (local.get $quads)))))

  ;; The newlines, and the words that start, among the bytes of the buffer from $i to $len, fewer
  ;; than 16, each lane 1 where its byte is one, else 0: the lanes past $len are made spaces, which
  ;; end no line and start no word. $was is the whitespace of the step before, as count_piece keeps
  ;; it. Gives two vectors of zeros when no bytes are left.
  (func $count_last (param $i i32) (param $len i32) (param $was v128) (result v128 v128)
    (local $rest i32)
    (local $chunk v128)
    (local $space v128)
    (local $lines v128)
    (local $words v128)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (if (i32.gt_u (local.get $i) (local.get $len))
      (then (call $assert_fail (i32.const 8)) unreachable))
    (local.set $rest (i32.sub (local.get $len) (local.get $i)))
    ;; The whole steps took every 16 bytes, so fewer are left.
    (if (i32.ge_u (local.get $rest) (global.get $CHUNK))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (block $none
      (br_if $none (i32.eqz (local.get $rest)))
      ;; Bytes are left, so the step starts before $len, at LAST_CHUNK or before.
      (if (i32.gt_u (local.get $i) (global.get $LAST_CHUNK))
        (then (call $assert_fail (i32.const 10)) unreachable))
      ;; The step's first $rest lanes from the buffer, and spaces after them.
      (local.set $chunk
        (v128.bitselect
          (v128.load (i32.add (global.get $buffer) (local.get $i)))
          (i8x16.splat (global.get $SPACE))
          (v128.load
            (i32.add (global.get $keep) (i32.sub (global.get $CHUNK) (local.get $rest))))))
      (local.set $lines
        (i8x16.shr_u (i8x16.eq (local.get $chunk) (i8x16.splat (global.get $NEWLINE)))
          (global.get $TOP_BIT)))
      (local.set $space
        (v128.or (i8x16.eq (local.get $chunk) (i8x16.splat (global.get $SPACE)))
          (i8x16.le_u (i8x16.sub (local.get $chunk) (i8x16.splat (global.get $TAB)))
            (i8x16.splat (global.get $TAB_TO_CR)))))
      (local.set $words
        (i8x16.shr_u
          (v128.andnot
            (i8x16.shuffle 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30
              (local.get $was) (local.get $space))
            (local.get $space))
          (global.get $TOP_BIT))))
    (local.get $lines)
    (local.get $words))

  ;; The last of the first $len bytes of the buffer; for no bytes, $before, the byte before them.
  (func $last_byte (param $len i32) (param $before i32) (result i32)
    (local $at i32)
    (local $byte i32)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 11)) unreachable))
    (if (i32.gt_u (local.get $before) (global.get $BYTE_MASK))
      (then (call $assert_fail (i32.const 12)) unreachable))
    (local.set $byte (local.get $before))
    (if (local.get $len)
      (then
        (local.set $at (i32.sub (local.get $len) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $buffer.size))
          (then (call $assert_fail (i32.const 13)) unreachable))
        (local.set $byte (i32.load8_u (i32.add (global.get $buffer) (local.get $at))))))
    (local.get $byte))

  ;; Writes $value's decimal digits into the output, the last just before byte $end, and gives
  ;; where the first is.
  (func $put_digits (param $value i64) (param $end i32) (result i32)
    (local $at i32)
    (local $rest i64)
    (local $left i32)
    (if (i32.gt_u (local.get $end) (global.get $out.size))
      (then (call $assert_fail (i32.const 14)) unreachable))
    (local.set $at (local.get $end))
    (local.set $rest (local.get $value))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 15)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; A 64-bit number has at most 20 digits.
      (call $assert_fail (i32.const 16))
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
      (then (call $assert_fail (i32.const 17)) unreachable))
    (local.set $at (i32.sub (local.get $at) (i32.const 1)))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $SPACE))
    (local.set $at (call $put_digits (local.get $lines) (local.get $at)))
    (if (i32.ge_u (local.get $at) (global.get $out.size))
      (then (call $assert_fail (i32.const 18)) unreachable))
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
