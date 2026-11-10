;; sort-large: sorts the lines of standard input of up to 64 MiB in byte order, as LC_ALL=C sort
;; does. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md): lengua's library
;; reads the whole input with $fill, and each line gets a record of 8 bytes, found 16 bytes at a
;; time from a bitmask of the newlines: the line's start in the low bits, and above it a key, 4
;; bytes of the line, the first the highest, so two keys compare as the bytes do, as one number.
;; A multikey quicksort orders the records in place, with no recursion: a stack of parts, each a
;; run of records whose lines are the same up to a depth, keyed by their 4 bytes from there. Two
;; passes split a part around the median of three keys, with no branch on a record: the records
;; below the pivot first, then those the same as it, then those above it. The lines the same as
;; the pivot get their next 4 bytes as their key and are sorted on, unless they end in these. A
;; line is read again only for its next key. A part split below or above a pivot more times than
;; twice the bits of the number of lines, and 8 more, is sorted by a heap sort instead, as
;; introsort does, so even an input made against the pivots' choice takes about n log n
;; comparisons, not n squared. The lines go out 16 bytes a step through a 64 KiB buffer.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; 1538 pages: lengua's first 1024 bytes, then the regions below, which end at 100736208.
  (memory (export "memory") 1538 1538)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_WRITE_FAILED i32 (i32.const 2))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  (global $EXIT_TOO_MUCH i32 (i32.const 4))
  ;; What emit gives when every write went out, and what index_lines gives for too many lines.
  (global $NO_ERROR i32 (i32.const 0))
  (global $TOO_MANY i32 (i32.const -1))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The longest input, and the bytes read, one more, which shows a longer input; the most lines.
  (global $MAX_INPUT i32 (i32.const 67108864))
  (global $READ_ROOM i32 (i32.const 67108865))
  (global $MAX_LINES i32 (i32.const 4194304))
  ;; Vectors of 16 bytes: the most that pass the input, and where the last starts.
  (global $CHUNK i32 (i32.const 16))
  (global $VECTORS i32 (i32.const 4194306))
  (global $LAST_VECTOR i32 (i32.const 67113008))
  (global $NEWLINE i32 (i32.const 10))
  ;; A record, 8 bytes: the line's start in its low 26 bits, below MAX_INPUT; from bit KEY_SHIFT,
  ;; its key: how many of the key's KEY_BYTES bytes are the line's, 0 to 4, under COUNT_MASK, and
  ;; from bit BYTES_SHIFT the bytes. A record shifted down by KEY_SHIFT is its key, and two keys
  ;; compare as their lines do where the lines are the same before them.
  (global $RECORD_SHIFT i32 (i32.const 3))
  (global $START_MASK i32 (i32.const 67108863))
  (global $KEY_SHIFT i64 (i64.const 28))
  (global $BYTES_SHIFT i64 (i64.const 32))
  (global $COUNT_MASK i32 (i32.const 7))
  (global $KEY_BYTES i32 (i32.const 4))
  (global $BYTE_SHIFT i32 (i32.const 3))
  ;; Finding a line's newline in 4 bytes: those that are zero after an exclusive or with NEWLINES
  ;; borrow when ONES is taken from them, which their high bits, HIGHS, show; the lowest is exact.
  ;; A key's 4 bytes are loaded from LAST_WORD or before.
  (global $NEWLINES i32 (i32.const 168430090))
  (global $ONES i32 (i32.const 16843009))
  (global $HIGHS i32 (i32.const -2139062144))
  (global $ALL_ONES i32 (i32.const -1))
  (global $HIGH_BIT i32 (i32.const 7))
  (global $LAST_WORD i32 (i32.const 67113020))
  ;; Sorting: each step takes a part off the stack, MAX_STEPS steps at most. The stack holds
  ;; STACK_MAX parts of 16 bytes, and a part of FEWEST records or more is put on it. A part's
  ;; pivot is the median of three of its records, which the bits of SEED, made new by xorshift
  ;; each step, pick: each PICK_BITS of them a fraction of the part.
  (global $MAX_STEPS i32 (i32.const 2147483647))
  (global $STACK_MAX i32 (i32.const 128))
  (global $STACK_SHIFT i32 (i32.const 4))
  (global $FEWEST i32 (i32.const 2))
  (global $SEED i64 (i64.const 88172645463325252))
  (global $XOR_FIRST i64 (i64.const 13))
  (global $XOR_SECOND i64 (i64.const 7))
  (global $XOR_THIRD i64 (i64.const 17))
  (global $PICK_BITS i64 (i64.const 21))
  (global $PICK_TWICE i64 (i64.const 42))
  (global $PICK_MASK i64 (i64.const 2097151))
  ;; The splits below or above a pivot a part may take, then a heap sort sorts it: twice the
  ;; bits of the number of lines, WORD_BITS less the zeros above its highest one, and
  ;; SPARE_SPLITS more. A heap of MAX_LINES records has HEAP_LEVELS levels and HALF_LINES places
  ;; with a child at most, and two lines go on the same for fewer than MAX_KEYS keys.
  (global $WORD_BITS i32 (i32.const 32))
  (global $SPARE_SPLITS i32 (i32.const 8))
  (global $HEAP_LEVELS i32 (i32.const 23))
  (global $HALF_LINES i32 (i32.const 2097152))
  (global $MAX_KEYS i32 (i32.const 16777217))
  ;; Printing: the output takes 16 bytes more where it holds LAST_OUT bytes or fewer.
  (global $LAST_OUT i32 (i32.const 65536))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then the input, the records,
  ;; the stack and the output.
  (global $too_long i32 (i32.const 1024))
  (global $too_long.rosize i32 (i32.const 40))
  (data (i32.const 1024) "error: input longer than 67108864 bytes\0a")
  (global $too_many i32 (i32.const 1088))
  (global $too_many.rosize i32 (i32.const 31))
  (data (i32.const 1088) "error: more than 4194304 lines\0a")
  ;; Standard input: READ_ROOM bytes, then room to spare, so every vector and every key loaded
  ;; from a place inside the input stays inside.
  (global $input i32 (i32.const 1152))  ;; shared: main fills it, and the lines are read from it
  (global $input.size i32 (i32.const 67113024))
  ;; Each line's record, MAX_LINES of them.
  (global $records i32 (i32.const 67114176))  ;; shared: index_lines writes them, sort orders them
  (global $records.size i32 (i32.const 33554432))
  ;; The parts still to sort: each a record's place where it starts and where it ends, the depth
  ;; its lines are the same to, and how many more splits it may take, 4 bytes each.
  (global $stack i32 (i32.const 100668608))  ;; shared: push puts the parts on it, sort takes them
  (global $stack.size i32 (i32.const 2048))
  ;; The lines it prints, a buffer-full at a time, and room for a vector more.
  (global $out i32 (i32.const 100670656))  ;; shared: put_line fills it, emit prints the rest
  (global $out.size i32 (i32.const 65552))

  ;; Reads standard input, sorts its lines and prints them, or says why it can't. Never returns:
  ;; every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $len i32)
    (local $lines i32)
    (block $write_failed
      (block $read_failed
        (block $too_many
          (block $too_long
            (local.set $got
              (call $fill (global.get $STDIN) (global.get $input) (global.get $READ_ROOM)
                (global.get $READ_TO_END)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $READ_ROOM))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (br_if $too_long (i32.gt_u (local.get $got) (global.get $MAX_INPUT)))
            (local.set $len (call $end_lines (local.get $got)))
            (local.set $lines (call $index_lines (local.get $len)))
            (br_if $too_many (i32.lt_s (local.get $lines) (i32.const 0)))
            (if (i32.gt_u (local.get $lines) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 2)) unreachable))
            ;; Each line's first key, then the records in their lines' order.
            (call $refresh (i32.const 0) (local.get $lines) (i32.const 0))
            (call $sort (local.get $lines))
            (br_if $write_failed (call $emit (local.get $lines)))
            (call $proc_exit (global.get $EXIT_OK))
            unreachable)
          (br_if $write_failed
            (call $write_all (global.get $STDERR) (global.get $too_long)
              (global.get $too_long.rosize)))
          (call $proc_exit (global.get $EXIT_TOO_MUCH))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $too_many)
            (global.get $too_many.rosize)))
        (call $proc_exit (global.get $EXIT_TOO_MUCH))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Gives the input's $got bytes a newline at their end where they have bytes and their last
  ;; isn't one, so every line ends with a newline. Gives how many bytes the input holds then.
  (func $end_lines (param $got i32) (result i32)
    (local $last i32)
    (local $len i32)
    (if (i32.gt_u (local.get $got) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (local.set $len (local.get $got))
    (if (local.get $got)
      (then
        (local.set $last (i32.sub (local.get $got) (i32.const 1)))
        (if (i32.ge_u (local.get $last) (global.get $MAX_INPUT))
          (then (call $assert_fail (i32.const 4)) unreachable))
        (if (i32.ne (i32.load8_u (i32.add (global.get $input) (local.get $last)))
              (global.get $NEWLINE))
          (then
            (i32.store8 (i32.add (global.get $input) (local.get $got)) (global.get $NEWLINE))
            (local.set $len (i32.add (local.get $got) (i32.const 1)))))))
    (if (i32.gt_u (local.get $len) (global.get $READ_ROOM))
      (then (call $assert_fail (i32.const 5)) unreachable))
    (local.get $len))

  ;; Writes a record for each line of the input's first $len bytes, in order, with the line's
  ;; start and no key yet: the first at 0, and each other right after a newline, which the
  ;; bitmask of each 16 bytes' newline lanes gives, the lowest first. The bytes end with a
  ;; newline, and the bytes past them are the zeros memory starts with, which no newline is.
  ;; Gives how many lines there are, or TOO_MANY.
  (func $index_lines (param $len i32) (result i32)
    (local $at i32)
    (local $found v128)
    (local $mask i32)
    (local $start i32)
    (local $count i32)
    (local $result i32)
    (local $bits i32)
    (local $newlines v128)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $READ_ROOM))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (local.set $result (global.get $TOO_MANY))
    (local.set $newlines (i8x16.splat (global.get $NEWLINE)))
    (block $failed
      (block $done
        (br_if $done (i32.eqz (local.get $len)))
        ;; The first line starts at 0.
        (i64.store (global.get $records) (i64.const 0))
        (local.set $count (i32.const 1))
        (local.set $left (global.get $VECTORS))
        (loop $vectors  ;; bound: 4194306
          (br_if $done (i32.ge_u (local.get $at) (local.get $len)))
          (if (i32.gt_u (local.get $at) (global.get $LAST_VECTOR))
            (then (call $assert_fail (i32.const 7)) unreachable))
          (local.set $found
            (i8x16.eq (v128.load (i32.add (global.get $input) (local.get $at)))
              (local.get $newlines)))
          ;; Most 16 bytes hold no newline, which one test shows.
          (if (v128.any_true (local.get $found))
            (then
              (local.set $mask (i8x16.bitmask (local.get $found)))
              ;; Each newline's lane, from the lowest: the next line starts after it, unless the
              ;; bytes end there.
              (block $noted
                (local.set $bits (global.get $CHUNK))
                (loop $lanes  ;; bound: 16
                  (br_if $noted (i32.eqz (local.get $mask)))
                  (local.set $start
                    (i32.add (i32.add (local.get $at) (i32.ctz (local.get $mask))) (i32.const 1)))
                  (br_if $noted (i32.ge_u (local.get $start) (local.get $len)))
                  (br_if $failed (i32.ge_u (local.get $count) (global.get $MAX_LINES)))
                  (i64.store
                    (i32.add (global.get $records)
                      (i32.shl (local.get $count) (global.get $RECORD_SHIFT)))
                    (i64.extend_i32_u (local.get $start)))
                  (local.set $count (i32.add (local.get $count) (i32.const 1)))
                  ;; The lowest bit, done, comes off.
                  (local.set $mask
                    (i32.and (local.get $mask) (i32.sub (local.get $mask) (i32.const 1))))
                  (local.set $bits (i32.sub (local.get $bits) (i32.const 1)))
                  (br_if $lanes (local.get $bits))))))
          (local.set $at (i32.add (local.get $at) (global.get $CHUNK)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $vectors (local.get $left))))
      (local.set $result (local.get $count)))
    (if (i32.gt_s (local.get $result) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 8)) unreachable))
    (local.get $result))

  ;; Gives each record from place $lo to $hi the key of its line from byte $depth on, where its
  ;; line goes on to there: the line's next 4 bytes, the first the highest, those from its
  ;; newline on cleared, and how many of the 4 are the line's.
  (func $refresh (param $lo i32) (param $hi i32) (param $depth i32)
    (local $i i32)
    (local $at i32)
    (local $start i32)
    (local $word i32)
    (local $marks i32)
    (local $bytes v128)
    (local $left i32)
    (if (i32.gt_u (local.get $hi) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (if (i32.gt_u (local.get $depth) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 10)) unreachable))
    (local.set $i (local.get $lo))
    (block $done
      (local.set $left (global.get $MAX_LINES))
      (loop $each  ;; bound: 4194304
        (br_if $done (i32.ge_u (local.get $i) (local.get $hi)))
        (if (i32.ge_u (local.get $i) (global.get $MAX_LINES))
          (then (call $assert_fail (i32.const 11)) unreachable))
        (local.set $at
          (i32.add (global.get $records) (i32.shl (local.get $i) (global.get $RECORD_SHIFT))))
        (local.set $start
          (i32.and (i32.wrap_i64 (i64.load (local.get $at))) (global.get $START_MASK)))
        (local.set $word (i32.add (local.get $start) (local.get $depth)))
        (if (i32.gt_u (local.get $word) (global.get $LAST_WORD))
          (then (call $assert_fail (i32.const 12)) unreachable))
        (local.set $word (i32.load (i32.add (global.get $input) (local.get $word))))
        ;; The high bit of each byte that is a newline, the lowest exact.
        (local.set $marks (i32.xor (local.get $word) (global.get $NEWLINES)))
        (local.set $marks
          (i32.and
            (i32.and (i32.sub (local.get $marks) (global.get $ONES))
              (i32.xor (local.get $marks) (global.get $ALL_ONES)))
            (global.get $HIGHS)))
        ;; The bytes below the lowest mark, in every lane, which a shuffle puts first to last.
        (local.set $bytes
          (i32x4.splat
            (i32.and (local.get $word)
              (i32.sub
                (i32.shr_u
                  (i32.and (local.get $marks) (i32.sub (i32.const 0) (local.get $marks)))
                  (global.get $HIGH_BIT))
                (i32.const 1)))))
        (i64.store (local.get $at)
          (i64.or
            (i64.or
              (i64.shl
                (i64.extend_i32_u
                  (i32x4.extract_lane 0
                    (i8x16.shuffle 3 2 1 0 3 2 1 0 3 2 1 0 3 2 1 0
                      (local.get $bytes) (local.get $bytes))))
                (global.get $BYTES_SHIFT))
              (i64.shl
                (i64.extend_i32_u
                  (i32.shr_u (i32.ctz (local.get $marks)) (global.get $BYTE_SHIFT)))
                (global.get $KEY_SHIFT)))
            (i64.extend_i32_u (local.get $start))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $each (local.get $left)))))

  ;; The key of the median of three records from place $lo to $hi, which $seed picks: each of its
  ;; three parts of PICK_BITS bits, as a fraction of the records, the place of one.
  (func $pivot (param $lo i32) (param $hi i32) (param $seed i64) (result i64)
    (local $size i64)
    (local $at i32)
    (local $a i64)
    (local $b i64)
    (local $c i64)
    (local $low i64)
    (local $high i64)
    (if (i32.gt_u (local.get $hi) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 13)) unreachable))
    (if (i32.ge_u (local.get $lo) (local.get $hi))
      (then (call $assert_fail (i32.const 14)) unreachable))
    (local.set $size (i64.extend_i32_u (i32.sub (local.get $hi) (local.get $lo))))
    (local.set $at
      (i32.add (local.get $lo)
        (i32.wrap_i64
          (i64.shr_u (i64.mul (i64.and (local.get $seed) (global.get $PICK_MASK)) (local.get $size))
            (global.get $PICK_BITS)))))
    (if (i32.ge_u (local.get $at) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (local.set $a
      (i64.shr_u
        (i64.load
          (i32.add (global.get $records) (i32.shl (local.get $at) (global.get $RECORD_SHIFT))))
        (global.get $KEY_SHIFT)))
    (local.set $at
      (i32.add (local.get $lo)
        (i32.wrap_i64
          (i64.shr_u
            (i64.mul
              (i64.and (i64.shr_u (local.get $seed) (global.get $PICK_BITS))
                (global.get $PICK_MASK))
              (local.get $size))
            (global.get $PICK_BITS)))))
    (if (i32.ge_u (local.get $at) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 16)) unreachable))
    (local.set $b
      (i64.shr_u
        (i64.load
          (i32.add (global.get $records) (i32.shl (local.get $at) (global.get $RECORD_SHIFT))))
        (global.get $KEY_SHIFT)))
    (local.set $at
      (i32.add (local.get $lo)
        (i32.wrap_i64
          (i64.shr_u
            (i64.mul
              (i64.and (i64.shr_u (local.get $seed) (global.get $PICK_TWICE))
                (global.get $PICK_MASK))
              (local.get $size))
            (global.get $PICK_BITS)))))
    (if (i32.ge_u (local.get $at) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 17)) unreachable))
    (local.set $c
      (i64.shr_u
        (i64.load
          (i32.add (global.get $records) (i32.shl (local.get $at) (global.get $RECORD_SHIFT))))
        (global.get $KEY_SHIFT)))
    ;; The median: the larger of the smaller of a and b, and the smaller of the larger and c.
    (local.set $low
      (select (local.get $a) (local.get $b) (i64.lt_u (local.get $a) (local.get $b))))
    (local.set $high
      (select (local.get $b) (local.get $a) (i64.lt_u (local.get $a) (local.get $b))))
    (local.set $high
      (select (local.get $high) (local.get $c) (i64.lt_u (local.get $high) (local.get $c))))
    (select (local.get $high) (local.get $low) (i64.lt_u (local.get $low) (local.get $high))))

  ;; Moves the records from place $lo to $hi below $bound before the others, in one pass with no
  ;; branch on a record: each is swapped with the first not yet known to be below, and that place
  ;; moves on by one where it is. Gives where the others start.
  (func $split (param $lo i32) (param $hi i32) (param $bound i64) (result i32)
    (local $i i32)
    (local $front i32)
    (local $record i64)
    (local $left i32)
    (if (i32.gt_u (local.get $hi) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 18)) unreachable))
    (if (i32.gt_u (local.get $lo) (local.get $hi))
      (then (call $assert_fail (i32.const 19)) unreachable))
    (local.set $i (local.get $lo))
    (local.set $front (local.get $lo))
    (block $done
      (local.set $left (global.get $MAX_LINES))
      (loop $each  ;; bound: 4194304
        (br_if $done (i32.ge_u (local.get $i) (local.get $hi)))
        (if (i32.ge_u (local.get $i) (global.get $MAX_LINES))
          (then (call $assert_fail (i32.const 20)) unreachable))
        (if (i32.ge_u (local.get $front) (global.get $MAX_LINES))
          (then (call $assert_fail (i32.const 21)) unreachable))
        (local.set $record
          (i64.load
            (i32.add (global.get $records) (i32.shl (local.get $i) (global.get $RECORD_SHIFT)))))
        (i64.store
          (i32.add (global.get $records) (i32.shl (local.get $i) (global.get $RECORD_SHIFT)))
          (i64.load
            (i32.add (global.get $records)
              (i32.shl (local.get $front) (global.get $RECORD_SHIFT)))))
        (i64.store
          (i32.add (global.get $records) (i32.shl (local.get $front) (global.get $RECORD_SHIFT)))
          (local.get $record))
        (local.set $front
          (i32.add (local.get $front) (i64.lt_u (local.get $record) (local.get $bound))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $each (local.get $left))))
    (local.get $front))

  ;; Puts the part from record $lo to $hi, whose lines are the same up to byte $depth and which
  ;; may take $splits splits more, on the stack above its $top parts, where it has FEWEST records
  ;; or more. Gives how many parts the stack holds then.
  (func $push
    (param $lo i32) (param $hi i32) (param $depth i32) (param $splits i32) (param $top i32)
    (result i32)
    (local $at i32)
    (if (i32.gt_u (local.get $lo) (local.get $hi))
      (then (call $assert_fail (i32.const 22)) unreachable))
    (block $kept
      (br_if $kept (i32.lt_u (i32.sub (local.get $hi) (local.get $lo)) (global.get $FEWEST)))
      (if (i32.ge_u (local.get $top) (global.get $STACK_MAX))
        (then (call $assert_fail (i32.const 23)) unreachable))
      (local.set $at
        (i32.add (global.get $stack) (i32.shl (local.get $top) (global.get $STACK_SHIFT))))
      (i32.store (local.get $at) (local.get $lo))
      (i32.store offset=4 (local.get $at) (local.get $hi))
      (i32.store offset=8 (local.get $at) (local.get $depth))
      (i32.store offset=12 (local.get $at) (local.get $splits))
      (local.set $top (i32.add (local.get $top) (i32.const 1))))
    (local.get $top))

  ;; Puts on the stack above its $top parts the three a split of the records from $lo to $hi
  ;; made, the largest first: those below the pivot, from $lo to $lt, and those above it, from
  ;; $gt to $hi, whose lines are the same up to byte $depth, and those the same as it, from $lt
  ;; to $same, whose lines are the same KEY_BYTES bytes further. Each part taken off before the
  ;; largest is half its split or less, so the stack holds two parts for each halving at most.
  ;; The part split could take $splits splits: those below and above the pivot may take one
  ;; fewer, and those the same as it, which are split by a key of their next bytes, as many.
  ;; Gives how many parts the stack holds then.
  (func $push_parts
    (param $lo i32) (param $lt i32) (param $same i32) (param $gt i32) (param $hi i32)
    (param $depth i32) (param $splits i32) (param $top i32) (result i32)
    (local $deeper i32)
    (local $fewer i32)
    (local $below i32)
    (local $level i32)
    (local $above i32)
    (if (i32.gt_u (local.get $depth) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 24)) unreachable))
    (if (i32.gt_u (local.get $same) (local.get $gt))
      (then (call $assert_fail (i32.const 25)) unreachable))
    ;; A part that can take no split goes to the heap sort, never to a split.
    (if (i32.eqz (local.get $splits))
      (then (call $assert_fail (i32.const 37)) unreachable))
    (local.set $deeper (i32.add (local.get $depth) (global.get $KEY_BYTES)))
    (local.set $fewer (i32.sub (local.get $splits) (i32.const 1)))
    (local.set $below (i32.sub (local.get $lt) (local.get $lo)))
    (local.set $level (i32.sub (local.get $same) (local.get $lt)))
    (local.set $above (i32.sub (local.get $hi) (local.get $gt)))
    (if (i32.and (i32.ge_u (local.get $below) (local.get $level))
          (i32.ge_u (local.get $below) (local.get $above)))
      (then
        (local.set $top
          (call $push (local.get $lo) (local.get $lt) (local.get $depth) (local.get $fewer)
            (local.get $top)))
        (local.set $top
          (call $push (local.get $lt) (local.get $same) (local.get $deeper) (local.get $splits)
            (local.get $top)))
        (local.set $top
          (call $push (local.get $gt) (local.get $hi) (local.get $depth) (local.get $fewer)
            (local.get $top))))
      (else
        (if (i32.ge_u (local.get $level) (local.get $above))
          (then
            (local.set $top
              (call $push (local.get $lt) (local.get $same) (local.get $deeper)
                (local.get $splits) (local.get $top)))
            (local.set $top
              (call $push (local.get $lo) (local.get $lt) (local.get $depth) (local.get $fewer)
                (local.get $top)))
            (local.set $top
              (call $push (local.get $gt) (local.get $hi) (local.get $depth) (local.get $fewer)
                (local.get $top))))
          (else
            (local.set $top
              (call $push (local.get $gt) (local.get $hi) (local.get $depth) (local.get $fewer)
                (local.get $top)))
            (local.set $top
              (call $push (local.get $lo) (local.get $lt) (local.get $depth) (local.get $fewer)
                (local.get $top)))
            (local.set $top
              (call $push (local.get $lt) (local.get $same) (local.get $deeper)
                (local.get $splits) (local.get $top)))))))
    (local.get $top))

  ;; Sorts the $n records by their lines. Each step takes a part off the stack, and splits it
  ;; twice around its pivot's key: the records below it first, then those the same as it, then
  ;; those above it. The lines the same as the pivot go on with their next KEY_BYTES bytes where
  ;; all of its bytes are theirs, and end there where not: such lines are the same line. The
  ;; first part may take twice as many splits as $n has bits, and SPARE_SPLITS more, below or
  ;; above a pivot; a part that can take no more is sorted by a heap sort instead.
  (func $sort (param $n i32)
    (local $top i32)
    (local $at i32)
    (local $lo i32)
    (local $hi i32)
    (local $depth i32)
    (local $splits i32)
    (local $key i64)
    (local $lt i32)
    (local $gt i32)
    (local $same i32)
    (local $seed i64)
    (local $left i32)
    (if (i32.gt_u (local.get $n) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 26)) unreachable))
    (local.set $top
      (call $push (i32.const 0) (local.get $n) (i32.const 0)
        (i32.add
          (i32.shl (i32.sub (global.get $WORD_BITS) (i32.clz (local.get $n))) (i32.const 1))
          (global.get $SPARE_SPLITS))
        (i32.const 0)))
    (local.set $seed (global.get $SEED))
    (block $sorted
      (local.set $left (global.get $MAX_STEPS))
      (loop $step  ;; bound: 2147483647
        (br_if $sorted (i32.eqz (local.get $top)))
        (local.set $top (i32.sub (local.get $top) (i32.const 1)))
        (if (i32.ge_u (local.get $top) (global.get $STACK_MAX))
          (then (call $assert_fail (i32.const 27)) unreachable))
        (local.set $at
          (i32.add (global.get $stack) (i32.shl (local.get $top) (global.get $STACK_SHIFT))))
        (local.set $lo (i32.load (local.get $at)))
        (local.set $hi (i32.load offset=4 (local.get $at)))
        (local.set $depth (i32.load offset=8 (local.get $at)))
        (local.set $splits (i32.load offset=12 (local.get $at)))
        (if (i32.eqz (local.get $splits))
          (then (call $heap_sort (local.get $lo) (local.get $hi) (local.get $depth)))
          (else
            ;; xorshift: the seed's next value.
            (local.set $seed
              (i64.xor (local.get $seed) (i64.shl (local.get $seed) (global.get $XOR_FIRST))))
            (local.set $seed
              (i64.xor (local.get $seed) (i64.shr_u (local.get $seed) (global.get $XOR_SECOND))))
            (local.set $seed
              (i64.xor (local.get $seed) (i64.shl (local.get $seed) (global.get $XOR_THIRD))))
            (local.set $key (call $pivot (local.get $lo) (local.get $hi) (local.get $seed)))
            (local.set $lt
              (call $split (local.get $lo) (local.get $hi)
                (i64.shl (local.get $key) (global.get $KEY_SHIFT))))
            (local.set $gt
              (call $split (local.get $lt) (local.get $hi)
                (i64.shl (i64.add (local.get $key) (i64.const 1)) (global.get $KEY_SHIFT))))
            (local.set $same (local.get $lt))
            (if (i32.eq (i32.and (i32.wrap_i64 (local.get $key)) (global.get $COUNT_MASK))
                  (global.get $KEY_BYTES))
              (then
                (if (i32.ge_u (i32.sub (local.get $gt) (local.get $lt)) (global.get $FEWEST))
                  (then
                    (call $refresh (local.get $lt) (local.get $gt)
                      (i32.add (local.get $depth) (global.get $KEY_BYTES)))
                    (local.set $same (local.get $gt))))))
            (local.set $top
              (call $push_parts (local.get $lo) (local.get $lt) (local.get $same)
                (local.get $gt) (local.get $hi) (local.get $depth) (local.get $splits)
                (local.get $top)))))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $step (local.get $left)))
      ;; A step takes a part off the stack, and every part is a different run of records at a
      ;; different depth, of lines of 64 MiB in all: far fewer than MAX_STEPS.
      (call $assert_fail (i32.const 28))
      unreachable))

  ;; Sorts the records from place $lo to $hi by their lines with a heap sort, in place: their
  ;; lines are the same before byte $depth, and their keys are from there. First a heap, where no
  ;; record's line comes after its parent's; then, one at a time, the first record, whose line
  ;; comes last of the heap's, swaps with the heap's last, which the heap then leaves.
  (func $heap_sort (param $lo i32) (param $hi i32) (param $depth i32)
    (local $n i32)
    (local $i i32)
    (local $last i32)
    (local $back i32)
    (local $front i32)
    (local $record i64)
    (local $left i32)
    (if (i32.gt_u (local.get $hi) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 38)) unreachable))
    (if (i32.gt_u (local.get $lo) (local.get $hi))
      (then (call $assert_fail (i32.const 39)) unreachable))
    (local.set $n (i32.sub (local.get $hi) (local.get $lo)))
    ;; Each place with a child sifted down, from the last.
    (local.set $i (i32.shr_u (local.get $n) (i32.const 1)))
    (block $heaped
      (local.set $left (global.get $HALF_LINES))
      (loop $parent  ;; bound: 2097152
        (br_if $heaped (i32.eqz (local.get $i)))
        (local.set $i (i32.sub (local.get $i) (i32.const 1)))
        (call $sift (local.get $lo) (local.get $n) (local.get $i) (local.get $depth))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $parent (local.get $left))))
    (local.set $i (local.get $n))
    (block $sorted
      (local.set $left (global.get $MAX_LINES))
      (loop $largest  ;; bound: 4194304
        (br_if $sorted (i32.le_u (local.get $i) (i32.const 1)))
        (local.set $i (i32.sub (local.get $i) (i32.const 1)))
        (local.set $last (i32.add (local.get $lo) (local.get $i)))
        (if (i32.ge_u (local.get $last) (global.get $MAX_LINES))
          (then (call $assert_fail (i32.const 40)) unreachable))
        (if (i32.ge_u (local.get $lo) (global.get $MAX_LINES))
          (then (call $assert_fail (i32.const 41)) unreachable))
        (local.set $back
          (i32.add (global.get $records) (i32.shl (local.get $last) (global.get $RECORD_SHIFT))))
        (local.set $front
          (i32.add (global.get $records) (i32.shl (local.get $lo) (global.get $RECORD_SHIFT))))
        (local.set $record (i64.load (local.get $back)))
        (i64.store (local.get $back) (i64.load (local.get $front)))
        (i64.store (local.get $front) (local.get $record))
        (call $sift (local.get $lo) (local.get $i) (i32.const 0) (local.get $depth))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $largest (local.get $left)))))

  ;; Sifts the record at place $i of the heap of the $n records from place $lo down: below its
  ;; child whose line comes later, the later of two, while that line comes after its own. Their
  ;; lines are the same before byte $depth, and their keys are from there.
  (func $sift (param $lo i32) (param $n i32) (param $i i32) (param $depth i32)
    (local $record i64)
    (local $larger i64)
    (local $other i64)
    (local $child i32)
    (local $at i32)
    (local $left i32)
    (if (i32.gt_u (local.get $n) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 42)) unreachable))
    (if (i32.ge_u (local.get $i) (local.get $n))
      (then (call $assert_fail (i32.const 43)) unreachable))
    (local.set $at (i32.add (local.get $lo) (local.get $i)))
    (if (i32.ge_u (local.get $at) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 44)) unreachable))
    (local.set $record
      (i64.load
        (i32.add (global.get $records) (i32.shl (local.get $at) (global.get $RECORD_SHIFT)))))
    (block $placed
      (local.set $left (global.get $HEAP_LEVELS))
      (loop $level  ;; bound: 23
        ;; Place $i's first child, where it has one.
        (local.set $child (i32.add (i32.shl (local.get $i) (i32.const 1)) (i32.const 1)))
        (br_if $placed (i32.ge_u (local.get $child) (local.get $n)))
        (local.set $at (i32.add (local.get $lo) (local.get $child)))
        (if (i32.ge_u (local.get $at) (global.get $MAX_LINES))
          (then (call $assert_fail (i32.const 45)) unreachable))
        (local.set $larger
          (i64.load
            (i32.add (global.get $records) (i32.shl (local.get $at) (global.get $RECORD_SHIFT)))))
        ;; Its second child instead, where it has one whose line comes after the first's.
        (if (i32.lt_u (i32.add (local.get $child) (i32.const 1)) (local.get $n))
          (then
            (local.set $at (i32.add (local.get $at) (i32.const 1)))
            (if (i32.ge_u (local.get $at) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 46)) unreachable))
            (local.set $other
              (i64.load
                (i32.add (global.get $records)
                  (i32.shl (local.get $at) (global.get $RECORD_SHIFT)))))
            (if (call $before (local.get $larger) (local.get $other) (local.get $depth))
              (then
                (local.set $larger (local.get $other))
                (local.set $child (i32.add (local.get $child) (i32.const 1)))))))
        ;; The record stays at place $i where the child's line doesn't come after its own.
        (br_if $placed
          (i32.eqz (call $before (local.get $record) (local.get $larger) (local.get $depth))))
        ;; Else the child's record moves up to place $i, and the record goes on from the child's.
        (local.set $at (i32.add (local.get $lo) (local.get $i)))
        (if (i32.ge_u (local.get $at) (global.get $MAX_LINES))
          (then (call $assert_fail (i32.const 47)) unreachable))
        (i64.store
          (i32.add (global.get $records) (i32.shl (local.get $at) (global.get $RECORD_SHIFT)))
          (local.get $larger))
        (local.set $i (local.get $child))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $level (local.get $left)))
      ;; Each level more than doubles the place, and a heap of MAX_LINES records has HEAP_LEVELS.
      (call $assert_fail (i32.const 48))
      unreachable)
    (local.set $at (i32.add (local.get $lo) (local.get $i)))
    (if (i32.ge_u (local.get $at) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 49)) unreachable))
    (i64.store
      (i32.add (global.get $records) (i32.shl (local.get $at) (global.get $RECORD_SHIFT)))
      (local.get $record)))

  ;; Whether the line of record $a comes before the line of record $b: their bytes before $depth
  ;; are the same, and their keys are from there. The keys decide, then those of the next
  ;; KEY_BYTES bytes, while both are the same and hold neither line's end. Gives 1 where it comes
  ;; before, else 0.
  (func $before (param $a i64) (param $b i64) (param $depth i32) (result i32)
    (local $x i64)
    (local $y i64)
    (local $at i32)
    (local $left i32)
    (if (i32.gt_u (local.get $depth) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 50)) unreachable))
    (local.set $x (i64.shr_u (local.get $a) (global.get $KEY_SHIFT)))
    (local.set $y (i64.shr_u (local.get $b) (global.get $KEY_SHIFT)))
    (local.set $at (local.get $depth))
    (block $decided
      (local.set $left (global.get $MAX_KEYS))
      (loop $key  ;; bound: 16777217
        (br_if $decided (i64.ne (local.get $x) (local.get $y)))
        ;; The same key, which holds the end of both lines: the same line.
        (br_if $decided
          (i32.ne (i32.and (i32.wrap_i64 (local.get $x)) (global.get $COUNT_MASK))
            (global.get $KEY_BYTES)))
        (local.set $at (i32.add (local.get $at) (global.get $KEY_BYTES)))
        (local.set $x
          (call $key_at (i32.and (i32.wrap_i64 (local.get $a)) (global.get $START_MASK))
            (local.get $at)))
        (local.set $y
          (call $key_at (i32.and (i32.wrap_i64 (local.get $b)) (global.get $START_MASK))
            (local.get $at)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $key (local.get $left)))
      ;; Lines the same for MAX_KEYS keys would be longer than the input.
      (call $assert_fail (i32.const 51))
      unreachable)
    (i64.lt_u (local.get $x) (local.get $y)))

  ;; The key of the line that starts at byte $start of the input, from its byte $depth on, where
  ;; it goes on to there, as a record shifted down by KEY_SHIFT holds it: the line's next 4 bytes,
  ;; the first the highest, those from its newline on cleared, then how many of the 4 are the
  ;; line's, as refresh makes it.
  (func $key_at (param $start i32) (param $depth i32) (result i64)
    (local $word i32)
    (local $marks i32)
    (local $bytes v128)
    (if (i32.gt_u (local.get $start) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 52)) unreachable))
    (if (i32.gt_u (local.get $depth) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 53)) unreachable))
    (local.set $word (i32.add (local.get $start) (local.get $depth)))
    (if (i32.gt_u (local.get $word) (global.get $LAST_WORD))
      (then (call $assert_fail (i32.const 54)) unreachable))
    (local.set $word (i32.load (i32.add (global.get $input) (local.get $word))))
    ;; The high bit of each byte that is a newline, the lowest exact.
    (local.set $marks (i32.xor (local.get $word) (global.get $NEWLINES)))
    (local.set $marks
      (i32.and
        (i32.and (i32.sub (local.get $marks) (global.get $ONES))
          (i32.xor (local.get $marks) (global.get $ALL_ONES)))
        (global.get $HIGHS)))
    ;; The bytes below the lowest mark, in every lane, which a shuffle puts first to last.
    (local.set $bytes
      (i32x4.splat
        (i32.and (local.get $word)
          (i32.sub
            (i32.shr_u
              (i32.and (local.get $marks) (i32.sub (i32.const 0) (local.get $marks)))
              (global.get $HIGH_BIT))
            (i32.const 1)))))
    (i64.shr_u
      (i64.or
        (i64.shl
          (i64.extend_i32_u
            (i32x4.extract_lane 0
              (i8x16.shuffle 3 2 1 0 3 2 1 0 3 2 1 0 3 2 1 0
                (local.get $bytes) (local.get $bytes))))
          (global.get $BYTES_SHIFT))
        (i64.shl
          (i64.extend_i32_u (i32.shr_u (i32.ctz (local.get $marks)) (global.get $BYTE_SHIFT)))
          (global.get $KEY_SHIFT)))
      (global.get $KEY_SHIFT)))

  ;; Prints the lines of the $n records in their order, through the output. Gives 0, or the error
  ;; of a write that failed.
  (func $emit (param $n i32) (result i32)  ;; returns: result
    (local $k i32)
    (local $taken i32)
    (local $err i32)
    (local $left i32)
    (local.set $err (global.get $NO_ERROR))
    (if (i32.gt_u (local.get $n) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 29)) unreachable))
    (block $failed
      (block $done
        (local.set $left (global.get $MAX_LINES))
        (loop $line  ;; bound: 4194304
          (br_if $done (i32.ge_u (local.get $k) (local.get $n)))
          (if (i32.ge_u (local.get $k) (global.get $MAX_LINES))
            (then (call $assert_fail (i32.const 30)) unreachable))
          (call $put_line
            (i32.and
              (i32.wrap_i64
                (i64.load
                  (i32.add (global.get $records)
                    (i32.shl (local.get $k) (global.get $RECORD_SHIFT)))))
              (global.get $START_MASK))
            (local.get $taken))
          (local.set $taken)
          (local.set $err)
          (br_if $failed (local.get $err))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $line (local.get $left))))
      ;; What the output holds at the end.
      (if (i32.gt_u (local.get $taken) (global.get $out.size))
        (then (call $assert_fail (i32.const 31)) unreachable))
      (local.set $err (call $write_all (global.get $STDOUT) (global.get $out) (local.get $taken)))
      (br_if $failed (local.get $err)))
    (local.get $err))

  ;; Copies the line that starts at byte $start of the input, its newline included, to the output
  ;; from byte $taken, 16 bytes a step, writing what the output holds whenever 16 bytes more may
  ;; not fit. The bytes a step copies past the newline are the next line's to write over. Gives 0
  ;; or the error of a write that failed, and how many bytes the output holds then.
  (func $put_line (param $start i32) (param $taken i32) (result i32 i32)
    (local $at i32)
    (local $chunk v128)
    (local $newlines v128)
    (local $err i32)
    (local $left i32)
    (if (i32.gt_u (local.get $start) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 32)) unreachable))
    (local.set $newlines (i8x16.splat (global.get $NEWLINE)))
    (local.set $at (local.get $start))
    (block $failed
      (block $done
        (local.set $left (global.get $VECTORS))
        (loop $vectors  ;; bound: 4194306
          ;; The output takes the next 16 bytes, or goes out first.
          (if (i32.gt_u (local.get $taken) (global.get $LAST_OUT))
            (then
              ;; What the output holds, which is never more than its size.
              (if (i32.gt_u (local.get $taken) (global.get $out.size))
                (then (call $assert_fail (i32.const 33)) unreachable))
              (local.set $err
                (call $write_all (global.get $STDOUT) (global.get $out) (local.get $taken)))
              (br_if $failed (local.get $err))
              (local.set $taken (i32.const 0))))
          (if (i32.gt_u (local.get $taken) (global.get $LAST_OUT))
            (then (call $assert_fail (i32.const 34)) unreachable))
          (if (i32.gt_u (local.get $at) (global.get $LAST_VECTOR))
            (then (call $assert_fail (i32.const 35)) unreachable))
          (local.set $chunk (v128.load (i32.add (global.get $input) (local.get $at))))
          (v128.store (i32.add (global.get $out) (local.get $taken)) (local.get $chunk))
          (local.set $chunk (i8x16.eq (local.get $chunk) (local.get $newlines)))
          ;; The line's newline: its bytes are those up to it.
          (if (v128.any_true (local.get $chunk))
            (then
              (local.set $taken
                (i32.add (local.get $taken)
                  (i32.add (i32.ctz (i8x16.bitmask (local.get $chunk))) (i32.const 1))))
              (br $done)))
          (local.set $taken (i32.add (local.get $taken) (global.get $CHUNK)))
          (local.set $at (i32.add (local.get $at) (global.get $CHUNK)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $vectors (local.get $left)))
        ;; Every line ends with a newline before the input's end.
        (call $assert_fail (i32.const 36))
        unreachable))
    (local.get $err)
    (local.get $taken))
)
