;; crc32c-split: prints the CRC-32C (Castagnoli) of standard input of any length, with the work
;; split into pieces that run at once. In the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): the whole program reads standard input 8 MiB at a
;; time and splits each part into eight pieces. A piece is one more run of this module, with its
;; share of the bytes as its standard input and a memory of its own: it takes their CRC a 64 KiB
;; buffer-full at a time, sixteen bytes a step through sixteen tables it makes when it starts,
;; and prints it. The program joins the pieces' CRCs in their order: the CRC of two texts one
;; after the other is the first's CRC times x to the power of the second's bits, in the CRC's
;; own arithmetic, xored with the second's CRC. `piece` says which of the two a run is.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (import "lengua_split_1" "split" (func $split (param i32 i32 i32 i32 i32) (result i32)))
  (import "lengua_split_1" "piece" (func $piece (result i32)))
  ;; 130 pages: lengua's first 1024 bytes, then the regions below, which end at 8473600.
  (memory (export "memory") 130 130)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_READ_FAILED i32 (i32.const 2))
  (global $EXIT_WRITE_FAILED i32 (i32.const 3))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; What `piece` answers in the run that is the whole program: NOENT.
  (global $NOT_A_PIECE i32 (i32.const 44))
  ;; The pieces a part of the input is split into, as the idea's line grants, as a count and as
  ;; a divisor; and an answer's 16 bytes as a shift.
  (global $PIECES i32 (i32.const 8))
  (global $PIECES_64 i64 (i64.const 8))
  (global $ANSWER_SHIFT i32 (i32.const 4))
  ;; Where the last answer starts among the answers, and an answer's bytes at most: a CRC's ten
  ;; digits and a newline.
  (global $LAST_ANSWER i32 (i32.const 112))
  (global $ANSWER_BYTES i32 (i32.const 11))
  (global $TEN_32 i32 (i32.const 10))
  ;; The CRC's own arithmetic, its bits reflected: 1, x, and a word's bits.
  (global $ONE i32 (i32.const 0x80000000))
  (global $X i32 (i32.const 0x40000000))
  (global $WORD_BITS i32 (i32.const 32))
  ;; The buffer-fulls it takes at most: 2147483647, of 64 KiB in a piece and of 8 MiB in the
  ;; whole program.
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

  ;; Regions (rule 3, M1), from 1024 up: the tables, a buffer-full of standard input and the line
  ;; it prints, as a piece uses them, then the pieces' answers and a part of the input, as the
  ;; whole program does.
  (global $tables i32 (i32.const 1024))  ;; shared: make_tables fills them; the CRC reads them
  (global $tables.size i32 (i32.const 16384))
  (global $buffer i32 (i32.const 17408))  ;; shared: main fills it; crc_piece and crc_tail read it
  (global $buffer.size i32 (i32.const 65536))
  (global $out i32 (i32.const 82944))
  (global $out.size i32 (i32.const 21))
  ;; What the eight pieces of a part printed, each at the start of its 16 bytes, with zeros
  ;; after it.
  (global $answers i32 (i32.const 83968))  ;; shared: join_part has them written; answer reads them
  (global $answers.size i32 (i32.const 128))
  ;; A part of standard input, 8 MiB, which the whole program splits.
  (global $block i32 (i32.const 84992))  ;; shared: main fills it; join_part lends it to the pieces
  (global $block.size i32 (i32.const 8388608))

  ;; Asks whether this run is a piece. A piece makes the tables, takes the CRC of standard input
  ;; a buffer-full at a time, and prints it. The whole program reads standard input a part at a
  ;; time, has each part's pieces take their CRCs, joins them, and prints the CRC of all. Never
  ;; returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $crc i32)
    (local $from i32)
    (local $left i32)
    (local $whole i32)
    ;; 0 in a piece of the work, NOT_A_PIECE in the whole program, and nothing else.
    (local.set $whole (call $piece))
    (if (local.get $whole)
      (then
        (if (i32.ne (local.get $whole) (global.get $NOT_A_PIECE))
          (then (call $assert_fail (i32.const 11)) unreachable))))
    (block $write_failed
      (block $read_failed
        (if (local.get $whole)
          (then
            ;; The CRC of no bytes is 0, and each part's pieces join it in their order.
            (block $joined
              (local.set $left (global.get $MAX_PIECES))
              (loop $part  ;; bound: 2147483647
                (local.set $got
                  (call $fill (global.get $STDIN) (global.get $block) (global.get $block.size)
                    (global.get $READ_TO_END)))
                (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
                (if (i32.gt_u (local.get $got) (global.get $block.size))
                  (then (call $assert_fail (i32.const 12)) unreachable))
                (local.set $crc (call $join_part (local.get $crc) (local.get $got)))
                ;; A part that isn't full is the input's last.
                (br_if $joined (i32.lt_u (local.get $got) (global.get $block.size)))
                (local.set $left (i32.sub (local.get $left) (i32.const 1)))
                (br_if $part (local.get $left)))
              ;; 16 PiB is all it takes.
              (call $assert_fail (i32.const 13))
              unreachable))
          (else
            (call $make_tables)
            (local.set $crc (global.get $ALL_ONES))
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
                (local.set $crc
                  (call $crc_tail (local.get $crc) (local.get $from) (local.get $got)))
                ;; A buffer-full that isn't full is the input's last.
                (br_if $done (i32.lt_u (local.get $got) (global.get $buffer.size)))
                (local.set $left (i32.sub (local.get $left) (i32.const 1)))
                (br_if $piece (local.get $left)))
              ;; 128 TiB is all it takes.
              (call $assert_fail (i32.const 2))
              unreachable)
            (local.set $crc (i32.xor (local.get $crc) (global.get $ALL_ONES)))))
        (br_if $write_failed (call $write_count (i64.extend_i32_u (local.get $crc))))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Has the first $len bytes of the part taken as PIECES pieces, each a run of this module on
  ;; its share of the bytes, at once where the computer has the cores, and gives the CRC of the
  ;; bytes that have $crc followed by the part: each piece's CRC joined in the pieces' order,
  ;; with the length of its share, which is the host's cut. No bytes change nothing.
  (func $join_part (param $crc i32) (param $len i32) (result i32)
    (local $err i32)
    (local $index i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $block.size))
      (then (call $assert_fail (i32.const 14)) unreachable))
    (block $none
      (br_if $none (i32.eqz (local.get $len)))
      (local.set $err
        (call $split (global.get $block) (local.get $len) (global.get $PIECES)
          (global.get $answers) (global.get $answers.size)))
      ;; The idea's line covers the call, and a piece that fails ends the run, so it never fails.
      (if (local.get $err)
        (then (call $assert_fail (i32.const 15)) unreachable))
      (local.set $left (global.get $PIECES))
      (loop $next  ;; bound: 8
        (if (i32.ge_u (local.get $index) (global.get $PIECES))
          (then (call $assert_fail (i32.const 16)) unreachable))
        (local.set $crc
          (call $join (local.get $crc)
            (call $answer (i32.shl (local.get $index) (global.get $ANSWER_SHIFT)))
            (call $share (local.get $len) (local.get $index))))
        (local.set $index (i32.add (local.get $index) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $next (local.get $left))))
    (local.get $crc))

  ;; How many of $len bytes piece $index gets, as the host cuts them: the bytes from $len times
  ;; $index over PIECES to $len times $index and one over PIECES, each rounded down.
  (func $share (param $len i32) (param $index i32) (result i32)
    (if (i32.ge_u (local.get $index) (global.get $PIECES))
      (then (call $assert_fail (i32.const 17)) unreachable))
    (i32.wrap_i64
      (i64.sub
        (i64.div_u
          (i64.mul (i64.extend_i32_u (local.get $len))
            (i64.extend_i32_u (i32.add (local.get $index) (i32.const 1))))
          (global.get $PIECES_64))
        (i64.div_u
          (i64.mul (i64.extend_i32_u (local.get $len)) (i64.extend_i32_u (local.get $index)))
          (global.get $PIECES_64)))))

  ;; The number the answer that starts at byte $from of the answers holds: a piece's CRC, its
  ;; decimal digits and a newline.
  (func $answer (param $from i32) (result i32)
    (local $i i32)
    (local $byte i32)
    (local $value i32)
    (local $left i32)
    (if (i32.gt_u (local.get $from) (global.get $LAST_ANSWER))
      (then (call $assert_fail (i32.const 18)) unreachable))
    (local.set $i (local.get $from))
    (block $read
      (local.set $left (global.get $ANSWER_BYTES))
      (loop $digit  ;; bound: 11
        (if (i32.ge_u (local.get $i) (global.get $answers.size))
          (then (call $assert_fail (i32.const 19)) unreachable))
        (local.set $byte (i32.load8_u (i32.add (global.get $answers) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br_if $read (i32.eq (local.get $byte) (global.get $NEWLINE)))
        (local.set $value
          (i32.add (i32.mul (local.get $value) (global.get $TEN_32))
            (i32.sub (local.get $byte) (global.get $ZERO))))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; A CRC has ten digits at most, and a newline after them.
      (call $assert_fail (i32.const 20))
      unreachable)
    (local.get $value))

  ;; The CRC of a text followed by another, from the first's CRC, $crc1, the second's, $crc2,
  ;; and the second's length in bytes, $len2: $crc1 times x to the power of eight times $len2,
  ;; xored with $crc2. The power is made a bit of $len2 at a time, from x to the eighth, which
  ;; is x squared three times, squared again at each bit.
  (func $join (param $crc1 i32) (param $crc2 i32) (param $len2 i32) (result i32)
    (local $power i32)
    (local $square i32)
    (local $n i32)
    (local $left i32)
    (local.set $square (call $times (global.get $X) (global.get $X)))
    (local.set $square (call $times (local.get $square) (local.get $square)))
    (local.set $square (call $times (local.get $square) (local.get $square)))
    (local.set $power (global.get $ONE))
    (local.set $n (local.get $len2))
    (block $raised
      (local.set $left (global.get $WORD_BITS))
      (loop $bit  ;; bound: 32
        (br_if $raised (i32.eqz (local.get $n)))
        (if (i32.and (local.get $n) (i32.const 1))
          (then (local.set $power (call $times (local.get $square) (local.get $power)))))
        (local.set $n (i32.shr_u (local.get $n) (i32.const 1)))
        (local.set $square (call $times (local.get $square) (local.get $square)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $bit (local.get $left))))
    (i32.xor (call $times (local.get $power) (local.get $crc1)) (local.get $crc2)))

  ;; $a times $b in the CRC's own arithmetic, polynomials over bits modulo the CRC's, their
  ;; bits reflected: for each term of $a from the highest, $b is added, and $b is multiplied
  ;; by x. $a is never 0, so its last term comes within a word's bits.
  (func $times (param $a i32) (param $b i32) (result i32)
    (local $term i32)
    (local $product i32)
    (local $left i32)
    (local.set $term (global.get $ONE))
    (block $done
      (local.set $left (global.get $WORD_BITS))
      (loop $bit  ;; bound: 32
        (if (i32.and (local.get $a) (local.get $term))
          (then
            (local.set $product (i32.xor (local.get $product) (local.get $b)))
            ;; No term of $a after this one: done.
            (br_if $done
              (i32.eqz
                (i32.and (local.get $a) (i32.sub (local.get $term) (i32.const 1)))))))
        (local.set $term (i32.shr_u (local.get $term) (i32.const 1)))
        (local.set $b
          (i32.xor (i32.shr_u (local.get $b) (i32.const 1))
            (i32.and (global.get $POLY)
              (i32.sub (i32.const 0) (i32.and (local.get $b) (i32.const 1))))))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $bit (local.get $left)))
      (call $assert_fail (i32.const 21))
      unreachable)
    (local.get $product))

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
