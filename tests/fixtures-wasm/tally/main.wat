;; tally: counts how often each word of standard input of any length occurs and prints the words,
;; the most frequent first. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; the tally template's hash table, FNV-1a with linear probing, and its merge sort, here over a
;; stream: standard input a 64 KiB piece at a time, a word that runs to a piece's end carried to
;; the start of the next; a new word's bytes copied to a store of their own, so an entry needs no
;; input that is gone. Runs of whitespace and of word bytes are passed 16 bytes at a time, from a
;; bitmask of the whitespace lanes, and a word is matched against an entry 16 bytes at a time.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; 99 pages: lengua's first 1024 bytes, then the regions below, which end at 6424576.
  (memory (export "memory") 99 99)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_WRITE_FAILED i32 (i32.const 2))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  (global $EXIT_TOO_MUCH i32 (i32.const 4))
  ;; What the functions marked "returns: result" give: no error, or why the words can't be kept.
  (global $NO_ERROR i32 (i32.const 0))
  (global $ERROR_TOO_LONG i32 (i32.const 1))
  (global $ERROR_TOO_MANY i32 (i32.const 2))
  (global $ERROR_STORE_FULL i32 (i32.const 3))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The pieces it takes at most: 2147483647 of 64 KiB, 128 TiB.
  (global $MAX_PIECES i32 (i32.const 2147483647))
  ;; A piece: PIECE bytes read after the carried word, at most MAX_WORD bytes, so FILLED_MAX bytes
  ;; at most in the buffer.
  (global $PIECE i32 (i32.const 65536))
  (global $MAX_WORD i32 (i32.const 255))
  (global $FILLED_MAX i32 (i32.const 65791))
  ;; The words a piece holds at most, each a byte and the whitespace after it, and one more.
  (global $MAX_SCANS i32 (i32.const 32897))
  ;; Vectors of 16 bytes: the most that pass a piece, where the last starts in the buffer, and the
  ;; most that a word takes.
  (global $CHUNK i32 (i32.const 16))
  (global $VECTORS i32 (i32.const 4113))
  (global $LAST_VECTOR i32 (i32.const 66032))
  (global $WORD_VECTORS i32 (i32.const 16))
  (global $ALL_LANES i32 (i32.const 65535))
  ;; Bytes that matter. Tab to carriage return are the 5 bytes from 9 to 13.
  (global $SPACE i32 (i32.const 32))
  (global $TAB i32 (i32.const 9))
  (global $TAB_TO_CR i32 (i32.const 4))
  (global $NEWLINE i32 (i32.const 10))
  (global $ZERO i32 (i32.const 48))
  ;; The words it keeps: MAX_WORDS entries of 16 bytes, a count of 8 bytes, then where the word's
  ;; bytes start in the store and how many there are, 4 bytes each.
  (global $MAX_WORDS i32 (i32.const 65536))
  (global $ENTRY_SHIFT i32 (i32.const 4))
  ;; The store of the words' bytes, STORE_MAX at most, and where its last vector starts.
  (global $STORE_MAX i32 (i32.const 4194304))
  (global $LAST_STORE_VECTOR i32 (i32.const 4194560))
  ;; The index: a slot of 4 bytes, an entry's number plus one or 0 for none, found by the word's
  ;; hash; twice as many slots as words, a power of two.
  (global $SLOTS i32 (i32.const 131072))
  (global $SLOT_MASK i32 (i32.const 131071))
  (global $SLOT_SHIFT i32 (i32.const 2))
  ;; FNV-1a, 64 bits: its offset basis, 0xcbf29ce484222325, as a signed number, and its prime.
  (global $FNV_OFFSET i64 (i64.const -3750763034362895579))
  (global $FNV_PRIME i64 (i64.const 1099511628211))
  ;; Sorting: entries' numbers of 4 bytes; a merge sort's runs double 16 times at most for
  ;; MAX_WORDS entries, and a round makes two of those passes.
  (global $INDEX_SHIFT i32 (i32.const 2))
  (global $MAX_ROUNDS i32 (i32.const 9))
  ;; Printing: a line is a count of up to 20 digits, a space, a word and a newline, LINE_MAX bytes
  ;; at most; a line starts at LAST_LINE or before, and its word at LAST_WORD_AT or before.
  (global $LAST_LINE i32 (i32.const 65259))
  (global $LAST_WORD_AT i32 (i32.const 65279))
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $TEN i64 (i64.const 10))

  ;; What the program keeps across the pieces: how many words it has, and how many bytes their
  ;; store holds.
  (global $word_count (mut i32) (i32.const 0))  ;; shared: add_word counts the words; main reads it
  (global $store_used (mut i32) (i32.const 0))  ;; shared: add_word adds to it as it keeps a word

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then the buffer, the store, the
  ;; entries, the index, the merge sort's two arrays and the output.
  (global $too_long i32 (i32.const 1024))
  (global $too_long.rosize i32 (i32.const 39))
  (data (i32.const 1024) "error: a word is longer than 255 bytes\0a")
  (global $too_many i32 (i32.const 1088))
  (global $too_many.rosize i32 (i32.const 39))
  (data (i32.const 1088) "error: more than 65536 different words\0a")
  (global $store_full i32 (i32.const 1152))
  (global $store_full.rosize i32 (i32.const 46))
  (data (i32.const 1152) "error: the words take more than 4194304 bytes\0a")
  ;; A piece of standard input after the carried word, and room for a word and a vector more, so
  ;; every word and every vector load from a place before FILLED_MAX stays inside.
  (global $buffer i32 (i32.const 1216))  ;; shared: main fills it, and the words are read from it
  (global $buffer.size i32 (i32.const 66048))
  ;; The words' bytes, STORE_MAX, and room for a word and a vector more.
  (global $store i32 (i32.const 67264))  ;; shared: keep writes the words; the others compare them
  (global $store.size i32 (i32.const 4194576))
  (global $entries i32 (i32.const 4261888))  ;; shared: add_word makes them; the others read them
  (global $entries.size i32 (i32.const 1048576))
  (global $table i32 (i32.const 5310464))  ;; shared: probe looks in it, add_word fills a slot
  (global $table.size i32 (i32.const 524288))
  ;; The merge sort's two arrays of entries' numbers.
  (global $order i32 (i32.const 5834752))
  (global $order.size i32 (i32.const 262144))
  (global $scratch i32 (i32.const 6096896))
  (global $scratch.size i32 (i32.const 262144))
  ;; The lines it prints, a buffer-full at a time.
  (global $out i32 (i32.const 6359040))  ;; shared: put_line and put_number write it, emit prints it
  (global $out.size i32 (i32.const 65536))

  ;; Counts the words of standard input a piece at a time, then prints each with its count, the
  ;; most frequent first. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $carry i32)
    (local $got i32)
    (local $filled i32)
    (local $ended i32)
    (local $status i32)
    (local $consumed i32)
    (local $words i32)
    (local $left i32)
    (block $write_failed
      (block $read_failed
        (block $too_much
          (block $counted
            (local.set $left (global.get $MAX_PIECES))
            (loop $piece  ;; bound: 2147483647
              (if (i32.gt_u (local.get $carry) (global.get $MAX_WORD))
                (then (call $assert_fail (i32.const 1)) unreachable))
              (local.set $got
                (call $fill (global.get $STDIN) (i32.add (global.get $buffer) (local.get $carry))
                  (global.get $PIECE) (global.get $READ_TO_END)))
              (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
              (if (i32.gt_u (local.get $got) (global.get $PIECE))
                (then (call $assert_fail (i32.const 2)) unreachable))
              (local.set $filled (i32.add (local.get $carry) (local.get $got)))
              ;; A piece that doesn't fill its room is the input's last.
              (local.set $ended (i32.lt_u (local.get $got) (global.get $PIECE)))
              (call $scan (local.get $filled) (local.get $ended))
              (local.set $consumed)
              (local.set $status)
              (br_if $too_much (local.get $status))
              (if (i32.gt_u (local.get $consumed) (local.get $filled))
                (then (call $assert_fail (i32.const 3)) unreachable))
              ;; What is left is a word that may go on in the next piece.
              (local.set $carry (i32.sub (local.get $filled) (local.get $consumed)))
              (local.set $status (global.get $ERROR_TOO_LONG))
              (br_if $too_much (i32.gt_u (local.get $carry) (global.get $MAX_WORD)))
              (call $carry_over (local.get $consumed) (local.get $carry))
              (br_if $counted (local.get $ended))
              (local.set $left (i32.sub (local.get $left) (i32.const 1)))
              (br_if $piece (local.get $left)))
            ;; 128 TiB is all it takes.
            (call $assert_fail (i32.const 4))
            unreachable)
          (local.set $words (global.get $word_count))
          (if (i32.gt_u (local.get $words) (global.get $MAX_WORDS))
            (then (call $assert_fail (i32.const 5)) unreachable))
          (br_if $write_failed
            (call $emit (call $merge_sort (local.get $words)) (local.get $words)))
          (call $proc_exit (global.get $EXIT_OK))
          unreachable)
        (br_if $write_failed (call $say_problem (local.get $status)))
        (call $proc_exit (global.get $EXIT_TOO_MUCH))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Moves the $count bytes of the buffer from byte $from, a word that may go on in the next
  ;; piece, to the buffer's start, where the next piece follows it.
  (func $carry_over (param $from i32) (param $count i32)
    (if (i32.gt_u (local.get $from) (global.get $FILLED_MAX))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (if (i32.gt_u (local.get $count) (global.get $MAX_WORD))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (memory.copy (global.get $buffer) (i32.add (global.get $buffer) (local.get $from))
      (local.get $count)))

  ;; Counts each whole word of the buffer's first $filled bytes. Where the bytes don't end the
  ;; input, $ended 0, a word that runs to their end may go on in the next piece, and is left.
  ;; Gives 0 or why the words can't be kept, and how many bytes it is done with.
  (func $scan (param $filled i32) (param $ended i32) (result i32 i32)
    (local $at i32)
    (local $start i32)
    (local $end i32)
    (local $status i32)
    (local $consumed i32)
    (local $left i32)
    (if (i32.gt_u (local.get $filled) (global.get $FILLED_MAX))
      (then (call $assert_fail (i32.const 8)) unreachable))
    (if (i32.gt_u (local.get $ended) (i32.const 1))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (local.set $consumed (local.get $filled))
    (block $done
      (local.set $left (global.get $MAX_SCANS))
      (loop $word  ;; bound: 32897
        (local.set $start (call $skip_space (local.get $at) (local.get $filled)))
        (br_if $done (i32.ge_u (local.get $start) (local.get $filled)))
        (local.set $end (call $skip_word (local.get $start) (local.get $filled)))
        ;; A word that runs to the end of a piece may go on in the next: it is left.
        (if (i32.and (i32.ge_u (local.get $end) (local.get $filled)) (i32.eqz (local.get $ended)))
          (then
            (local.set $consumed (local.get $start))
            (br $done)))
        (if (i32.gt_u (i32.sub (local.get $end) (local.get $start)) (global.get $MAX_WORD))
          (then
            (local.set $status (global.get $ERROR_TOO_LONG))
            (br $done)))
        (local.set $status
          (call $insert (local.get $start) (i32.sub (local.get $end) (local.get $start))))
        (br_if $done (local.get $status))
        (local.set $at (local.get $end))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $word (local.get $left)))
      ;; A word and the whitespace after it take two bytes at least.
      (call $assert_fail (i32.const 10))
      unreachable)
    (local.get $status)
    (local.get $consumed))

  ;; Finds where a run of whitespace ends: the first byte from $from on that isn't whitespace, or
  ;; $len where the bytes end, 16 bytes at a time. A lane past $len is never taken.
  (func $skip_space (param $from i32) (param $len i32) (result i32)
    (local $at i32)
    (local $mask i32)
    (local $found i32)
    (local $chunk v128)
    (local $spaces v128)
    (local $tab v128)
    (local $tab_to_cr v128)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $FILLED_MAX))
      (then (call $assert_fail (i32.const 11)) unreachable))
    ;; The bytes it compares with, spread over every lane, once, before the loop.
    (local.set $spaces (i8x16.splat (global.get $SPACE)))
    (local.set $tab (i8x16.splat (global.get $TAB)))
    (local.set $tab_to_cr (i8x16.splat (global.get $TAB_TO_CR)))
    (local.set $at (local.get $from))
    (local.set $found (local.get $len))
    (block $done
      (local.set $left (global.get $VECTORS))
      (loop $vectors  ;; bound: 4113
        (br_if $done (i32.ge_u (local.get $at) (local.get $len)))
        (if (i32.gt_u (local.get $at) (global.get $LAST_VECTOR))
          (then (call $assert_fail (i32.const 12)) unreachable))
        (local.set $chunk (v128.load (i32.add (global.get $buffer) (local.get $at))))
        ;; A bit for each lane that isn't whitespace: neither a space nor tab to carriage return,
        ;; which one unsigned compare finds, as below a tab wraps to a large number.
        (local.set $mask
          (i32.xor (global.get $ALL_LANES)
            (i8x16.bitmask
              (v128.or (i8x16.eq (local.get $chunk) (local.get $spaces))
                (i8x16.le_u (i8x16.sub (local.get $chunk) (local.get $tab))
                  (local.get $tab_to_cr))))))
        (if (local.get $mask)
          (then
            (local.set $found (i32.add (local.get $at) (i32.ctz (local.get $mask))))
            (br $done)))
        (local.set $at (i32.add (local.get $at) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $vectors (local.get $left))))
    (if (i32.gt_u (local.get $found) (i32.add (local.get $len) (global.get $CHUNK)))
      (then (call $assert_fail (i32.const 13)) unreachable))
    (select (local.get $found) (local.get $len) (i32.lt_u (local.get $found) (local.get $len))))

  ;; Finds where a word ends: the first byte from $from on that is whitespace, or $len where the
  ;; bytes end, 16 bytes at a time. A lane past $len is never taken.
  (func $skip_word (param $from i32) (param $len i32) (result i32)
    (local $at i32)
    (local $mask i32)
    (local $found i32)
    (local $chunk v128)
    (local $spaces v128)
    (local $tab v128)
    (local $tab_to_cr v128)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $FILLED_MAX))
      (then (call $assert_fail (i32.const 14)) unreachable))
    (local.set $spaces (i8x16.splat (global.get $SPACE)))
    (local.set $tab (i8x16.splat (global.get $TAB)))
    (local.set $tab_to_cr (i8x16.splat (global.get $TAB_TO_CR)))
    (local.set $at (local.get $from))
    (local.set $found (local.get $len))
    (block $done
      (local.set $left (global.get $VECTORS))
      (loop $vectors  ;; bound: 4113
        (br_if $done (i32.ge_u (local.get $at) (local.get $len)))
        (if (i32.gt_u (local.get $at) (global.get $LAST_VECTOR))
          (then (call $assert_fail (i32.const 15)) unreachable))
        (local.set $chunk (v128.load (i32.add (global.get $buffer) (local.get $at))))
        ;; A bit for each lane that is whitespace.
        (local.set $mask
          (i8x16.bitmask
            (v128.or (i8x16.eq (local.get $chunk) (local.get $spaces))
              (i8x16.le_u (i8x16.sub (local.get $chunk) (local.get $tab))
                (local.get $tab_to_cr)))))
        (if (local.get $mask)
          (then
            (local.set $found (i32.add (local.get $at) (i32.ctz (local.get $mask))))
            (br $done)))
        (local.set $at (i32.add (local.get $at) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $vectors (local.get $left))))
    (if (i32.gt_u (local.get $found) (i32.add (local.get $len) (global.get $CHUNK)))
      (then (call $assert_fail (i32.const 16)) unreachable))
    (select (local.get $found) (local.get $len) (i32.lt_u (local.get $found) (local.get $len))))

  ;; Counts one word, the $length bytes of the buffer from $start: once more in its entry, or in
  ;; a new entry where it's new. Gives 0, or why it can't be kept.
  (func $insert (param $start i32) (param $length i32) (result i32)  ;; returns: result
    (if (i32.gt_u (local.get $length) (global.get $MAX_WORD))
      (then (call $assert_fail (i32.const 17)) unreachable))
    (if (i32.gt_u (local.get $start) (global.get $FILLED_MAX))
      (then (call $assert_fail (i32.const 18)) unreachable))
    (call $probe (local.get $start) (local.get $length)
      (i32.and (i32.wrap_i64 (call $hash (local.get $start) (local.get $length)))
        (global.get $SLOT_MASK))))

  ;; FNV-1a, 64 bits, of the $length bytes of the buffer from $start.
  (func $hash (param $start i32) (param $length i32) (result i64)
    (local $hash i64)
    (local $k i32)
    (local $at i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $MAX_WORD))
      (then (call $assert_fail (i32.const 19)) unreachable))
    (local.set $hash (global.get $FNV_OFFSET))
    (block $done
      (local.set $left (global.get $MAX_WORD))
      (loop $byte  ;; bound: 255
        (br_if $done (i32.ge_u (local.get $k) (local.get $length)))
        (local.set $at (i32.add (local.get $start) (local.get $k)))
        (if (i32.ge_u (local.get $at) (global.get $buffer.size))
          (then (call $assert_fail (i32.const 20)) unreachable))
        (local.set $hash
          (i64.mul
            (i64.xor (local.get $hash)
              (i64.load8_u (i32.add (global.get $buffer) (local.get $at))))
            (global.get $FNV_PRIME)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $hash))

  ;; Finds the word's entry from slot $slot on, the next slot each time another word holds one,
  ;; and counts the word once more there, or gives it a new entry in the first empty slot. The
  ;; word is the $length bytes of the buffer from $start. Gives 0, or why it can't be kept.
  (func $probe  ;; returns: result
    (param $start i32) (param $length i32) (param $slot i32) (result i32)
    (local $held i32)
    (local $status i32)
    (local $left i32)
    (local.set $status (global.get $NO_ERROR))
    (block $done
      (block $new
        (local.set $left (global.get $SLOTS))
        (loop $next  ;; bound: 131072
          (if (i32.ge_u (local.get $slot) (global.get $SLOTS))
            (then (call $assert_fail (i32.const 21)) unreachable))
          (local.set $held
            (i32.load
              (i32.add (global.get $table) (i32.shl (local.get $slot) (global.get $SLOT_SHIFT)))))
          (br_if $new (i32.eqz (local.get $held)))
          (br_if $done
            (call $same_word (i32.sub (local.get $held) (i32.const 1)) (local.get $start)
              (local.get $length)))
          (local.set $slot
            (i32.and (i32.add (local.get $slot) (i32.const 1)) (global.get $SLOT_MASK)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $next (local.get $left)))
        ;; The index has twice as many slots as there are words: it is never full.
        (call $assert_fail (i32.const 22))
        unreachable)
      (local.set $status (call $add_word (local.get $slot) (local.get $start) (local.get $length)))
      (br_if $done (local.get $status)))
    (local.get $status))

  ;; Says whether entry $entry is the word's, the $length bytes of the buffer from $start, and if
  ;; so counts the word once more there. Gives 1 where it is, else 0.
  (func $same_word (param $entry i32) (param $start i32) (param $length i32) (result i32)
    (local $at i32)
    (local $same i32)
    (if (i32.ge_u (local.get $entry) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 23)) unreachable))
    (if (i32.gt_u (local.get $length) (global.get $MAX_WORD))
      (then (call $assert_fail (i32.const 24)) unreachable))
    (local.set $at
      (i32.add (global.get $entries) (i32.shl (local.get $entry) (global.get $ENTRY_SHIFT))))
    (block $differ
      (br_if $differ (i32.ne (i32.load offset=12 (local.get $at)) (local.get $length)))
      (br_if $differ
        (call $differs (i32.load offset=8 (local.get $at)) (local.get $start) (local.get $length)))
      (i64.store (local.get $at) (i64.add (i64.load (local.get $at)) (i64.const 1)))
      (local.set $same (i32.const 1)))
    (local.get $same))

  ;; Says whether the $length bytes of the store from $offset differ from those of the buffer from
  ;; $start, 16 at a time; the lanes past $length count for nothing. Gives 1 where they differ,
  ;; else 0.
  (func $differs (param $offset i32) (param $start i32) (param $length i32) (result i32)
    (local $k i32)
    (local $p i32)
    (local $q i32)
    (local $lanes i32)
    (local $valid i32)
    (local $differ i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $MAX_WORD))
      (then (call $assert_fail (i32.const 25)) unreachable))
    (block $done
      (local.set $left (global.get $WORD_VECTORS))
      (loop $vectors  ;; bound: 16
        (br_if $done (i32.ge_u (local.get $k) (local.get $length)))
        (local.set $p (i32.add (local.get $offset) (local.get $k)))
        (if (i32.gt_u (local.get $p) (global.get $LAST_STORE_VECTOR))
          (then (call $assert_fail (i32.const 26)) unreachable))
        (local.set $q (i32.add (local.get $start) (local.get $k)))
        (if (i32.gt_u (local.get $q) (global.get $LAST_VECTOR))
          (then (call $assert_fail (i32.const 27)) unreachable))
        ;; The lanes of this step that are the word's: 16, or fewer at its end, as low bits.
        (local.set $lanes (i32.sub (local.get $length) (local.get $k)))
        (local.set $lanes
          (select (local.get $lanes) (global.get $CHUNK)
            (i32.lt_u (local.get $lanes) (global.get $CHUNK))))
        (local.set $valid (i32.sub (i32.shl (i32.const 1) (local.get $lanes)) (i32.const 1)))
        (local.set $differ
          (i32.ne (local.get $valid)
            (i32.and (local.get $valid)
              (i8x16.bitmask
                (i8x16.eq (v128.load (i32.add (global.get $store) (local.get $p)))
                  (v128.load (i32.add (global.get $buffer) (local.get $q))))))))
        (br_if $done (local.get $differ))
        (local.set $k (i32.add (local.get $k) (global.get $CHUNK)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $vectors (local.get $left))))
    (local.get $differ))

  ;; Gives the word, the $length bytes of the buffer from $start, a new entry with a count of 1,
  ;; its bytes kept in the store, and the empty slot $slot of the index. Gives 0, or why it can't:
  ;; too many words, or a store too full for its bytes.
  (func $add_word  ;; returns: result
    (param $slot i32) (param $start i32) (param $length i32) (result i32)
    (local $words i32)
    (local $stored i32)
    (local $at i32)
    (local $status i32)
    (if (i32.ge_u (local.get $slot) (global.get $SLOTS))
      (then (call $assert_fail (i32.const 28)) unreachable))
    (if (i32.gt_u (local.get $length) (global.get $MAX_WORD))
      (then (call $assert_fail (i32.const 29)) unreachable))
    (local.set $words (global.get $word_count))
    (local.set $stored (global.get $store_used))
    (local.set $status (global.get $ERROR_TOO_MANY))
    (block $full
      (br_if $full (i32.ge_u (local.get $words) (global.get $MAX_WORDS)))
      (local.set $status (global.get $ERROR_STORE_FULL))
      (br_if $full
        (i32.gt_u (local.get $stored) (i32.sub (global.get $STORE_MAX) (local.get $length))))
      (call $keep (local.get $stored) (local.get $start) (local.get $length))
      (local.set $at
        (i32.add (global.get $entries) (i32.shl (local.get $words) (global.get $ENTRY_SHIFT))))
      (i64.store (local.get $at) (i64.const 1))
      (i32.store offset=8 (local.get $at) (local.get $stored))
      (i32.store offset=12 (local.get $at) (local.get $length))
      (i32.store
        (i32.add (global.get $table) (i32.shl (local.get $slot) (global.get $SLOT_SHIFT)))
        (i32.add (local.get $words) (i32.const 1)))
      (global.set $word_count (i32.add (local.get $words) (i32.const 1)))
      (global.set $store_used (i32.add (local.get $stored) (local.get $length)))
      (local.set $status (global.get $NO_ERROR)))
    (local.get $status))

  ;; Copies the $length bytes of the buffer from $start to the store from byte $at.
  (func $keep (param $at i32) (param $start i32) (param $length i32)
    (if (i32.gt_u (local.get $at) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 30)) unreachable))
    (if (i32.gt_u (local.get $start) (global.get $FILLED_MAX))
      (then (call $assert_fail (i32.const 31)) unreachable))
    (if (i32.gt_u (local.get $length) (global.get $MAX_WORD))
      (then (call $assert_fail (i32.const 32)) unreachable))
    (memory.copy (i32.add (global.get $store) (local.get $at))
      (i32.add (global.get $buffer) (local.get $start)) (local.get $length)))

  ;; Says which of entries $x and $y comes first: the more frequent, or with the same count, the
  ;; word first in byte order. Gives a negative number where $x comes first, 0 where they're the
  ;; same, and a positive one where $y does.
  (func $compare (param $x i32) (param $y i32) (result i32)
    (local $a i32)
    (local $b i32)
    (local $count i64)
    (local $other i64)
    (local $order i32)
    (if (i32.ge_u (local.get $x) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 33)) unreachable))
    (if (i32.ge_u (local.get $y) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 34)) unreachable))
    (local.set $a (i32.add (global.get $entries) (i32.shl (local.get $x) (global.get $ENTRY_SHIFT))))
    (local.set $b (i32.add (global.get $entries) (i32.shl (local.get $y) (global.get $ENTRY_SHIFT))))
    (local.set $count (i64.load (local.get $a)))
    (local.set $other (i64.load (local.get $b)))
    (local.set $order
      (i32.sub (i64.lt_u (local.get $count) (local.get $other))
        (i64.gt_u (local.get $count) (local.get $other))))
    (if (i32.eqz (local.get $order))
      (then
        (local.set $order
          (call $compare_bytes (i32.load offset=8 (local.get $a)) (i32.load offset=12 (local.get $a))
            (i32.load offset=8 (local.get $b)) (i32.load offset=12 (local.get $b))))))
    (local.get $order))

  ;; Compares two words of the store in byte order, a word that starts the other coming first:
  ;; the $a_len bytes at $a, and the $b_len bytes at $b. Gives -1 where the first comes first, 0
  ;; where they're the same, and 1 where the second does.
  (func $compare_bytes (param $a i32) (param $a_len i32) (param $b i32) (param $b_len i32)
    (result i32)
    (local $n i32)
    (local $k i32)
    (local $at i32)
    (local $x i32)
    (local $y i32)
    (local $left i32)
    (if (i32.gt_u (local.get $a_len) (global.get $MAX_WORD))
      (then (call $assert_fail (i32.const 35)) unreachable))
    (if (i32.gt_u (local.get $b_len) (global.get $MAX_WORD))
      (then (call $assert_fail (i32.const 36)) unreachable))
    ;; The shorter word's bytes are compared.
    (local.set $n
      (select (local.get $a_len) (local.get $b_len)
        (i32.lt_u (local.get $a_len) (local.get $b_len))))
    (block $decided
      (block $same_start
        (local.set $left (global.get $MAX_WORD))
        (loop $byte  ;; bound: 255
          (br_if $same_start (i32.ge_u (local.get $k) (local.get $n)))
          (local.set $at (i32.add (local.get $a) (local.get $k)))
          (if (i32.ge_u (local.get $at) (global.get $store.size))
            (then (call $assert_fail (i32.const 37)) unreachable))
          (local.set $x (i32.load8_u (i32.add (global.get $store) (local.get $at))))
          (local.set $at (i32.add (local.get $b) (local.get $k)))
          (if (i32.ge_u (local.get $at) (global.get $store.size))
            (then (call $assert_fail (i32.const 38)) unreachable))
          (local.set $y (i32.load8_u (i32.add (global.get $store) (local.get $at))))
          (br_if $decided (i32.ne (local.get $x) (local.get $y)))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $byte (local.get $left))))
      ;; Every byte of the shorter word is the other's: the lengths decide.
      (local.set $x (local.get $a_len))
      (local.set $y (local.get $b_len)))
    (i32.sub (i32.gt_u (local.get $x) (local.get $y)) (i32.lt_u (local.get $x) (local.get $y))))

  ;; Merges the sorted runs $src[lo, mid) and $src[mid, hi) into $dst[lo, hi), each an entry's
  ;; number of 4 bytes: merge_pass lends it the two arrays, MAX_WORDS numbers each.
  (func $merge (param $src i32) (param $dst i32) (param $lo i32) (param $mid i32) (param $hi i32)
    (local $i i32)
    (local $j i32)
    (local $k i32)
    (local $from_left i32)
    (local $taken i32)
    (local $left i32)
    (if (i32.gt_u (local.get $lo) (local.get $mid))
      (then (call $assert_fail (i32.const 39)) unreachable))
    (if (i32.gt_u (local.get $mid) (local.get $hi))
      (then (call $assert_fail (i32.const 40)) unreachable))
    (local.set $i (local.get $lo))
    (local.set $j (local.get $mid))
    (local.set $k (local.get $lo))
    (block $done
      (local.set $left (global.get $MAX_WORDS))
      (loop $entry  ;; bound: 65536
        (br_if $done (i32.ge_u (local.get $k) (local.get $hi)))
        ;; The left run's entry, while it has one and the right run's isn't earlier.
        (local.set $from_left (i32.lt_u (local.get $i) (local.get $mid)))
        (if (i32.and (local.get $from_left) (i32.lt_u (local.get $j) (local.get $hi)))
          (then
            (if (i32.ge_u (local.get $i) (global.get $MAX_WORDS))
              (then (call $assert_fail (i32.const 41)) unreachable))
            (if (i32.ge_u (local.get $j) (global.get $MAX_WORDS))
              (then (call $assert_fail (i32.const 42)) unreachable))
            (local.set $from_left
              (i32.le_s
                (call $compare
                  (i32.load
                    (i32.add (local.get $src) (i32.shl (local.get $i) (global.get $INDEX_SHIFT))))
                  (i32.load
                    (i32.add (local.get $src) (i32.shl (local.get $j) (global.get $INDEX_SHIFT)))))
                (i32.const 0)))))
        (if (local.get $from_left)
          (then
            (if (i32.ge_u (local.get $i) (global.get $MAX_WORDS))
              (then (call $assert_fail (i32.const 43)) unreachable))
            (local.set $taken
              (i32.load (i32.add (local.get $src) (i32.shl (local.get $i) (global.get $INDEX_SHIFT)))))
            (local.set $i (i32.add (local.get $i) (i32.const 1))))
          (else
            (if (i32.ge_u (local.get $j) (global.get $MAX_WORDS))
              (then (call $assert_fail (i32.const 44)) unreachable))
            (local.set $taken
              (i32.load (i32.add (local.get $src) (i32.shl (local.get $j) (global.get $INDEX_SHIFT)))))
            (local.set $j (i32.add (local.get $j) (i32.const 1)))))
        (if (i32.ge_u (local.get $k) (global.get $MAX_WORDS))
          (then (call $assert_fail (i32.const 45)) unreachable))
        (i32.store (i32.add (local.get $dst) (i32.shl (local.get $k) (global.get $INDEX_SHIFT)))
          (local.get $taken))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $entry (local.get $left)))))

  ;; Merges each pair of neighboring sorted runs of $width numbers from $src into $dst, the $n
  ;; numbers of the two arrays merge_sort lends it.
  (func $merge_pass (param $src i32) (param $dst i32) (param $n i32) (param $width i32)
    (local $lo i32)
    (local $mid i32)
    (local $hi i32)
    (local $left i32)
    (if (i32.gt_u (local.get $n) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 46)) unreachable))
    ;; A run is never empty, and never longer than the numbers, so lo + width can't wrap.
    (if (i32.eqz (local.get $width))
      (then (call $assert_fail (i32.const 47)) unreachable))
    (if (i32.gt_u (local.get $width) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 48)) unreachable))
    (block $done
      (local.set $left (global.get $MAX_WORDS))
      (loop $pair  ;; bound: 65536
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

  ;; Sorts the $n entries' numbers with a bottom-up merge sort, back and forth between the two
  ;; arrays, two passes a round, so each pass's arrays are named, and gives the array that holds
  ;; them sorted.
  (func $merge_sort (param $n i32) (result i32)
    (local $width i32)
    (local $sorted i32)
    (local $k i32)
    (local $left i32)
    (if (i32.gt_u (local.get $n) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 49)) unreachable))
    ;; The entries' numbers, in the order their words first appeared.
    (block $numbered
      (local.set $left (global.get $MAX_WORDS))
      (loop $number  ;; bound: 65536
        (br_if $numbered (i32.ge_u (local.get $k) (local.get $n)))
        (if (i32.ge_u (local.get $k) (global.get $MAX_WORDS))
          (then (call $assert_fail (i32.const 50)) unreachable))
        (i32.store (i32.add (global.get $order) (i32.shl (local.get $k) (global.get $INDEX_SHIFT)))
          (local.get $k))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $number (local.get $left))))
    (local.set $width (i32.const 1))
    (block $done
      (local.set $sorted (global.get $order))
      (local.set $left (global.get $MAX_ROUNDS))
      (loop $round  ;; bound: 9
        (br_if $done (i32.ge_u (local.get $width) (local.get $n)))
        (call $merge_pass (global.get $order) (global.get $scratch) (local.get $n)
          (local.get $width))
        (local.set $width (i32.shl (local.get $width) (i32.const 1)))
        (local.set $sorted (global.get $scratch))
        (br_if $done (i32.ge_u (local.get $width) (local.get $n)))
        (call $merge_pass (global.get $scratch) (global.get $order) (local.get $n)
          (local.get $width))
        (local.set $width (i32.shl (local.get $width) (i32.const 1)))
        (local.set $sorted (global.get $order))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $round (local.get $left)))
      ;; 9 rounds sort 2 to the 18th numbers, more than MAX_WORDS.
      (call $assert_fail (i32.const 51))
      unreachable)
    (local.get $sorted))

  ;; Prints the line of each of the $n entries whose numbers are at $sorted, which main lends it,
  ;; in that order, a buffer-full at a time. Gives 0, or the error of a write that failed.
  (func $emit (param $sorted i32) (param $n i32) (result i32)  ;; returns: result
    (local $k i32)
    (local $taken i32)
    (local $err i32)
    (local $left i32)
    (local.set $err (global.get $NO_ERROR))
    (if (i32.gt_u (local.get $n) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 52)) unreachable))
    (block $failed
      (block $done
        (local.set $left (global.get $MAX_WORDS))
        (loop $line  ;; bound: 65536
          (br_if $done (i32.ge_u (local.get $k) (local.get $n)))
          (if (i32.ge_u (local.get $k) (global.get $MAX_WORDS))
            (then (call $assert_fail (i32.const 53)) unreachable))
          (if (i32.gt_u (local.get $taken) (global.get $out.size))
            (then (call $assert_fail (i32.const 54)) unreachable))
          ;; What the buffer holds goes first, where the next line may not fit after it.
          (if (i32.gt_u (local.get $taken) (global.get $LAST_LINE))
            (then
              (local.set $err
                (call $write_all (global.get $STDOUT) (global.get $out) (local.get $taken)))
              (br_if $failed (local.get $err))
              (local.set $taken (i32.const 0))))
          (local.set $taken
            (call $put_line
              (i32.load (i32.add (local.get $sorted) (i32.shl (local.get $k) (global.get $INDEX_SHIFT))))
              (local.get $taken)))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $line (local.get $left))))
      ;; What the buffer holds at the end.
      (if (i32.gt_u (local.get $taken) (global.get $out.size))
        (then (call $assert_fail (i32.const 55)) unreachable))
      (if (local.get $taken)
        (then
          (local.set $err
            (call $write_all (global.get $STDOUT) (global.get $out) (local.get $taken)))
          (br_if $failed (local.get $err)))))
    (local.get $err))

  ;; Writes entry $index's line into the output from byte $at on, LAST_LINE or before: its count,
  ;; a space, its word and a newline. Gives where the line ends.
  (func $put_line (param $index i32) (param $at i32) (result i32)
    (local $entry i32)
    (local $offset i32)
    (local $length i32)
    (local $to i32)
    (if (i32.ge_u (local.get $index) (global.get $MAX_WORDS))
      (then (call $assert_fail (i32.const 56)) unreachable))
    (if (i32.gt_u (local.get $at) (global.get $LAST_LINE))
      (then (call $assert_fail (i32.const 57)) unreachable))
    (local.set $entry
      (i32.add (global.get $entries) (i32.shl (local.get $index) (global.get $ENTRY_SHIFT))))
    (local.set $offset (i32.load offset=8 (local.get $entry)))
    (local.set $length (i32.load offset=12 (local.get $entry)))
    (if (i32.gt_u (local.get $offset) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 58)) unreachable))
    (if (i32.gt_u (local.get $length) (global.get $MAX_WORD))
      (then (call $assert_fail (i32.const 59)) unreachable))
    (local.set $to (call $put_number (i64.load (local.get $entry)) (local.get $at)))
    (if (i32.gt_u (local.get $to) (global.get $LAST_WORD_AT))
      (then (call $assert_fail (i32.const 60)) unreachable))
    (i32.store8 (i32.add (global.get $out) (local.get $to)) (global.get $SPACE))
    (local.set $to (i32.add (local.get $to) (i32.const 1)))
    (memory.copy (i32.add (global.get $out) (local.get $to))
      (i32.add (global.get $store) (local.get $offset)) (local.get $length))
    (local.set $to (i32.add (local.get $to) (local.get $length)))
    (if (i32.ge_u (local.get $to) (global.get $out.size))
      (then (call $assert_fail (i32.const 61)) unreachable))
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
      (call $assert_fail (i32.const 62))
      unreachable)
    ;; The digits from the last.
    (local.set $to (i32.add (local.get $at) (local.get $digits)))
    (local.set $rest (local.get $number))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $to (i32.sub (local.get $to) (i32.const 1)))
        (if (i32.ge_u (local.get $to) (global.get $out.size))
          (then (call $assert_fail (i32.const 63)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $to))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      (call $assert_fail (i32.const 64))
      unreachable)
    (i32.add (local.get $at) (local.get $digits)))

  ;; Writes why the words can't be kept, $status, to standard error. Gives 0, or an error number.
  (func $say_problem (param $status i32) (result i32)  ;; returns: result
    (local $err i32)
    (if (i32.eqz (local.get $status))
      (then (call $assert_fail (i32.const 65)) unreachable))
    (if (i32.gt_u (local.get $status) (global.get $ERROR_STORE_FULL))
      (then (call $assert_fail (i32.const 66)) unreachable))
    (block $said
      (if (i32.eq (local.get $status) (global.get $ERROR_TOO_LONG))
        (then
          (local.set $err
            (call $write_all (global.get $STDERR) (global.get $too_long)
              (global.get $too_long.rosize)))
          (br_if $said (local.get $err)))
        (else
          (if (i32.eq (local.get $status) (global.get $ERROR_TOO_MANY))
            (then
              (local.set $err
                (call $write_all (global.get $STDERR) (global.get $too_many)
                  (global.get $too_many.rosize)))
              (br_if $said (local.get $err)))
            (else
              (local.set $err
                (call $write_all (global.get $STDERR) (global.get $store_full)
                  (global.get $store_full.rosize)))
              (br_if $said (local.get $err)))))))
    (local.get $err))
)
