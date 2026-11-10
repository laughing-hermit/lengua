;; crc32c: prints the CRC-32C (Castagnoli) of standard input of any length, as iSCSI and ext4
;; compute it. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md): WebAssembly
;; has no CRC instruction, so the CRC goes sixteen bytes a step through sixteen tables, slicing by
;; sixteen: each byte's share comes from the table of as many zero bytes as follow it in the step,
;; so the sixteen lookups don't wait on each other, and only the first word's four wait on the CRC
;; before them, which joins the other twelve's shares last. The tables, 16 KiB in all, are made
;; when the program starts, not kept in the module, and stay in a core's first-level cache.
;; Standard input comes a 64 KiB buffer-full at a time.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; Two pages: lengua's first 1024 bytes, then the regions below, which end at 82965.
  (memory (export "memory") 2 2)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_READ_FAILED i32 (i32.const 2))
  (global $EXIT_WRITE_FAILED i32 (i32.const 3))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The buffer-fulls it takes at most: 2147483647 of 64 KiB, 128 TiB.
  (global $MAX_PIECES i32 (i32.const 2147483647))
  ;; The CRC-32C's polynomial, its bits reflected, and the value it starts from and ends xored
  ;; with: all ones.
  (global $POLY i32 (i32.const 0x82f63b78))
  (global $ALL_ONES i32 (i32.const -1))
  ;; The tables: sixteen of 256 entries of 4 bytes; an entry is its byte value moved left by
  ;; ENTRY_SHIFT. The fifteen tables after the first are LATER_ENTRIES entries.
  (global $VALUES i32 (i32.const 256))
  (global $LATER_ENTRIES i32 (i32.const 3840))
  (global $ENTRY_SHIFT i32 (i32.const 2))
  ;; A byte: its bits, its mask, and where the top byte of a word starts.
  (global $BYTE_BITS i32 (i32.const 8))
  (global $TWO_BYTES i32 (i32.const 16))
  (global $TOP_BYTE i32 (i32.const 24))
  (global $BYTE_MASK i32 (i32.const 255))
  ;; A buffer-full's steps of sixteen bytes, where the last of them starts, and the bytes after
  ;; them.
  (global $STEP i32 (i32.const 16))
  (global $STEPS i32 (i32.const 4096))
  (global $LAST_STEP i32 (i32.const 65520))
  (global $TAIL_MAX i32 (i32.const 15))
  ;; Printing: a number of up to 20 digits, then a newline.
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $NEWLINE_AT i32 (i32.const 20))
  (global $TEN i64 (i64.const 10))
  (global $NEWLINE i32 (i32.const 10))
  (global $ZERO i32 (i32.const 48))

  ;; Regions (rule 3, M1), from 1024 up: the tables, a buffer-full of standard input, and the line
  ;; it prints.
  (global $tables i32 (i32.const 1024))  ;; shared: make_tables fills them; the CRC reads them
  (global $tables.size i32 (i32.const 16384))
  (global $buffer i32 (i32.const 17408))  ;; shared: main fills it; crc_piece and crc_tail read it
  (global $buffer.size i32 (i32.const 65536))
  (global $out i32 (i32.const 82944))
  (global $out.size i32 (i32.const 21))

  ;; Makes the tables, then takes the CRC of standard input a buffer-full at a time, and prints it.
  ;; Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $crc i32)
    (local $from i32)
    (local $left i32)
    (call $make_tables)
    (local.set $crc (global.get $ALL_ONES))
    (block $write_failed
      (block $read_failed
        (block $done
          (local.set $left (global.get $MAX_PIECES))
          (loop $piece  ;; bound: 2147483647
            (local.set $got
              (call $fill (global.get $STDIN) (global.get $buffer) (global.get $buffer.size)
                (global.get $READ_TO_END)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $buffer.size))
              (then (call $assert_fail (i32.const 1)) unreachable))
            ;; Sixteen bytes a step, then the bytes after the last step one at a time.
            (call $crc_piece (local.get $crc) (local.get $got))
            (local.set $from)
            (local.set $crc)
            (local.set $crc (call $crc_tail (local.get $crc) (local.get $from) (local.get $got)))
            ;; A buffer-full that isn't full is the input's last.
            (br_if $done (i32.lt_u (local.get $got) (global.get $buffer.size)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $piece (local.get $left)))
          ;; 128 TiB is all it takes.
          (call $assert_fail (i32.const 2))
          unreachable)
        (br_if $write_failed
          (call $write_count
            (i64.extend_i32_u (i32.xor (local.get $crc) (global.get $ALL_ONES)))))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Fills the tables. The first holds each byte value's CRC alone, eight steps of the polynomial
  ;; each. Each next table holds, for a byte value, the CRC of the value followed by one more zero
  ;; byte than in the table before: that table's entry moved down a byte, xored with the first
  ;; table's entry for the byte that falls out.
  (func $make_tables
    (local $n i32)
    (local $crc i32)
    (local $p i32)
    (local $prev i32)
    (local $bits i32)
    (local $left i32)
    (local.set $left (global.get $VALUES))
    (loop $entries  ;; bound: 256
      (if (i32.ge_u (local.get $n) (global.get $VALUES))
        (then (call $assert_fail (i32.const 3)) unreachable))
      (local.set $crc (local.get $n))
      (local.set $bits (global.get $BYTE_BITS))
      (loop $step  ;; bound: 8
        ;; Shift a bit out, and where it was one, xor the polynomial in.
        (local.set $crc
          (i32.xor (i32.shr_u (local.get $crc) (i32.const 1))
            (i32.and (global.get $POLY)
              (i32.sub (i32.const 0) (i32.and (local.get $crc) (i32.const 1))))))
        (local.set $bits (i32.sub (local.get $bits) (i32.const 1)))
        (br_if $step (local.get $bits)))
      (i32.store (i32.add (global.get $tables) (i32.shl (local.get $n) (global.get $ENTRY_SHIFT)))
        (local.get $crc))
      (local.set $n (i32.add (local.get $n) (i32.const 1)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $entries (local.get $left)))
    ;; Entry p + 256, in the next table, from entry p, in order, so each is made from one made
    ;; before it.
    (local.set $left (global.get $LATER_ENTRIES))
    (loop $later  ;; bound: 3840
      (if (i32.ge_u (local.get $p) (global.get $LATER_ENTRIES))
        (then (call $assert_fail (i32.const 4)) unreachable))
      (local.set $prev
        (i32.load (i32.add (global.get $tables) (i32.shl (local.get $p) (global.get $ENTRY_SHIFT)))))
      (i32.store offset=1024
        (i32.add (global.get $tables) (i32.shl (local.get $p) (global.get $ENTRY_SHIFT)))
        (i32.xor (i32.shr_u (local.get $prev) (global.get $BYTE_BITS))
          (i32.load
            (i32.add (global.get $tables)
              (i32.shl (i32.and (local.get $prev) (global.get $BYTE_MASK))
                (global.get $ENTRY_SHIFT))))))
      (local.set $p (i32.add (local.get $p) (i32.const 1)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $later (local.get $left))))

  ;; The CRC after the whole steps of sixteen bytes among the first $len bytes of the buffer, from
  ;; $crc, and where the bytes after the last step start, which crc_tail takes. A step's first
  ;; word, xored with the CRC, gives the shares of tables 15 to 12; the twelve bytes after it, each
  ;; loaded on its own, give those of tables 11 to 0, and are xored together before the first
  ;; word's shares join them. An entry is found from its byte alone, 0 to 255, so it needs no
  ;; check. A step starts at $last or before, so all sixteen of its bytes are in the buffer.
  (func $crc_piece (param $crc i32) (param $len i32) (result i32 i32)
    (local $i i32)
    (local $p i32)
    (local $x i32)
    (local $last i32)
    (local $left i32)
    (block $tail
      ;; Where the last step may start. Fewer than sixteen bytes wrap it past LAST_STEP, and so
      ;; does a length past the buffer's, which crc_tail fails on: either takes no step.
      (local.set $last (i32.sub (local.get $len) (global.get $STEP)))
      (br_if $tail (i32.gt_u (local.get $last) (global.get $LAST_STEP)))
      (local.set $left (global.get $STEPS))
      (loop $steps  ;; bound: 4096
        (br_if $tail (i32.gt_u (local.get $i) (local.get $last)))
        (local.set $x
          (i32.xor (local.get $crc)
            (i32.load (local.tee $p (i32.add (global.get $buffer) (local.get $i))))))
        (local.set $crc
          (i32.xor
            (i32.xor
              (i32.xor
                (i32.load offset=15360
                  (i32.add (global.get $tables)
                    (i32.shl (i32.and (local.get $x) (global.get $BYTE_MASK))
                      (global.get $ENTRY_SHIFT))))
                (i32.load offset=14336
                  (i32.add (global.get $tables)
                    (i32.shl
                      (i32.and (i32.shr_u (local.get $x) (global.get $BYTE_BITS))
                        (global.get $BYTE_MASK))
                      (global.get $ENTRY_SHIFT)))))
              (i32.xor
                (i32.load offset=13312
                  (i32.add (global.get $tables)
                    (i32.shl
                      (i32.and (i32.shr_u (local.get $x) (global.get $TWO_BYTES))
                        (global.get $BYTE_MASK))
                      (global.get $ENTRY_SHIFT))))
                (i32.load offset=12288
                  (i32.add (global.get $tables)
                    (i32.shl (i32.shr_u (local.get $x) (global.get $TOP_BYTE))
                      (global.get $ENTRY_SHIFT))))))
            (i32.xor
              (i32.xor
                (i32.xor
                  (i32.load offset=11264
                    (i32.add (global.get $tables)
                      (i32.shl (i32.load8_u offset=4 (local.get $p)) (global.get $ENTRY_SHIFT))))
                  (i32.load offset=10240
                    (i32.add (global.get $tables)
                      (i32.shl (i32.load8_u offset=5 (local.get $p)) (global.get $ENTRY_SHIFT)))))
                (i32.xor
                  (i32.load offset=9216
                    (i32.add (global.get $tables)
                      (i32.shl (i32.load8_u offset=6 (local.get $p)) (global.get $ENTRY_SHIFT))))
                  (i32.load offset=8192
                    (i32.add (global.get $tables)
                      (i32.shl (i32.load8_u offset=7 (local.get $p)) (global.get $ENTRY_SHIFT))))))
              (i32.xor
                (i32.xor
                  (i32.xor
                    (i32.load offset=7168
                      (i32.add (global.get $tables)
                        (i32.shl (i32.load8_u offset=8 (local.get $p)) (global.get $ENTRY_SHIFT))))
                    (i32.load offset=6144
                      (i32.add (global.get $tables)
                        (i32.shl (i32.load8_u offset=9 (local.get $p)) (global.get $ENTRY_SHIFT)))))
                  (i32.xor
                    (i32.load offset=5120
                      (i32.add (global.get $tables)
                        (i32.shl (i32.load8_u offset=10 (local.get $p))
                          (global.get $ENTRY_SHIFT))))
                    (i32.load offset=4096
                      (i32.add (global.get $tables)
                        (i32.shl (i32.load8_u offset=11 (local.get $p))
                          (global.get $ENTRY_SHIFT))))))
                (i32.xor
                  (i32.xor
                    (i32.load offset=3072
                      (i32.add (global.get $tables)
                        (i32.shl (i32.load8_u offset=12 (local.get $p))
                          (global.get $ENTRY_SHIFT))))
                    (i32.load offset=2048
                      (i32.add (global.get $tables)
                        (i32.shl (i32.load8_u offset=13 (local.get $p))
                          (global.get $ENTRY_SHIFT)))))
                  (i32.xor
                    (i32.load offset=1024
                      (i32.add (global.get $tables)
                        (i32.shl (i32.load8_u offset=14 (local.get $p))
                          (global.get $ENTRY_SHIFT))))
                    (i32.load
                      (i32.add (global.get $tables)
                        (i32.shl (i32.load8_u offset=15 (local.get $p))
                          (global.get $ENTRY_SHIFT))))))))))
        (local.set $i (i32.add (local.get $i) (global.get $STEP)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $steps (local.get $left))))
    (local.get $crc)
    (local.get $i))

  ;; The CRC after the bytes of the buffer from $from to $len, fewer than sixteen, from $crc: a
  ;; byte at a time through the first table.
  (func $crc_tail (param $crc i32) (param $from i32) (param $len i32) (result i32)
    (local $i i32)
    (local $c i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $buffer.size))
      (then (call $assert_fail (i32.const 5)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $len))
      (then (call $assert_fail (i32.const 6)) unreachable))
    ;; The steps took every whole sixteen bytes, so at most fifteen are left.
    (if (i32.gt_u (i32.sub (local.get $len) (local.get $from)) (global.get $TAIL_MAX))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (local.set $i (local.get $from))
    (local.set $c (local.get $crc))
    (block $done
      (local.set $left (global.get $TAIL_MAX))
      (loop $bytes  ;; bound: 15
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.ge_u (local.get $i) (global.get $buffer.size))
          (then (call $assert_fail (i32.const 8)) unreachable))
        (local.set $c
          (i32.xor (i32.shr_u (local.get $c) (global.get $BYTE_BITS))
            (i32.load
              (i32.add (global.get $tables)
                (i32.shl
                  (i32.and
                    (i32.xor (local.get $c)
                      (i32.load8_u (i32.add (global.get $buffer) (local.get $i))))
                    (global.get $BYTE_MASK))
                  (global.get $ENTRY_SHIFT))))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $bytes (local.get $left))))
    (local.get $c))

  ;; Writes $count in decimal, then a newline, to standard output: the newline last, the digits
  ;; before it from the last. Gives 0, or an error number.
  (func $write_count (param $count i64) (result i32)  ;; returns: result
    (local $rest i64)
    (local $at i32)
    (local $left i32)
    (local.set $rest (local.get $count))
    (local.set $at (global.get $NEWLINE_AT))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 9)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; 20 digits are all a 64-bit number has.
      (call $assert_fail (i32.const 10))
      unreachable)
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
