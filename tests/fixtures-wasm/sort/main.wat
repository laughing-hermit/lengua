;; sort: sorts the lines of standard input in byte order, as LC_ALL=C sort does, and with -u
;; keeps one of each. The WebAssembly twin of examples/sort, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): the input read whole, each line an entry of its start
;; and its length, the entries sorted by tally-template.wat's bottom-up merge sort, and the lines
;; printed in one write. Lines are found, compared and copied 16 bytes at a time with 128-bit
;; vectors: a compare's bitmask has a bit for each lane, and its lowest set bit is the first
;; newline, or the first byte where two lines differ.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; 289 pages, 18939904 bytes: lengua's first 1024 bytes and the regions below, which end at
  ;; 18875904.
  (memory (export "memory") 289 289)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_WRITE_FAILED i32 (i32.const 2))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  (global $EXIT_TOO_LONG i32 (i32.const 4))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The argument counts, the program's name included: none, or the option; and the size of a
  ;; count args_sizes_get writes.
  (global $ARGC_PLAIN i32 (i32.const 1))
  (global $ARGC_OPTION i32 (i32.const 2))
  (global $WORD i32 (i32.const 4))
  ;; Where the option starts at the latest, for its 3 bytes, `-u` and a NUL, to fit in $args.
  (global $LAST_OPTION i32 (i32.const 61))
  ;; The longest input; what $fill reads at most, one byte more, which shows a longer input; and
  ;; the text, with a newline added to a last line without one.
  (global $TEXT_MAX i32 (i32.const 1048576))
  (global $READ_ROOM i32 (i32.const 1048577))
  (global $TEXT_ROOM i32 (i32.const 1048577))
  ;; The most lines: a line takes a byte at least, its newline.
  (global $MAX_LINES i32 (i32.const 1048577))
  ;; Vectors of 16 bytes: the text's, a line's at most, and the last places a vector may start
  ;; in the input and in the output.
  (global $CHUNK i32 (i32.const 16))
  (global $TEXT_CHUNKS i32 (i32.const 65537))
  (global $LINE_CHUNKS i32 (i32.const 65536))
  (global $LAST_TEXT_CHUNK i32 (i32.const 1048624))
  (global $LAST_OUT_CHUNK i32 (i32.const 1048624))
  ;; A compare's 16 lanes as the bits of a bitmask.
  (global $ALL_LANES i32 (i32.const 65535))
  ;; An entry, 8 bytes: a line's start in the input in its low 32 bits, its length, the newline
  ;; left out, in the high 32.
  (global $ENTRY_SHIFT i32 (i32.const 3))
  (global $LENGTH_SHIFT i64 (i64.const 32))
  ;; Sorting: a merge sort's runs double 21 times at most for MAX_LINES entries; a round makes
  ;; two of those passes.
  (global $MAX_ROUNDS i32 (i32.const 11))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))

  ;; Regions (rule 3, M1), from 1024 up: the messages and the option, read-only, then what the
  ;; host writes of the arguments, the input, the two arrays of entries and the output.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 17))
  (data (i32.const 1024) "usage: sort [-u]\0a")
  (global $too_long i32 (i32.const 1056))
  (global $too_long.rosize i32 (i32.const 39))
  (data (i32.const 1056) "error: input longer than 1048576 bytes\0a")
  (global $option i32 (i32.const 1104))
  (global $option.rosize i32 (i32.const 3))
  (data (i32.const 1104) "-u\00")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 1120))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then the option.
  (global $argv i32 (i32.const 1136))
  (global $argv.size i32 (i32.const 8))
  ;; The arguments' text, each ended by a NUL.
  (global $args i32 (i32.const 1152))  ;; shared: main has the host fill it; is_option reads it
  (global $args.size i32 (i32.const 64))
  ;; Standard input, READ_ROOM bytes, rounded up to whole vectors and some, so that every vector
  ;; a line's bytes start stays inside.
  (global $input i32 (i32.const 1280))  ;; shared: main fills it, the others read its lines
  (global $input.size i32 (i32.const 1048640))
  ;; Each line's entry, MAX_LINES of them, in the order the lines come.
  (global $lines_a i32 (i32.const 1049920))  ;; shared: split_lines fills it, merge_sort sorts it
  (global $lines_a.size i32 (i32.const 8388616))
  ;; The merge sort's second array of entries.
  (global $lines_b i32 (i32.const 9438592))
  (global $lines_b.size i32 (i32.const 8388616))
  ;; The sorted lines, each with its newline: as many bytes as the text, rounded up as the
  ;; input is, for the last vector of a line.
  (global $out i32 (i32.const 17827264))  ;; shared: put_line writes the lines, emit prints them
  (global $out.size i32 (i32.const 1048640))

  ;; Checks the arguments, reads standard input, sorts its lines and prints them, or says why it
  ;; can't. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $argc i32)
    (local $unique i32)
    (local $got i32)
    (local $lines i32)
    (local $sorted i32)
    (block $write_failed
      (block $read_failed
        (block $too_long
          (block $usage
            (local.set $err
              (call $args_sizes_get (global.get $sizes)
                (i32.add (global.get $sizes) (global.get $WORD))))
            (if (local.get $err)
              (then (call $assert_fail (i32.const 1)) unreachable))
            (local.set $argc (i32.load (global.get $sizes)))
            (if (i32.ne (local.get $argc) (global.get $ARGC_PLAIN))
              (then
                ;; One argument, which must be -u.
                (br_if $usage (i32.ne (local.get $argc) (global.get $ARGC_OPTION)))
                ;; Text longer than $args holds isn't -u.
                (br_if $usage
                  (i32.gt_u (i32.load offset=4 (global.get $sizes)) (global.get $args.size)))
                (local.set $err (call $args_get (global.get $argv) (global.get $args)))
                (if (local.get $err)
                  (then (call $assert_fail (i32.const 2)) unreachable))
                ;; The option starts where argv[1] points: an offset into $args.
                (br_if $usage
                  (i32.eqz
                    (call $is_option
                      (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args)))))
                (local.set $unique (i32.const 1))))
            (local.set $got
              (call $fill (global.get $STDIN) (global.get $input) (global.get $READ_ROOM)
                (global.get $READ_TO_END)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $READ_ROOM))
              (then (call $assert_fail (i32.const 3)) unreachable))
            (br_if $too_long (i32.gt_u (local.get $got) (global.get $TEXT_MAX)))
            ;; Empty input prints nothing.
            (if (local.get $got)
              (then
                (local.set $lines (call $split_lines (call $end_last_line (local.get $got))))
                (local.set $sorted (call $merge_sort (local.get $lines)))
                (if (local.get $unique)
                  (then (local.set $lines (call $dedupe (local.get $sorted) (local.get $lines)))))
                (br_if $write_failed (call $emit (local.get $sorted) (local.get $lines)))))
            (call $proc_exit (global.get $EXIT_OK))
            unreachable)
          (br_if $write_failed
            (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
          (call $proc_exit (global.get $EXIT_USAGE))
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

  ;; Whether the argument at byte $at of $args is `-u`: its two bytes, then its NUL. Gives 1 if it
  ;; is, else 0.
  (func $is_option (param $at i32) (result i32)
    (local $k i32)
    (local $same i32)
    (local $left i32)
    (block $compared
      ;; Too near the text's end to hold `-u` and its NUL, it isn't `-u`.
      (br_if $compared (i32.gt_u (local.get $at) (global.get $LAST_OPTION)))
      (local.set $same (i32.const 1))
      (local.set $left (global.get $option.rosize))
      (loop $byte  ;; bound: 3
        (if (i32.ge_u (local.get $k) (global.get $option.rosize))
          (then (call $assert_fail (i32.const 4)) unreachable))
        (if (i32.ne
              (i32.load8_u (i32.add (i32.add (global.get $args) (local.get $at)) (local.get $k)))
              (i32.load8_u (i32.add (global.get $option) (local.get $k))))
          (then (local.set $same (i32.const 0))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $same))

  ;; Gives the text's last line a newline where it has none, and gives the text's length: the
  ;; $got bytes read, and the newline when it is added.
  (func $end_last_line (param $got i32) (result i32)
    (local $len i32)
    (if (i32.gt_u (local.get $got) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 5)) unreachable))
    (if (i32.eqz (local.get $got))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (local.set $len (local.get $got))
    (if (i32.ne (i32.load8_u (i32.add (global.get $input) (i32.sub (local.get $got) (i32.const 1))))
          (global.get $NEWLINE))
      (then
        (i32.store8 (i32.add (global.get $input) (local.get $got)) (global.get $NEWLINE))
        (local.set $len (i32.add (local.get $got) (i32.const 1)))))
    (local.get $len))

  ;; Splits the text's first $len bytes, whose every line ends with a newline, into an entry for
  ;; each line in $lines_a, and gives how many. It finds the newlines 16 bytes at a time: a
  ;; chunk's bitmask has a bit for each, and its lowest set bit is the next one's place.
  (func $split_lines (param $len i32) (result i32)
    (local $i i32)
    (local $mask i32)
    (local $at i32)
    (local $start i32)
    (local $count i32)
    (local $left i32)
    (local $bits i32)
    (local $newlines v128)
    (if (i32.gt_u (local.get $len) (global.get $TEXT_ROOM))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (local.set $newlines (i8x16.splat (global.get $NEWLINE)))
    (block $done
      (local.set $left (global.get $TEXT_CHUNKS))
      (loop $chunk  ;; bound: 65537
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.gt_u (local.get $i) (global.get $LAST_TEXT_CHUNK))
          (then (call $assert_fail (i32.const 8)) unreachable))
        (local.set $mask
          (i8x16.bitmask
            (i8x16.eq (v128.load (i32.add (global.get $input) (local.get $i)))
              (local.get $newlines))))
        (block $chunk_done
          (local.set $bits (global.get $CHUNK))
          (loop $newline  ;; bound: 16
            (br_if $chunk_done (i32.eqz (local.get $mask)))
            (local.set $at (i32.add (local.get $i) (i32.ctz (local.get $mask))))
            ;; Past the text are only the zeros memory starts with, but it stops there anyway.
            (br_if $done (i32.ge_u (local.get $at) (local.get $len)))
            (if (i32.ge_u (local.get $count) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 9)) unreachable))
            (i64.store
              (i32.add (global.get $lines_a)
                (i32.shl (local.get $count) (global.get $ENTRY_SHIFT)))
              (i64.or
                (i64.shl (i64.extend_i32_u (i32.sub (local.get $at) (local.get $start)))
                  (global.get $LENGTH_SHIFT))
                (i64.extend_i32_u (local.get $start))))
            (local.set $count (i32.add (local.get $count) (i32.const 1)))
            (local.set $start (i32.add (local.get $at) (i32.const 1)))
            ;; The newline taken off the mask: its lowest set bit cleared.
            (local.set $mask (i32.and (local.get $mask) (i32.sub (local.get $mask) (i32.const 1))))
            (local.set $bits (i32.sub (local.get $bits) (i32.const 1)))
            (br_if $newline (local.get $bits))))
        (local.set $i (i32.add (local.get $i) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $chunk (local.get $left))))
    (local.get $count))

  ;; Compares the lines of the entries $x and $y in byte order, unsigned, a line that starts the
  ;; other coming first, 16 bytes at a time. A vector may take bytes past a line's end, other
  ;; lines' bytes or the zeros past the text, which are read and never compared. Gives -1 where
  ;; the first comes first, 0 where they're the same, and 1 where the second does.
  (func $compare_lines (param $x i64) (param $y i64) (result i32)
    (local $a i32)
    (local $b i32)
    (local $a_len i32)
    (local $b_len i32)
    (local $n i32)
    (local $k i32)
    (local $at i32)
    (local $bt i32)
    (local $place i32)
    (local $order i32)
    (local $left i32)
    (local $va v128)
    (local $vb v128)
    (local.set $a (i32.wrap_i64 (local.get $x)))
    (local.set $a_len (i32.wrap_i64 (i64.shr_u (local.get $x) (global.get $LENGTH_SHIFT))))
    (local.set $b (i32.wrap_i64 (local.get $y)))
    (local.set $b_len (i32.wrap_i64 (i64.shr_u (local.get $y) (global.get $LENGTH_SHIFT))))
    (if (i32.gt_u (local.get $a_len) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 10)) unreachable))
    (if (i32.gt_u (local.get $b_len) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 11)) unreachable))
    ;; The shorter line's bytes are compared.
    (local.set $n
      (select (local.get $a_len) (local.get $b_len)
        (i32.lt_u (local.get $a_len) (local.get $b_len))))
    (block $decided
      (block $same_start
        (local.set $left (global.get $LINE_CHUNKS))
        (loop $chunk  ;; bound: 65536
          (br_if $same_start (i32.ge_u (local.get $k) (local.get $n)))
          (local.set $at (i32.add (local.get $a) (local.get $k)))
          (if (i32.gt_u (local.get $at) (global.get $LAST_TEXT_CHUNK))
            (then (call $assert_fail (i32.const 12)) unreachable))
          (local.set $bt (i32.add (local.get $b) (local.get $k)))
          (if (i32.gt_u (local.get $bt) (global.get $LAST_TEXT_CHUNK))
            (then (call $assert_fail (i32.const 13)) unreachable))
          (local.set $va (v128.load (i32.add (global.get $input) (local.get $at))))
          (local.set $vb (v128.load (i32.add (global.get $input) (local.get $bt))))
          ;; The first lane where they differ, or 32 where none does.
          (local.set $place
            (i32.ctz
              (i32.xor (i8x16.bitmask (i8x16.eq (local.get $va) (local.get $vb)))
                (global.get $ALL_LANES))))
          ;; A difference before the shorter line's end decides: the first line comes first
          ;; where its byte there is the smaller.
          (if (i32.and (i32.lt_u (local.get $place) (global.get $CHUNK))
                (i32.lt_u (i32.add (local.get $k) (local.get $place)) (local.get $n)))
            (then
              (local.set $order
                (i32.sub (i32.const 1)
                  (i32.shl
                    (i32.and
                      (i32.shr_u (i8x16.bitmask (i8x16.lt_u (local.get $va) (local.get $vb)))
                        (local.get $place))
                      (i32.const 1))
                    (i32.const 1))))
              (br $decided)))
          (local.set $k (i32.add (local.get $k) (global.get $CHUNK)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $chunk (local.get $left))))
      ;; Every byte of the shorter line is the other's: the lengths decide.
      (local.set $order
        (i32.sub (i32.gt_u (local.get $a_len) (local.get $b_len))
          (i32.lt_u (local.get $a_len) (local.get $b_len)))))
    (local.get $order))

  ;; Merges the sorted runs $src[lo, mid) and $src[mid, hi) into $dst[lo, hi), each entry 8 bytes:
  ;; merge_pass lends it the two arrays, MAX_LINES entries each. The left run's line comes first
  ;; where the two are the same, so the merge keeps their order.
  (func $merge (param $src i32) (param $dst i32) (param $lo i32) (param $mid i32) (param $hi i32)
    (local $i i32)
    (local $j i32)
    (local $k i32)
    (local $from_left i32)
    (local $taken i64)
    (local $left i32)
    (if (i32.gt_u (local.get $lo) (local.get $mid))
      (then (call $assert_fail (i32.const 14)) unreachable))
    (if (i32.gt_u (local.get $mid) (local.get $hi))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (local.set $i (local.get $lo))
    (local.set $j (local.get $mid))
    (local.set $k (local.get $lo))
    (block $done
      (local.set $left (global.get $MAX_LINES))
      (loop $entry  ;; bound: 1048577
        (br_if $done (i32.ge_u (local.get $k) (local.get $hi)))
        ;; The left run's entry, while it has one and the right run's isn't earlier.
        (local.set $from_left (i32.lt_u (local.get $i) (local.get $mid)))
        (if (i32.and (local.get $from_left) (i32.lt_u (local.get $j) (local.get $hi)))
          (then
            (if (i32.ge_u (local.get $i) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 16)) unreachable))
            (if (i32.ge_u (local.get $j) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 17)) unreachable))
            (local.set $from_left
              (i32.le_s
                (call $compare_lines
                  (i64.load
                    (i32.add (local.get $src) (i32.shl (local.get $i) (global.get $ENTRY_SHIFT))))
                  (i64.load
                    (i32.add (local.get $src) (i32.shl (local.get $j) (global.get $ENTRY_SHIFT)))))
                (i32.const 0)))))
        (if (local.get $from_left)
          (then
            (if (i32.ge_u (local.get $i) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 18)) unreachable))
            (local.set $taken
              (i64.load
                (i32.add (local.get $src) (i32.shl (local.get $i) (global.get $ENTRY_SHIFT)))))
            (local.set $i (i32.add (local.get $i) (i32.const 1))))
          (else
            (if (i32.ge_u (local.get $j) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 19)) unreachable))
            (local.set $taken
              (i64.load
                (i32.add (local.get $src) (i32.shl (local.get $j) (global.get $ENTRY_SHIFT)))))
            (local.set $j (i32.add (local.get $j) (i32.const 1)))))
        (if (i32.ge_u (local.get $k) (global.get $MAX_LINES))
          (then (call $assert_fail (i32.const 20)) unreachable))
        (i64.store (i32.add (local.get $dst) (i32.shl (local.get $k) (global.get $ENTRY_SHIFT)))
          (local.get $taken))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $entry (local.get $left)))))

  ;; Merges each pair of neighboring sorted runs of $width entries from $src into $dst, the $n
  ;; entries of the two arrays merge_sort lends it.
  (func $merge_pass (param $src i32) (param $dst i32) (param $n i32) (param $width i32)
    (local $lo i32)
    (local $mid i32)
    (local $hi i32)
    (local $left i32)
    (if (i32.gt_u (local.get $n) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 21)) unreachable))
    ;; A run is never empty, and never longer than the entries, so lo + width can't wrap.
    (if (i32.eqz (local.get $width))
      (then (call $assert_fail (i32.const 22)) unreachable))
    (if (i32.gt_u (local.get $width) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 23)) unreachable))
    (block $done
      (local.set $left (global.get $MAX_LINES))
      (loop $pair  ;; bound: 1048577
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

  ;; Sorts the $n entries of $lines_a with a bottom-up merge sort, back and forth between the two
  ;; arrays, two passes a round, so each pass's arrays are named, and gives the array that holds
  ;; them sorted.
  (func $merge_sort (param $n i32) (result i32)
    (local $width i32)
    (local $sorted i32)
    (local $left i32)
    (if (i32.gt_u (local.get $n) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 24)) unreachable))
    (local.set $width (i32.const 1))
    (block $done
      (local.set $sorted (global.get $lines_a))
      (local.set $left (global.get $MAX_ROUNDS))
      (loop $round  ;; bound: 11
        (br_if $done (i32.ge_u (local.get $width) (local.get $n)))
        (call $merge_pass (global.get $lines_a) (global.get $lines_b) (local.get $n)
          (local.get $width))
        (local.set $width (i32.shl (local.get $width) (i32.const 1)))
        (local.set $sorted (global.get $lines_b))
        (br_if $done (i32.ge_u (local.get $width) (local.get $n)))
        (call $merge_pass (global.get $lines_b) (global.get $lines_a) (local.get $n)
          (local.get $width))
        (local.set $width (i32.shl (local.get $width) (i32.const 1)))
        (local.set $sorted (global.get $lines_a))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $round (local.get $left)))
      ;; 11 rounds sort 2 to the 22nd entries, more than MAX_LINES.
      (call $assert_fail (i32.const 25))
      unreachable)
    (local.get $sorted))

  ;; Keeps the first line of each run of the same lines among the $n sorted entries at $sorted,
  ;; in place, and gives how many it keeps.
  (func $dedupe (param $sorted i32) (param $n i32) (result i32)
    (local $k i32)
    (local $kept i32)
    (local $entry i64)
    (local $last i64)
    (local $left i32)
    (if (i32.gt_u (local.get $n) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 26)) unreachable))
    (block $done
      (br_if $done (i32.eqz (local.get $n)))
      (local.set $last (i64.load (local.get $sorted)))
      (local.set $kept (i32.const 1))
      (local.set $k (i32.const 1))
      (local.set $left (global.get $MAX_LINES))
      (loop $next  ;; bound: 1048577
        (br_if $done (i32.ge_u (local.get $k) (local.get $n)))
        (if (i32.ge_u (local.get $k) (global.get $MAX_LINES))
          (then (call $assert_fail (i32.const 27)) unreachable))
        (local.set $entry
          (i64.load
            (i32.add (local.get $sorted) (i32.shl (local.get $k) (global.get $ENTRY_SHIFT)))))
        ;; A line that isn't the last one kept is kept after it.
        (if (call $compare_lines (local.get $last) (local.get $entry))
          (then
            (if (i32.ge_u (local.get $kept) (global.get $MAX_LINES))
              (then (call $assert_fail (i32.const 28)) unreachable))
            (i64.store
              (i32.add (local.get $sorted) (i32.shl (local.get $kept) (global.get $ENTRY_SHIFT)))
              (local.get $entry))
            (local.set $kept (i32.add (local.get $kept) (i32.const 1)))
            (local.set $last (local.get $entry))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $next (local.get $left))))
    (local.get $kept))

  ;; Copies the line of the entry $entry, and a newline, into the output from byte $to on, 16
  ;; bytes at a time, and gives where they end. The last vector may take bytes past the line,
  ;; which the newline and the next line write over.
  (func $put_line (param $entry i64) (param $to i32) (result i32)
    (local $start i32)
    (local $length i32)
    (local $k i32)
    (local $from i32)
    (local $at i32)
    (local $left i32)
    (local.set $start (i32.wrap_i64 (local.get $entry)))
    (local.set $length (i32.wrap_i64 (i64.shr_u (local.get $entry) (global.get $LENGTH_SHIFT))))
    (if (i32.gt_u (local.get $length) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 29)) unreachable))
    (block $copied
      (local.set $left (global.get $LINE_CHUNKS))
      (loop $chunk  ;; bound: 65536
        (br_if $copied (i32.ge_u (local.get $k) (local.get $length)))
        (local.set $from (i32.add (local.get $start) (local.get $k)))
        (if (i32.gt_u (local.get $from) (global.get $LAST_TEXT_CHUNK))
          (then (call $assert_fail (i32.const 30)) unreachable))
        (local.set $at (i32.add (local.get $to) (local.get $k)))
        (if (i32.gt_u (local.get $at) (global.get $LAST_OUT_CHUNK))
          (then (call $assert_fail (i32.const 31)) unreachable))
        (v128.store (i32.add (global.get $out) (local.get $at))
          (v128.load (i32.add (global.get $input) (local.get $from))))
        (local.set $k (i32.add (local.get $k) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $chunk (local.get $left))))
    ;; The newline after the line.
    (local.set $at (i32.add (local.get $to) (local.get $length)))
    (if (i32.ge_u (local.get $at) (global.get $out.size))
      (then (call $assert_fail (i32.const 32)) unreachable))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (i32.add (local.get $at) (i32.const 1)))

  ;; Prints the lines of the $n entries at $sorted, which main lends it, each with a newline, in
  ;; one write. Gives 0, or the error of the write.
  (func $emit (param $sorted i32) (param $n i32) (result i32)  ;; returns: result
    (local $k i32)
    (local $to i32)
    (local $left i32)
    (if (i32.gt_u (local.get $n) (global.get $MAX_LINES))
      (then (call $assert_fail (i32.const 33)) unreachable))
    (block $done
      (local.set $left (global.get $MAX_LINES))
      (loop $line  ;; bound: 1048577
        (br_if $done (i32.ge_u (local.get $k) (local.get $n)))
        (if (i32.ge_u (local.get $k) (global.get $MAX_LINES))
          (then (call $assert_fail (i32.const 34)) unreachable))
        (local.set $to
          (call $put_line
            (i64.load
              (i32.add (local.get $sorted) (i32.shl (local.get $k) (global.get $ENTRY_SHIFT))))
            (local.get $to)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $line (local.get $left))))
    (if (i32.gt_u (local.get $to) (global.get $out.size))
      (then (call $assert_fail (i32.const 35)) unreachable))
    (call $write_all (global.get $STDOUT) (global.get $out) (local.get $to)))
)
