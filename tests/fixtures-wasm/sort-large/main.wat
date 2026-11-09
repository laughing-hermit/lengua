;; sort-large: sorts the lines of standard input of up to 64 MiB in byte order, as LC_ALL=C sort
;; does. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md): lengua's library
;; reads the whole input with $fill; the place where each line starts goes in an array of 4 bytes
;; a line, found 16 bytes at a time from a bitmask of the newlines; a bottom-up merge sort orders
;; the array, comparing two lines 16 bytes at a time, where the first lane in which they differ
;; or one of them ends decides; and the lines go out through a 1 MiB buffer.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; 1553 pages: lengua's first 1024 bytes, then the regions below, which end at 101717184.
  (memory (export "memory") 1553 1553)

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
  ;; Vectors of 16 bytes: the most that pass the input, where the last starts, and a lane's
  ;; place in a vector.
  (global $CHUNK i32 (i32.const 16))
  (global $VECTORS i32 (i32.const 4194306))
  (global $LAST_VECTOR i32 (i32.const 67113008))
  (global $LANE_MASK i32 (i32.const 15))
  ;; Sorting: a line's start of 4 bytes; a merge sort's runs double 22 times at most for
  ;; MAX_LINES lines, and a round makes two of those passes.
  (global $INDEX_SHIFT i32 (i32.const 2))
  (global $MAX_ROUNDS i32 (i32.const 12))
  ;; Printing: a line goes to the output in parts of COPY_CHUNK bytes at most, the most parts a
  ;; line takes, and the last place a part starts in the output.
  (global $COPY_CHUNK i32 (i32.const 4096))
  (global $MAX_PARTS i32 (i32.const 16385))
  (global $LAST_COPY i32 (i32.const 1044480))
  (global $NEWLINE i32 (i32.const 10))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then the input, the lines'
  ;; starts, the merge sort's second array and the output.
  (global $too_long i32 (i32.const 1024))
  (global $too_long.rosize i32 (i32.const 40))
  (data (i32.const 1024) "error: input longer than 67108864 bytes\0a")
  (global $too_many i32 (i32.const 1088))
  (global $too_many.rosize i32 (i32.const 31))
  (data (i32.const 1088) "error: more than 4194304 lines\0a")
  ;; Standard input: READ_ROOM bytes, then room for a part and a vector more, so every part
  ;; copied and every vector loaded from a place inside the input stays inside.
  (global $input i32 (i32.const 1152))  ;; shared: main fills it, and the lines are read from it
  (global $input.size i32 (i32.const 67113024))
  ;; Where each line starts, MAX_LINES of them.
  (global $starts i32 (i32.const 67114176))  ;; shared: index_lines fills it, merge_sort sorts it
  (global $starts.size i32 (i32.const 16777216))
  ;; The merge sort's second array of starts.
  (global $scratch i32 (i32.const 83891392))
  (global $scratch.size i32 (i32.const 16777216))
  ;; The lines it prints, a buffer-full at a time.
  (global $out i32 (i32.const 100668608))  ;; shared: put_line fills it, emit prints the rest
  (global $out.size i32 (i32.const 1048576))

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
            (br_if $write_failed
              (call $emit (call $merge_sort (local.get $lines)) (local.get $lines)))
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

  ;; Writes where each line of the input's first $len bytes starts, in order, to the starts: the
  ;; first at 0, and each other right after a newline, which the bitmask of each 16 bytes' newline
  ;; lanes gives, the lowest first. The bytes end with a newline, and the bytes past them are the
  ;; zeros memory starts with, which no newline is. Gives how many lines there are, or TOO_MANY.
  (func $index_lines (param $len i32) (result i32)
    (local $at i32)
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
        (i32.store (global.get $starts) (i32.const 0))
        (local.set $count (i32.const 1))
        (local.set $left (global.get $VECTORS))
        (loop $vectors  ;; bound: 4194306
          (br_if $done (i32.ge_u (local.get $at) (local.get $len)))
          (if (i32.gt_u (local.get $at) (global.get $LAST_VECTOR))
            (then (call $assert_fail (i32.const 7)) unreachable))
          (local.set $mask
            (i8x16.bitmask
              (i8x16.eq (v128.load (i32.add (global.get $input) (local.get $at)))
                (local.get $newlines))))
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
              (i32.store
                (i32.add (global.get $starts) (i32.shl (local.get $count) (global.get $INDEX_SHIFT)))
                (local.get $start))
              (local.set $count (i32.add (local.get $count) (i32.const 1)))
              ;; The lowest bit, done, comes off.
              (local.set $mask (i32.and (local.get $mask) (i32.sub (local.get $mask) (i32.const 1))))
              (local.set $bits (i32.sub (local.get $bits) (i32.const 1)))
              (br_if $lanes (local.get $bits))))
          (local.set $at (i32.add (local.get $at) (global.get $CHUNK)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $vectors (local.get $left))))
      (local.set $result (local.get $count)))
    (if (i32.gt_s (local.get $result) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 8)) unreachable))
    (local.get $result))

  ;; The length of the line that starts at byte $start of the input, its newline included, found
  ;; 16 bytes at a time: every line ends with a newline.
  (func $line_length (param $start i32) (result i32)
    (local $at i32)
    (local $mask i32)
    (local $length i32)
    (local $newlines v128)
    (local $left i32)
    (if (i32.gt_u (local.get $start) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (local.set $newlines (i8x16.splat (global.get $NEWLINE)))
    (local.set $at (local.get $start))
    (block $found
      (local.set $left (global.get $VECTORS))
      (loop $vectors  ;; bound: 4194306
        (if (i32.gt_u (local.get $at) (global.get $LAST_VECTOR))
          (then (call $assert_fail (i32.const 10)) unreachable))
        (local.set $mask
          (i8x16.bitmask
            (i8x16.eq (v128.load (i32.add (global.get $input) (local.get $at)))
              (local.get $newlines))))
        (if (local.get $mask)
          (then
            (local.set $length
              (i32.sub (i32.add (i32.add (local.get $at) (i32.ctz (local.get $mask))) (i32.const 1))
                (local.get $start)))
            (br $found)))
        (local.set $at (i32.add (local.get $at) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $vectors (local.get $left)))
      ;; Every line ends with a newline before the input's end.
      (call $assert_fail (i32.const 11))
      unreachable)
    (local.get $length))

  ;; Compares the lines that start at bytes $a and $b of the input in byte order, a line that
  ;; starts the other coming first, 16 bytes at a time: the first lane in which the bytes differ,
  ;; or in which either line ends, decides. A line that ends there comes first; two that end there
  ;; are the same. Gives a negative number where the first comes first, 0 where they're the same,
  ;; and a positive one where the second does.
  (func $compare_lines (param $a i32) (param $b i32) (result i32)
    (local $pa i32)
    (local $pb i32)
    (local $stop i32)
    (local $lane i32)
    (local $x i32)
    (local $y i32)
    (local $order i32)
    (local $va v128)
    (local $vb v128)
    (local $newlines v128)
    (local $left i32)
    (local.set $newlines (i8x16.splat (global.get $NEWLINE)))
    (local.set $pa (local.get $a))
    (local.set $pb (local.get $b))
    (block $decided
      (local.set $left (global.get $VECTORS))
      (loop $vectors  ;; bound: 4194306
        (if (i32.gt_u (local.get $pa) (global.get $LAST_VECTOR))
          (then (call $assert_fail (i32.const 12)) unreachable))
        (if (i32.gt_u (local.get $pb) (global.get $LAST_VECTOR))
          (then (call $assert_fail (i32.const 13)) unreachable))
        (local.set $va (v128.load (i32.add (global.get $input) (local.get $pa))))
        (local.set $vb (v128.load (i32.add (global.get $input) (local.get $pb))))
        (local.set $stop
          (i8x16.bitmask
            (v128.or (i8x16.ne (local.get $va) (local.get $vb))
              (v128.or (i8x16.eq (local.get $va) (local.get $newlines))
                (i8x16.eq (local.get $vb) (local.get $newlines))))))
        (if (local.get $stop)
          (then
            ;; A bitmask of 16 lanes that isn't 0 has its lowest bit below 16.
            (local.set $lane (i32.and (i32.ctz (local.get $stop)) (global.get $LANE_MASK)))
            (local.set $x
              (i32.load8_u (i32.add (global.get $input) (i32.add (local.get $pa) (local.get $lane)))))
            (local.set $y
              (i32.load8_u (i32.add (global.get $input) (i32.add (local.get $pb) (local.get $lane)))))
            (local.set $order
              (select
                (i32.sub (i32.eq (local.get $y) (global.get $NEWLINE))
                  (i32.eq (local.get $x) (global.get $NEWLINE)))
                (i32.sub (i32.gt_u (local.get $x) (local.get $y))
                  (i32.lt_u (local.get $x) (local.get $y)))
                (i32.or (i32.eq (local.get $x) (global.get $NEWLINE))
                  (i32.eq (local.get $y) (global.get $NEWLINE)))))
            (br $decided)))
        (local.set $pa (i32.add (local.get $pa) (global.get $CHUNK)))
        (local.set $pb (i32.add (local.get $pb) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $vectors (local.get $left)))
      ;; Every line ends with a newline before the input's end.
      (call $assert_fail (i32.const 14))
      unreachable)
    (local.get $order))

  ;; Merges the sorted runs $src[lo, mid) and $src[mid, hi) into $dst[lo, hi), each a line's start
  ;; of 4 bytes: merge_pass lends it the two arrays, MAX_LINES starts each.
  (func $merge (param $src i32) (param $dst i32) (param $lo i32) (param $mid i32) (param $hi i32)
    (local $i i32)
    (local $j i32)
    (local $k i32)
    (local $from_left i32)
    (local $taken i32)
    (local $left i32)
    (if (i32.gt_u (local.get $lo) (local.get $mid))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (if (i32.gt_u (local.get $mid) (local.get $hi))
      (then (call $assert_fail (i32.const 16)) unreachable))
    (local.set $i (local.get $lo))
    (local.set $j (local.get $mid))
    (local.set $k (local.get $lo))
    (block $done
      (local.set $left (global.get $MAX_LINES))
      (loop $line  ;; bound: 4194304
        (br_if $done (i32.ge_u (local.get $k) (local.get $hi)))
        ;; The left run's line, while it has one and the right run's isn't earlier.
        (local.set $from_left (i32.lt_u (local.get $i) (local.get $mid)))
        (if (i32.and (local.get $from_left) (i32.lt_u (local.get $j) (local.get $hi)))
          (then
            (if (i32.ge_u (local.get $i) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 17)) unreachable))
            (if (i32.ge_u (local.get $j) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 18)) unreachable))
            (local.set $from_left
              (i32.le_s
                (call $compare_lines
                  (i32.load
                    (i32.add (local.get $src) (i32.shl (local.get $i) (global.get $INDEX_SHIFT))))
                  (i32.load
                    (i32.add (local.get $src) (i32.shl (local.get $j) (global.get $INDEX_SHIFT)))))
                (i32.const 0)))))
        (if (local.get $from_left)
          (then
            (if (i32.ge_u (local.get $i) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 19)) unreachable))
            (local.set $taken
              (i32.load (i32.add (local.get $src) (i32.shl (local.get $i) (global.get $INDEX_SHIFT)))))
            (local.set $i (i32.add (local.get $i) (i32.const 1))))
          (else
            (if (i32.ge_u (local.get $j) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 20)) unreachable))
            (local.set $taken
              (i32.load (i32.add (local.get $src) (i32.shl (local.get $j) (global.get $INDEX_SHIFT)))))
            (local.set $j (i32.add (local.get $j) (i32.const 1)))))
        (if (i32.ge_u (local.get $k) (global.get $MAX_LINES))
          (then (call $assert_fail (i32.const 21)) unreachable))
        (i32.store (i32.add (local.get $dst) (i32.shl (local.get $k) (global.get $INDEX_SHIFT)))
          (local.get $taken))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $line (local.get $left)))))

  ;; Merges each pair of neighboring sorted runs of $width starts from $src into $dst, the $n
  ;; starts of the two arrays merge_sort lends it.
  (func $merge_pass (param $src i32) (param $dst i32) (param $n i32) (param $width i32)
    (local $lo i32)
    (local $mid i32)
    (local $hi i32)
    (local $left i32)
    (if (i32.gt_u (local.get $n) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 22)) unreachable))
    ;; A run is never empty, and never longer than the starts, so lo + width can't wrap.
    (if (i32.eqz (local.get $width))
      (then (call $assert_fail (i32.const 23)) unreachable))
    (if (i32.gt_u (local.get $width) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 24)) unreachable))
    (block $done
      (local.set $left (global.get $MAX_LINES))
      (loop $pair  ;; bound: 4194304
        (br_if $done (i32.ge_u (local.get $lo) (local.get $n)))
        (local.set $mid (i32.add (local.get $lo) (local.get $width)))
        (if (i32.gt_u (local.get $mid) (local.get $n)) (then (local.set $mid (local.get $n))))
        (local.set $hi (i32.add (local.get $mid) (local.get $width)))
        (if (i32.gt_u (local.get $hi) (local.get $n)) (then (local.set $hi (local.get $n))))
        (call $merge (local.get $src) (local.get $dst) (local.get $lo) (local.get $mid)
          (local.get $hi))
        (local.set $lo (local.get $hi))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $pair (local.get $left)))))

  ;; Sorts the $n starts with a bottom-up merge sort, back and forth between the starts and the
  ;; scratch array, two passes a round, so each pass's arrays are named, and gives the array that
  ;; holds them sorted.
  (func $merge_sort (param $n i32) (result i32)
    (local $width i32)
    (local $sorted i32)
    (local $left i32)
    (if (i32.gt_u (local.get $n) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 25)) unreachable))
    (local.set $width (i32.const 1))
    (block $done
      (local.set $sorted (global.get $starts))
      (local.set $left (global.get $MAX_ROUNDS))
      (loop $round  ;; bound: 12
        (br_if $done (i32.ge_u (local.get $width) (local.get $n)))
        (call $merge_pass (global.get $starts) (global.get $scratch) (local.get $n)
          (local.get $width))
        (local.set $width (i32.shl (local.get $width) (i32.const 1)))
        (local.set $sorted (global.get $scratch))
        (br_if $done (i32.ge_u (local.get $width) (local.get $n)))
        (call $merge_pass (global.get $scratch) (global.get $starts) (local.get $n)
          (local.get $width))
        (local.set $width (i32.shl (local.get $width) (i32.const 1)))
        (local.set $sorted (global.get $starts))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $round (local.get $left)))
      ;; 12 rounds sort 2 to the 24th starts, more than MAX_LINES.
      (call $assert_fail (i32.const 26))
      unreachable)
    (local.get $sorted))

  ;; Prints the $n lines whose starts are at $sorted, which main lends it, in that order, through
  ;; the output. Gives 0, or the error of a write that failed.
  (func $emit (param $sorted i32) (param $n i32) (result i32)  ;; returns: result
    (local $k i32)
    (local $taken i32)
    (local $err i32)
    (local $left i32)
    (local.set $err (global.get $NO_ERROR))
    (if (i32.gt_u (local.get $n) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 27)) unreachable))
    (block $failed
      (block $done
        (local.set $left (global.get $MAX_LINES))
        (loop $line  ;; bound: 4194304
          (br_if $done (i32.ge_u (local.get $k) (local.get $n)))
          (if (i32.ge_u (local.get $k) (global.get $MAX_LINES))
            (then (call $assert_fail (i32.const 28)) unreachable))
          (call $put_line
            (i32.load (i32.add (local.get $sorted) (i32.shl (local.get $k) (global.get $INDEX_SHIFT))))
            (local.get $taken))
          (local.set $taken)
          (local.set $err)
          (br_if $failed (local.get $err))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $line (local.get $left))))
      ;; What the output holds at the end.
      (if (i32.gt_u (local.get $taken) (global.get $out.size))
        (then (call $assert_fail (i32.const 29)) unreachable))
      (local.set $err (call $write_all (global.get $STDOUT) (global.get $out) (local.get $taken)))
      (br_if $failed (local.get $err)))
    (local.get $err))

  ;; Copies the line that starts at byte $start of the input, its newline included, to the output
  ;; from byte $taken, COPY_CHUNK bytes at most at a time, writing what the output holds whenever
  ;; the next part may not fit. Gives 0 or the error of a write that failed, and how many bytes
  ;; the output holds then.
  (func $put_line (param $start i32) (param $taken i32) (result i32 i32)
    (local $length i32)
    (local $copied i32)
    (local $part i32)
    (local $at i32)
    (local $err i32)
    (local $left i32)
    (if (i32.gt_u (local.get $start) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 30)) unreachable))
    (local.set $length (call $line_length (local.get $start)))
    (if (i32.gt_u (local.get $length) (global.get $READ_ROOM))
      (then (call $assert_fail (i32.const 31)) unreachable))
    (block $failed
      (block $done
        (local.set $left (global.get $MAX_PARTS))
        (loop $part  ;; bound: 16385
          (br_if $done (i32.ge_u (local.get $copied) (local.get $length)))
          ;; The output takes the next part, or goes out first.
          (if (i32.gt_u (local.get $taken) (global.get $LAST_COPY))
            (then
              ;; What the output holds, which is never more than its size.
              (if (i32.gt_u (local.get $taken) (global.get $out.size))
                (then (call $assert_fail (i32.const 35)) unreachable))
              (local.set $err
                (call $write_all (global.get $STDOUT) (global.get $out) (local.get $taken)))
              (br_if $failed (local.get $err))
              (local.set $taken (i32.const 0))))
          (if (i32.gt_u (local.get $taken) (global.get $LAST_COPY))
            (then (call $assert_fail (i32.const 32)) unreachable))
          (local.set $part (i32.sub (local.get $length) (local.get $copied)))
          (local.set $part
            (select (local.get $part) (global.get $COPY_CHUNK)
              (i32.lt_u (local.get $part) (global.get $COPY_CHUNK))))
          (if (i32.gt_u (local.get $part) (global.get $COPY_CHUNK))
            (then (call $assert_fail (i32.const 33)) unreachable))
          (local.set $at (i32.add (local.get $start) (local.get $copied)))
          (if (i32.gt_u (local.get $at) (global.get $READ_ROOM))
            (then (call $assert_fail (i32.const 34)) unreachable))
          (memory.copy (i32.add (global.get $out) (local.get $taken))
            (i32.add (global.get $input) (local.get $at)) (local.get $part))
          (local.set $taken (i32.add (local.get $taken) (local.get $part)))
          (local.set $copied (i32.add (local.get $copied) (local.get $part)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $part (local.get $left)))))
    (local.get $err)
    (local.get $taken))
)
