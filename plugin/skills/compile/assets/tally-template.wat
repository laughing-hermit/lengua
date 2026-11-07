;; tally: counts how often each word of standard input occurs and prints the words, the most
;; frequent first. The WebAssembly counterpart of tally-template.s, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): a hash table of a million slots by FNV-1a with linear
;; probing, its entries in the order the words first appear, sorted by a merge sort to print, so
;; printing walks the entries, never the slots. Every array is a region of linear memory sized
;; from the idea's limits, and every index into one is checked against a named bound.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; 225 pages, 14745600 bytes: lengua's first 1024 bytes and the regions below, which end at
  ;; 14681408.
  (memory (export "memory") 225 225)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_WRITE_FAILED i32 (i32.const 2))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  (global $EXIT_TOO_LONG i32 (i32.const 4))
  ;; What the functions marked "returns: result" give.
  (global $NO_ERROR i32 (i32.const 0))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; What skip passes: a run of whitespace, or a word.
  (global $WHITESPACE i32 (i32.const 1))
  (global $WORD i32 (i32.const 0))
  ;; Bytes that matter. Tab to carriage return are the 5 bytes from 9 to 13.
  (global $SPACE i32 (i32.const 32))
  (global $TAB i32 (i32.const 9))
  (global $TAB_TO_CR i32 (i32.const 4))
  (global $NEWLINE i32 (i32.const 10))
  (global $ZERO i32 (i32.const 48))
  ;; The longest text it counts. The input holds one byte more, which shows a longer text.
  (global $TEXT_MAX i32 (i32.const 1048576))
  ;; The most words a text holds, each a byte and the whitespace after it, and one more.
  (global $MAX_WORDS i32 (i32.const 524289))
  ;; A word's entry, 8 bytes: its count, its start in the input and its length, 21 bits each,
  ;; the count highest, so adding ONE_MORE counts it once more.
  (global $ENTRY_SHIFT i32 (i32.const 3))
  (global $FIELD_BITS i64 (i64.const 21))
  (global $FIELD_MASK i64 (i64.const 2097151))
  (global $COUNT_SHIFT i64 (i64.const 42))
  (global $ONE_MORE i64 (i64.const 4398046511104))
  ;; The index: each slot, 4 bytes, an entry's number plus one, or 0 for none, found by the
  ;; word's hash; a power of two of slots, twice the most words.
  (global $SLOTS i32 (i32.const 1048576))
  (global $SLOT_MASK i32 (i32.const 1048575))
  (global $SLOT_SHIFT i32 (i32.const 2))
  ;; FNV-1a, 64 bits: its offset basis, 0xcbf29ce484222325, as a signed number, and its prime.
  (global $FNV_OFFSET i64 (i64.const -3750763034362895579))
  (global $FNV_PRIME i64 (i64.const 1099511628211))
  ;; Sorting: a merge sort's runs double 20 times at most for MAX_WORDS entries; a round makes
  ;; two of those passes.
  (global $MAX_ROUNDS i32 (i32.const 11))
  ;; Printing: a line is a count of up to 20 digits, a space, the word and a newline.
  (global $LINE_EXTRA i32 (i32.const 22))
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $TEN i64 (i64.const 10))

  ;; Regions (rule 3, M1), from 1024 up: the message, read-only, then the input, the entries and
  ;; the merge sort's second array, the index and the output.
  (global $too_long i32 (i32.const 1024))
  (global $too_long.rosize i32 (i32.const 39))
  (data (i32.const 1024) "error: input longer than 1048576 bytes\0a")
  ;; Standard input, TEXT_MAX bytes and one more.
  (global $input i32 (i32.const 1088))  ;; shared: main fills it, and the words are read from it
  (global $input.size i32 (i32.const 1048577))
  ;; Each distinct word's entry, MAX_WORDS of them, in the order the words first appear.
  (global $entries i32 (i32.const 1049728))  ;; shared: probe adds them, same_word counts them
  (global $entries.size i32 (i32.const 4194312))
  ;; The merge sort's second array of entries.
  (global $scratch i32 (i32.const 5244096))
  (global $scratch.size i32 (i32.const 4194312))
  ;; The index from a word's hash to its entry, SLOTS slots.
  (global $table i32 (i32.const 9438464))
  (global $table.size i32 (i32.const 4194304))
  ;; The lines it prints, a buffer-full at a time: the longest line, and room to spare.
  (global $out i32 (i32.const 13632768))  ;; shared: put_line writes the lines, emit prints them
  (global $out.size i32 (i32.const 1048640))

  ;; Reads standard input, counts each distinct word, and prints each with its count, the most
  ;; frequent first, words with the same count in byte order. Never returns: every path ends in
  ;; proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $words i32)
    (local $sorted i32)
    (block $write_failed
      (block $read_failed
        (block $too_long
          (local.set $got
            (call $fill (global.get $STDIN) (global.get $input) (global.get $input.size)
              (global.get $READ_TO_END)))
          (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
          (br_if $too_long (i32.gt_u (local.get $got) (global.get $TEXT_MAX)))
          (local.set $words (call $count_words (local.get $got)))
          (if (i32.gt_u (local.get $words) (global.get $MAX_WORDS))
            (then (call $assert_fail (i32.const 1)) unreachable))
          (local.set $sorted (call $merge_sort (local.get $words)))
          (br_if $write_failed (call $emit (local.get $sorted) (local.get $words)))
          (call $proc_exit (global.get $EXIT_OK))
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

  ;; Counts each distinct word of the input's first $len bytes: each gets an entry the first time
  ;; it appears, and its count grows each time again. Gives how many distinct words there are.
  (func $count_words (param $len i32) (result i32)
    (local $at i32)
    (local $start i32)
    (local $words i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 2)) unreachable))
    (block $done
      (local.set $left (global.get $MAX_WORDS))
      (loop $word  ;; bound: 524289
        (local.set $start (call $skip (local.get $at) (local.get $len) (global.get $WHITESPACE)))
        (br_if $done (i32.ge_u (local.get $start) (local.get $len)))
        (local.set $at (call $skip (local.get $start) (local.get $len) (global.get $WORD)))
        (local.set $words
          (call $insert (local.get $start) (i32.sub (local.get $at) (local.get $start))
            (local.get $words)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $word (local.get $left)))
      ;; A text of TEXT_MAX bytes has fewer words than MAX_WORDS.
      (call $assert_fail (i32.const 3))
      unreachable)
    (local.get $words))

  ;; Finds where a run of whitespace, or of other bytes, ends: the first byte from $from on that
  ;; isn't of the kind $space says, WHITESPACE or WORD, or $len, where the text ends.
  (func $skip (param $from i32) (param $len i32) (param $space i32) (result i32)
    (local $at i32)
    (local $c i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (local.set $at (local.get $from))
    (block $done
      (local.set $left (global.get $input.size))
      (loop $byte  ;; bound: 1048577
        (br_if $done (i32.ge_u (local.get $at) (local.get $len)))
        (local.set $c (i32.load8_u (i32.add (global.get $input) (local.get $at))))
        ;; A space, or tab to carriage return: below a tab wraps to a large number, so one
        ;; unsigned test finds those five.
        (br_if $done
          (i32.ne (local.get $space)
            (i32.or (i32.eq (local.get $c) (global.get $SPACE))
              (i32.le_u (i32.sub (local.get $c) (global.get $TAB)) (global.get $TAB_TO_CR)))))
        (local.set $at (i32.add (local.get $at) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left)))
      ;; A text has at most TEXT_MAX bytes to pass.
      (call $assert_fail (i32.const 5))
      unreachable)
    (local.get $at))

  ;; Counts one word, the $length bytes at $start of the input: once more in its entry, or in a
  ;; new entry where it's new. Gives how many entries there are now, from the $words there were.
  (func $insert (param $start i32) (param $length i32) (param $words i32) (result i32)
    (if (i32.gt_u (local.get $length) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (if (i32.ge_u (local.get $words) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (call $probe (local.get $start) (local.get $length)
      (i32.and (i32.wrap_i64 (call $fnv (local.get $start) (local.get $length)))
        (global.get $SLOT_MASK))
      (local.get $words)))

  ;; FNV-1a, 64 bits, of the $length bytes at $start of the input.
  (func $fnv (param $start i32) (param $length i32) (result i64)
    (local $hash i64)
    (local $k i32)
    (local $at i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 8)) unreachable))
    (local.set $hash (global.get $FNV_OFFSET))
    (block $done
      (local.set $left (global.get $TEXT_MAX))
      (loop $byte  ;; bound: 1048576
        (br_if $done (i32.ge_u (local.get $k) (local.get $length)))
        (local.set $at (i32.add (local.get $start) (local.get $k)))
        (if (i32.ge_u (local.get $at) (global.get $TEXT_MAX))
          (then (call $assert_fail (i32.const 9)) unreachable))
        (local.set $hash
          (i64.mul
            (i64.xor (local.get $hash)
              (i64.load8_u (i32.add (global.get $input) (local.get $at))))
            (global.get $FNV_PRIME)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $hash))

  ;; Finds the word's entry from slot $slot on, the next slot each time another word holds one,
  ;; and counts the word once more there, or gives it a new entry in the first empty slot. The
  ;; word is the $length bytes at $start of the input. Gives how many entries there are now, from
  ;; the $words there were.
  (func $probe (param $start i32) (param $length i32) (param $slot i32) (param $words i32)
    (result i32)
    (local $held i32)
    (local $now i32)
    (local $left i32)
    (local.set $now (local.get $words))
    (block $counted
      (block $new
        (local.set $left (global.get $SLOTS))
        (loop $next  ;; bound: 1048576
          (if (i32.ge_u (local.get $slot) (global.get $SLOTS))
            (then (call $assert_fail (i32.const 10)) unreachable))
          (local.set $held
            (i32.load
              (i32.add (global.get $table) (i32.shl (local.get $slot) (global.get $SLOT_SHIFT)))))
          (br_if $new (i32.eqz (local.get $held)))
          (br_if $counted
            (call $same_word (i32.sub (local.get $held) (i32.const 1)) (local.get $start)
              (local.get $length)))
          (local.set $slot
            (i32.and (i32.add (local.get $slot) (i32.const 1)) (global.get $SLOT_MASK)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $next (local.get $left)))
        ;; The index has twice as many slots as there are words: it is never full.
        (call $assert_fail (i32.const 11))
        unreachable)
      ;; A new word: its entry, then its slot.
      (if (i32.ge_u (local.get $words) (global.get $MAX_WORDS))
        (then (call $assert_fail (i32.const 12)) unreachable))
      (i64.store
        (i32.add (global.get $entries) (i32.shl (local.get $words) (global.get $ENTRY_SHIFT)))
        (i64.or (global.get $ONE_MORE)
          (i64.or (i64.shl (i64.extend_i32_u (local.get $start)) (global.get $FIELD_BITS))
            (i64.extend_i32_u (local.get $length)))))
      (i32.store
        (i32.add (global.get $table) (i32.shl (local.get $slot) (global.get $SLOT_SHIFT)))
        (i32.add (local.get $words) (i32.const 1)))
      (local.set $now (i32.add (local.get $words) (i32.const 1))))
    (local.get $now))

  ;; Says whether entry $entry is the word's, the $length bytes at $start of the input, and if so
  ;; counts the word once more there. Gives 1 where it is, else 0.
  (func $same_word (param $entry i32) (param $start i32) (param $length i32) (result i32)
    (local $at i32)
    (local $held i64)
    (local $same i32)
    (if (i32.ge_u (local.get $entry) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 13)) unreachable))
    (if (i32.gt_u (local.get $length) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 14)) unreachable))
    (local.set $at
      (i32.add (global.get $entries) (i32.shl (local.get $entry) (global.get $ENTRY_SHIFT))))
    (local.set $held (i64.load (local.get $at)))
    (block $differ
      (br_if $differ
        (i64.ne (i64.and (local.get $held) (global.get $FIELD_MASK))
          (i64.extend_i32_u (local.get $length))))
      (br_if $differ
        (call $compare_bytes
          (i32.wrap_i64
            (i64.and (i64.shr_u (local.get $held) (global.get $FIELD_BITS))
              (global.get $FIELD_MASK)))
          (local.get $length) (local.get $start) (local.get $length)))
      (i64.store (local.get $at) (i64.add (local.get $held) (global.get $ONE_MORE)))
      (local.set $same (i32.const 1)))
    (local.get $same))

  ;; Compares two runs of bytes of the input in byte order, a run that starts the other coming
  ;; first: the $a_len bytes at $a, and the $b_len bytes at $b. Gives -1 where the first comes
  ;; first, 0 where they're the same, and 1 where the second does.
  (func $compare_bytes (param $a i32) (param $a_len i32) (param $b i32) (param $b_len i32)
    (result i32)
    (local $n i32)
    (local $k i32)
    (local $at i32)
    (local $x i32)
    (local $y i32)
    (local $left i32)
    (if (i32.gt_u (local.get $a_len) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (if (i32.gt_u (local.get $b_len) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 16)) unreachable))
    ;; The shorter run's bytes are compared.
    (local.set $n
      (select (local.get $a_len) (local.get $b_len)
        (i32.lt_u (local.get $a_len) (local.get $b_len))))
    (block $decided
      (block $same_start
        (local.set $left (global.get $TEXT_MAX))
        (loop $byte  ;; bound: 1048576
          (br_if $same_start (i32.ge_u (local.get $k) (local.get $n)))
          (local.set $at (i32.add (local.get $a) (local.get $k)))
          (if (i32.ge_u (local.get $at) (global.get $TEXT_MAX))
            (then (call $assert_fail (i32.const 17)) unreachable))
          (local.set $x (i32.load8_u (i32.add (global.get $input) (local.get $at))))
          (local.set $at (i32.add (local.get $b) (local.get $k)))
          (if (i32.ge_u (local.get $at) (global.get $TEXT_MAX))
            (then (call $assert_fail (i32.const 18)) unreachable))
          (local.set $y (i32.load8_u (i32.add (global.get $input) (local.get $at))))
          (br_if $decided (i32.ne (local.get $x) (local.get $y)))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $byte (local.get $left))))
      ;; Every byte of the shorter run is the other's: the lengths decide.
      (local.set $x (local.get $a_len))
      (local.set $y (local.get $b_len)))
    (i32.sub (i32.gt_u (local.get $x) (local.get $y)) (i32.lt_u (local.get $x) (local.get $y))))

  ;; Says which of two entries comes first: the more frequent, or with the same count, the word
  ;; first in byte order. Gives a negative number where $x comes first, 0 where they're the same,
  ;; and a positive one where $y does.
  (func $compare_entries (param $x i64) (param $y i64) (result i32)
    (local $count i64)
    (local $other i64)
    (local $order i32)
    (local.set $count (i64.shr_u (local.get $x) (global.get $COUNT_SHIFT)))
    (local.set $other (i64.shr_u (local.get $y) (global.get $COUNT_SHIFT)))
    ;; Every entry counts its word at least once.
    (if (i64.eqz (local.get $count))
      (then (call $assert_fail (i32.const 19)) unreachable))
    (if (i64.eqz (local.get $other))
      (then (call $assert_fail (i32.const 20)) unreachable))
    (local.set $order
      (i32.sub (i64.lt_u (local.get $count) (local.get $other))
        (i64.gt_u (local.get $count) (local.get $other))))
    (if (i32.eqz (local.get $order))
      (then
        (local.set $order
          (call $compare_bytes
            (i32.wrap_i64
              (i64.and (i64.shr_u (local.get $x) (global.get $FIELD_BITS))
                (global.get $FIELD_MASK)))
            (i32.wrap_i64 (i64.and (local.get $x) (global.get $FIELD_MASK)))
            (i32.wrap_i64
              (i64.and (i64.shr_u (local.get $y) (global.get $FIELD_BITS))
                (global.get $FIELD_MASK)))
            (i32.wrap_i64 (i64.and (local.get $y) (global.get $FIELD_MASK)))))))
    (local.get $order))

  ;; Merges the sorted runs $src[lo, mid) and $src[mid, hi) into $dst[lo, hi), each entry 8 bytes:
  ;; merge_pass lends it the two arrays, MAX_WORDS entries each.
  (func $merge (param $src i32) (param $dst i32) (param $lo i32) (param $mid i32) (param $hi i32)
    (local $i i32)
    (local $j i32)
    (local $k i32)
    (local $from_left i32)
    (local $taken i64)
    (local $left i32)
    (if (i32.gt_u (local.get $lo) (local.get $mid))
      (then (call $assert_fail (i32.const 21)) unreachable))
    (if (i32.gt_u (local.get $mid) (local.get $hi))
      (then (call $assert_fail (i32.const 22)) unreachable))
    (local.set $i (local.get $lo))
    (local.set $j (local.get $mid))
    (local.set $k (local.get $lo))
    (block $done
      (local.set $left (global.get $MAX_WORDS))
      (loop $entry  ;; bound: 524289
        (br_if $done (i32.ge_u (local.get $k) (local.get $hi)))
        ;; The left run's entry, while it has one and the right run's isn't earlier.
        (local.set $from_left (i32.lt_u (local.get $i) (local.get $mid)))
        (if (i32.and (local.get $from_left) (i32.lt_u (local.get $j) (local.get $hi)))
          (then
            (if (i32.ge_u (local.get $i) (global.get $MAX_WORDS))
              (then (call $assert_fail (i32.const 23)) unreachable))
            (if (i32.ge_u (local.get $j) (global.get $MAX_WORDS))
              (then (call $assert_fail (i32.const 24)) unreachable))
            (local.set $from_left
              (i32.le_s
                (call $compare_entries
                  (i64.load
                    (i32.add (local.get $src) (i32.shl (local.get $i) (global.get $ENTRY_SHIFT))))
                  (i64.load
                    (i32.add (local.get $src) (i32.shl (local.get $j) (global.get $ENTRY_SHIFT)))))
                (i32.const 0)))))
        (if (local.get $from_left)
          (then
            (if (i32.ge_u (local.get $i) (global.get $MAX_WORDS))
              (then (call $assert_fail (i32.const 25)) unreachable))
            (local.set $taken
              (i64.load
                (i32.add (local.get $src) (i32.shl (local.get $i) (global.get $ENTRY_SHIFT)))))
            (local.set $i (i32.add (local.get $i) (i32.const 1))))
          (else
            (if (i32.ge_u (local.get $j) (global.get $MAX_WORDS))
              (then (call $assert_fail (i32.const 26)) unreachable))
            (local.set $taken
              (i64.load
                (i32.add (local.get $src) (i32.shl (local.get $j) (global.get $ENTRY_SHIFT)))))
            (local.set $j (i32.add (local.get $j) (i32.const 1)))))
        (if (i32.ge_u (local.get $k) (global.get $MAX_WORDS))
          (then (call $assert_fail (i32.const 27)) unreachable))
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
    (if (i32.gt_u (local.get $n) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 28)) unreachable))
    ;; A run is never empty, and never longer than the entries, so lo + width can't wrap.
    (if (i32.eqz (local.get $width))
      (then (call $assert_fail (i32.const 29)) unreachable))
    (if (i32.gt_u (local.get $width) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 30)) unreachable))
    (block $done
      (local.set $left (global.get $MAX_WORDS))
      (loop $pair  ;; bound: 524289
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

  ;; Sorts the $n entries with a bottom-up merge sort, back and forth between the entries and the
  ;; scratch array, two passes a round, so each pass's arrays are named, and gives the array that
  ;; holds them sorted.
  (func $merge_sort (param $n i32) (result i32)
    (local $width i32)
    (local $sorted i32)
    (local $left i32)
    (if (i32.gt_u (local.get $n) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 31)) unreachable))
    (local.set $width (i32.const 1))
    (block $done
      (local.set $sorted (global.get $entries))
      (local.set $left (global.get $MAX_ROUNDS))
      (loop $round  ;; bound: 11
        (br_if $done (i32.ge_u (local.get $width) (local.get $n)))
        (call $merge_pass (global.get $entries) (global.get $scratch) (local.get $n)
          (local.get $width))
        (local.set $width (i32.shl (local.get $width) (i32.const 1)))
        (local.set $sorted (global.get $scratch))
        (br_if $done (i32.ge_u (local.get $width) (local.get $n)))
        (call $merge_pass (global.get $scratch) (global.get $entries) (local.get $n)
          (local.get $width))
        (local.set $width (i32.shl (local.get $width) (i32.const 1)))
        (local.set $sorted (global.get $entries))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $round (local.get $left)))
      ;; 11 rounds sort 2 to the 22nd entries, more than MAX_WORDS.
      (call $assert_fail (i32.const 32))
      unreachable)
    (local.get $sorted))

  ;; Prints each of the $n entries at $sorted, which main lends it, as a line: its count, a space,
  ;; its word and a newline, a buffer-full at a time. Gives 0, or the error of a write that failed.
  (func $emit (param $sorted i32) (param $n i32) (result i32)  ;; returns: result
    (local $k i32)
    (local $taken i32)
    (local $entry i64)
    (local $err i32)
    (local $left i32)
    (local.set $err (global.get $NO_ERROR))
    (if (i32.gt_u (local.get $n) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 33)) unreachable))
    (block $failed
      (block $done
        (local.set $left (global.get $MAX_WORDS))
        (loop $line  ;; bound: 524289
          (br_if $done (i32.ge_u (local.get $k) (local.get $n)))
          (if (i32.ge_u (local.get $k) (global.get $MAX_WORDS))
            (then (call $assert_fail (i32.const 34)) unreachable))
          (if (i32.gt_u (local.get $taken) (global.get $out.size))
            (then (call $assert_fail (i32.const 35)) unreachable))
          (local.set $entry
            (i64.load
              (i32.add (local.get $sorted) (i32.shl (local.get $k) (global.get $ENTRY_SHIFT)))))
          ;; What the buffer holds goes first, where the line doesn't fit after it.
          (if (i32.gt_u
                (i32.add (local.get $taken)
                  (i32.add (i32.wrap_i64 (i64.and (local.get $entry) (global.get $FIELD_MASK)))
                    (global.get $LINE_EXTRA)))
                (global.get $out.size))
            (then
              (local.set $err
                (call $write_all (global.get $STDOUT) (global.get $out) (local.get $taken)))
              (br_if $failed (local.get $err))
              (local.set $taken (i32.const 0))))
          (local.set $taken (call $put_line (local.get $entry) (local.get $taken)))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $line (local.get $left))))
      ;; What the buffer holds at the end.
      (if (i32.gt_u (local.get $taken) (global.get $out.size))
        (then (call $assert_fail (i32.const 36)) unreachable))
      (if (local.get $taken)
        (then
          (local.set $err
            (call $write_all (global.get $STDOUT) (global.get $out) (local.get $taken)))
          (br_if $failed (local.get $err)))))
    (local.get $err))

  ;; Writes entry $entry's line into the output from byte $at on: its count, a space, its word and
  ;; a newline. Gives where the line ends. emit leaves room for the line after $at.
  (func $put_line (param $entry i64) (param $at i32) (result i32)
    (local $start i32)
    (local $length i32)
    (local $k i32)
    (local $from i32)
    (local $to i32)
    (local $left i32)
    (local.set $start
      (i32.wrap_i64
        (i64.and (i64.shr_u (local.get $entry) (global.get $FIELD_BITS)) (global.get $FIELD_MASK))))
    (local.set $length (i32.wrap_i64 (i64.and (local.get $entry) (global.get $FIELD_MASK))))
    (local.set $to
      (call $put_number (i64.shr_u (local.get $entry) (global.get $COUNT_SHIFT)) (local.get $at)))
    (if (i32.ge_u (local.get $to) (global.get $out.size))
      (then (call $assert_fail (i32.const 37)) unreachable))
    (i32.store8 (i32.add (global.get $out) (local.get $to)) (global.get $SPACE))
    (local.set $to (i32.add (local.get $to) (i32.const 1)))
    (block $copied
      (local.set $left (global.get $TEXT_MAX))
      (loop $byte  ;; bound: 1048576
        (br_if $copied (i32.ge_u (local.get $k) (local.get $length)))
        (local.set $from (i32.add (local.get $start) (local.get $k)))
        (if (i32.ge_u (local.get $from) (global.get $TEXT_MAX))
          (then (call $assert_fail (i32.const 38)) unreachable))
        (if (i32.ge_u (local.get $to) (global.get $out.size))
          (then (call $assert_fail (i32.const 39)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $to))
          (i32.load8_u (i32.add (global.get $input) (local.get $from))))
        (local.set $to (i32.add (local.get $to) (i32.const 1)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (if (i32.ge_u (local.get $to) (global.get $out.size))
      (then (call $assert_fail (i32.const 40)) unreachable))
    (i32.store8 (i32.add (global.get $out) (local.get $to)) (global.get $NEWLINE))
    (i32.add (local.get $to) (i32.const 1)))

  ;; Writes $number's decimal digits into the output from byte $at on, and gives where they end.
  (func $put_number (param $number i64) (param $at i32) (result i32)
    (local $digits i32)
    (local $rest i64)
    (local $to i32)
    (local $left i32)
    ;; How many digits: one, and one more for each time ten goes into what is left.
    (local.set $rest (local.get $number))
    (block $counted
      (local.set $left (global.get $MAX_DIGITS))
      (loop $count  ;; bound: 20
        (local.set $digits (i32.add (local.get $digits) (i32.const 1)))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $counted (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $count (local.get $left)))
      ;; A 64-bit number has at most 20 digits.
      (call $assert_fail (i32.const 41))
      unreachable)
    ;; The digits from the last.
    (local.set $to (i32.add (local.get $at) (local.get $digits)))
    (local.set $rest (local.get $number))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $to (i32.sub (local.get $to) (i32.const 1)))
        (if (i32.ge_u (local.get $to) (global.get $out.size))
          (then (call $assert_fail (i32.const 42)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $to))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      (call $assert_fail (i32.const 43))
      unreachable)
    (i32.add (local.get $at) (local.get $digits))))
