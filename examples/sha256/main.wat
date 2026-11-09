;; sha256: prints the SHA-256 digest of standard input, as FIPS 180-4 defines it. The WebAssembly
;; port of the native sha256, in the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; standard input of any length a 4 KiB piece at a time, each whole block of 64 bytes hashed as it
;; comes, then the last bytes padded. A block's 16 words are read 4 at a time, each word's bytes
;; turned round with one shuffle, and the rounds' constants are added to the schedule 4 words at a
;; time; the rounds themselves are plain integer code, rotates, adds and xors, on eight locals.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; One page: lengua's first 1024 bytes, then the regions below, which end at 6016.
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_IO_FAILED i32 (i32.const 2))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The pieces it takes at most: 2147483647 of 4 KiB, 8 TiB, whose bits a 64-bit length holds.
  (global $MAX_PIECES i32 (i32.const 2147483647))
  ;; Blocks: 64 bytes, 64 of them in a piece, the place of a piece's last, and the masks that
  ;; split a piece's bytes into its whole blocks and the bytes after them.
  (global $BLOCK i32 (i32.const 64))
  (global $BLOCKS i32 (i32.const 64))
  (global $BLOCK_SHIFT i32 (i32.const 6))
  (global $LAST_BLOCK i32 (i32.const 4032))
  (global $WHOLE_BLOCKS i32 (i32.const -64))
  (global $TAIL_MASK i32 (i32.const 63))
  ;; The schedule: a block's 16 words read 16 bytes, 4 words, at a time, the last 16 at byte 48;
  ;; the 48 words made from them, the first of each step's words at byte LAST_EXTEND at most;
  ;; and its 64 words 4 at a time, the last 4 at byte 240.
  (global $VECTOR i32 (i32.const 16))
  (global $WORD i32 (i32.const 4))
  (global $LOAD_STEPS i32 (i32.const 4))
  (global $LAST_LOAD i32 (i32.const 48))
  (global $EXTEND_STEPS i32 (i32.const 48))
  (global $LAST_EXTEND i32 (i32.const 188))
  (global $ADD_STEPS i32 (i32.const 16))
  (global $LAST_W_VECTOR i32 (i32.const 240))
  ;; The rounds, and the place of the last one's word.
  (global $ROUNDS i32 (i32.const 64))
  (global $LAST_ROUND i32 (i32.const 252))
  ;; The padding: a 1 bit, then zeros, then the message's length in bits, 8 bytes big-endian,
  ;; at the end of the first block, or of the second where fewer than 9 bytes are left.
  (global $PAD_BYTE i32 (i32.const 128))
  (global $LENGTH_AT_ONE i32 (i32.const 56))
  (global $LENGTH_AT_TWO i32 (i32.const 120))
  (global $LENGTH_BYTES i32 (i32.const 8))
  (global $BITS_SHIFT i64 (i64.const 3))
  (global $TOP_BYTE_SHIFT i64 (i64.const 56))
  (global $BYTE_SHIFT i64 (i64.const 8))
  ;; The digest: the state's 8 words, each 8 hexadecimal digits, 64 digits and a newline.
  (global $STATE_WORDS i32 (i32.const 8))
  (global $LAST_WORD i32 (i32.const 28))
  (global $NIBBLES i32 (i32.const 8))
  (global $NIBBLE_BITS i32 (i32.const 4))
  (global $NIBBLE_MASK i32 (i32.const 15))
  (global $HEX_DIGITS i32 (i32.const 64))
  (global $NEWLINE i32 (i32.const 10))

  ;; Regions (rule 3, M1), from 1024 up: the constants, read-only, then the state, the schedule,
  ;; the padded blocks, the digest and a piece of standard input.
  ;; The 64 round constants, the first 32 bits of the fractional parts of the cube roots of the
  ;; first 64 primes, each a word in the memory's own order, its lowest byte first.
  (global $k i32 (i32.const 1024))
  (global $k.rosize i32 (i32.const 256))
  (data (i32.const 1024)
    "\98\2f\8a\42\91\44\37\71\cf\fb\c0\b5\a5\db\b5\e9"
    "\5b\c2\56\39\f1\11\f1\59\a4\82\3f\92\d5\5e\1c\ab"
    "\98\aa\07\d8\01\5b\83\12\be\85\31\24\c3\7d\0c\55"
    "\74\5d\be\72\fe\b1\de\80\a7\06\dc\9b\74\f1\9b\c1"
    "\c1\69\9b\e4\86\47\be\ef\c6\9d\c1\0f\cc\a1\0c\24"
    "\6f\2c\e9\2d\aa\84\74\4a\dc\a9\b0\5c\da\88\f9\76"
    "\52\51\3e\98\6d\c6\31\a8\c8\27\03\b0\c7\7f\59\bf"
    "\f3\0b\e0\c6\47\91\a7\d5\51\63\ca\06\67\29\29\14"
    "\85\0a\b7\27\38\21\1b\2e\fc\6d\2c\4d\13\0d\38\53"
    "\54\73\0a\65\bb\0a\6a\76\2e\c9\c2\81\85\2c\72\92"
    "\a1\e8\bf\a2\4b\66\1a\a8\70\8b\4b\c2\a3\51\6c\c7"
    "\19\e8\92\d1\24\06\99\d6\85\35\0e\f4\70\a0\6a\10"
    "\16\c1\a4\19\08\6c\37\1e\4c\77\48\27\b5\bc\b0\34"
    "\b3\0c\1c\39\4a\aa\d8\4e\4f\ca\9c\5b\f3\6f\2e\68"
    "\ee\82\8f\74\6f\63\a5\78\14\78\c8\84\08\02\c7\8c"
    "\fa\ff\be\90\eb\6c\50\a4\f7\a3\f9\be\f2\78\71\c6")
  ;; The first hash value, the first 32 bits of the fractional parts of the square roots of the
  ;; first 8 primes, as the round constants are kept.
  (global $initial i32 (i32.const 1280))
  (global $initial.rosize i32 (i32.const 32))
  (data (i32.const 1280)
    "\67\e6\09\6a\85\ae\67\bb\72\f3\6e\3c\3a\f5\4f\a5"
    "\7f\52\0e\51\8c\68\05\9b\ab\d9\83\1f\19\cd\e0\5b")
  (global $hex i32 (i32.const 1312))
  (global $hex.rosize i32 (i32.const 16))
  (data (i32.const 1312) "0123456789abcdef")
  ;; The hash value's eight words, a to h.
  (global $state i32 (i32.const 1344))  ;; shared: main sets it, rounds adds, print_digest prints
  (global $state.size i32 (i32.const 32))
  ;; One block's 64-word message schedule, each word with its round's constant added.
  (global $w i32 (i32.const 1376))  ;; shared: schedule writes it, and rounds reads it
  (global $w.size i32 (i32.const 256))
  ;; The last one or two blocks: the message's last bytes, a 1 bit, zeros and its length.
  (global $padded i32 (i32.const 1664))
  (global $padded.size i32 (i32.const 128))
  ;; The digest in hexadecimal and a newline.
  (global $out i32 (i32.const 1792))
  (global $out.size i32 (i32.const 65))
  ;; A piece of standard input, 64 blocks.
  (global $input i32 (i32.const 1920))  ;; shared: main fills it, hash_blocks and finish read it
  (global $input.size i32 (i32.const 4096))

  ;; Hashes standard input a piece at a time, whole blocks as they come, then the last bytes with
  ;; the padding, and prints the digest. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $total i64)
    (local $left i32)
    (memory.copy (global.get $state) (global.get $initial) (global.get $initial.rosize))
    (block $failed
      (block $read
        (local.set $left (global.get $MAX_PIECES))
        (loop $piece  ;; bound: 2147483647
          (local.set $got
            (call $fill (global.get $STDIN) (global.get $input) (global.get $input.size)
              (global.get $READ_TO_END)))
          (br_if $failed (i32.lt_s (local.get $got) (i32.const 0)))
          (if (i32.gt_u (local.get $got) (global.get $input.size))
            (then (call $assert_fail (i32.const 1)) unreachable))
          (local.set $total (i64.add (local.get $total) (i64.extend_i32_u (local.get $got))))
          (call $hash_blocks (i32.shr_u (local.get $got) (global.get $BLOCK_SHIFT)))
          ;; A piece that isn't full is the input's last.
          (br_if $read (i32.lt_u (local.get $got) (global.get $input.size)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $piece (local.get $left)))
        ;; 8 TiB is all it takes.
        (call $assert_fail (i32.const 2))
        unreachable)
      ;; The last piece's bytes after its whole blocks, padded.
      (call $finish (i32.and (local.get $got) (global.get $WHOLE_BLOCKS))
        (i32.and (local.get $got) (global.get $TAIL_MASK)) (local.get $total))
      (br_if $failed (call $print_digest))
      (call $proc_exit (global.get $EXIT_OK))
      unreachable)
    (call $proc_exit (global.get $EXIT_IO_FAILED))
    unreachable)

  ;; Hashes the first $count whole blocks of the piece of input, 64 bytes each.
  (func $hash_blocks (param $count i32)
    (local $b i32)
    (local $at i32)
    (local $left i32)
    (if (i32.gt_u (local.get $count) (global.get $BLOCKS))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (block $done
      (local.set $left (global.get $BLOCKS))
      (loop $next  ;; bound: 64
        (br_if $done (i32.ge_u (local.get $b) (local.get $count)))
        (local.set $at (i32.shl (local.get $b) (global.get $BLOCK_SHIFT)))
        (if (i32.gt_u (local.get $at) (global.get $LAST_BLOCK))
          (then (call $assert_fail (i32.const 4)) unreachable))
        (call $schedule (i32.add (global.get $input) (local.get $at)))
        (call $rounds)
        (local.set $b (i32.add (local.get $b) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $next (local.get $left)))))

  ;; Lays out a block's 64-word message schedule in $w: the block's 16 words, big-endian, then
  ;; each of the other 48 from the words before it,
  ;; W[t] = sigma1(W[t-2]) + W[t-7] + sigma0(W[t-15]) + W[t-16]; then adds each round's constant
  ;; to its word, as each round adds both. $block is the block's address, 64 bytes, which the
  ;; caller lends it.
  (func $schedule (param $block i32)
    (local $i i32)
    (local $x i32)
    (local $y i32)
    (local $left i32)
    (local $v v128)
    ;; The 16 words, 4 at a time, each word's 4 bytes turned round.
    (local.set $left (global.get $LOAD_STEPS))
    (loop $load  ;; bound: 4
      (if (i32.gt_u (local.get $i) (global.get $LAST_LOAD))
        (then (call $assert_fail (i32.const 5)) unreachable))
      (local.set $v (v128.load (i32.add (local.get $block) (local.get $i))))
      (v128.store (i32.add (global.get $w) (local.get $i))
        (i8x16.shuffle 3 2 1 0 7 6 5 4 11 10 9 8 15 14 13 12 (local.get $v) (local.get $v)))
      (local.set $i (i32.add (local.get $i) (global.get $VECTOR)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $load (local.get $left)))
    ;; Each word from 16 on, $i the place of W[t-16]: W[t-15] is 4 bytes on, W[t-7] 36, W[t-2]
    ;; 56, and W[t] itself 64.
    (local.set $i (i32.const 0))
    (local.set $left (global.get $EXTEND_STEPS))
    (loop $extend  ;; bound: 48
      (if (i32.gt_u (local.get $i) (global.get $LAST_EXTEND))
        (then (call $assert_fail (i32.const 6)) unreachable))
      (local.set $x (i32.load offset=56 (i32.add (global.get $w) (local.get $i))))
      (local.set $y (i32.load offset=4 (i32.add (global.get $w) (local.get $i))))
      (i32.store offset=64 (i32.add (global.get $w) (local.get $i))
        (i32.add
          (i32.add
            (i32.xor
              (i32.xor (i32.rotr (local.get $x) (i32.const 17))
                (i32.rotr (local.get $x) (i32.const 19)))
              (i32.shr_u (local.get $x) (i32.const 10)))
            (i32.load offset=36 (i32.add (global.get $w) (local.get $i))))
          (i32.add
            (i32.xor
              (i32.xor (i32.rotr (local.get $y) (i32.const 7))
                (i32.rotr (local.get $y) (i32.const 18)))
              (i32.shr_u (local.get $y) (i32.const 3)))
            (i32.load (i32.add (global.get $w) (local.get $i))))))
      (local.set $i (i32.add (local.get $i) (global.get $WORD)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $extend (local.get $left)))
    ;; Each round's constant added to its word, 4 at a time.
    (local.set $i (i32.const 0))
    (local.set $left (global.get $ADD_STEPS))
    (loop $add  ;; bound: 16
      (if (i32.gt_u (local.get $i) (global.get $LAST_W_VECTOR))
        (then (call $assert_fail (i32.const 7)) unreachable))
      (v128.store (i32.add (global.get $w) (local.get $i))
        (i32x4.add (v128.load (i32.add (global.get $w) (local.get $i)))
          (v128.load (i32.add (global.get $k) (local.get $i)))))
      (local.set $i (i32.add (local.get $i) (global.get $VECTOR)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $add (local.get $left))))

  ;; The 64 rounds of the compression function on the schedule, then the result added to the
  ;; state. Round t: T1 = h + Sigma1(e) + Ch(e, f, g) + K[t] + W[t], whose last two $w holds
  ;; added, and T2 = Sigma0(a) + Maj(a, b, c); then h = g, g = f, f = e, e = d + T1, d = c,
  ;; c = b, b = a and a = T1 + T2.
  (func $rounds
    (local $a i32)
    (local $b i32)
    (local $c i32)
    (local $d i32)
    (local $e i32)
    (local $f i32)
    (local $g i32)
    (local $h i32)
    (local $t1 i32)
    (local $t2 i32)
    (local $i i32)
    (local $left i32)
    (local.set $a (i32.load (global.get $state)))
    (local.set $b (i32.load offset=4 (global.get $state)))
    (local.set $c (i32.load offset=8 (global.get $state)))
    (local.set $d (i32.load offset=12 (global.get $state)))
    (local.set $e (i32.load offset=16 (global.get $state)))
    (local.set $f (i32.load offset=20 (global.get $state)))
    (local.set $g (i32.load offset=24 (global.get $state)))
    (local.set $h (i32.load offset=28 (global.get $state)))
    (local.set $left (global.get $ROUNDS))
    (loop $round  ;; bound: 64
      (if (i32.gt_u (local.get $i) (global.get $LAST_ROUND))
        (then (call $assert_fail (i32.const 8)) unreachable))
      ;; Ch(e, f, g) = (e and f) xor (not e and g), which is g xor (e and (f xor g)).
      (local.set $t1
        (i32.add
          (i32.add (local.get $h)
            (i32.xor
              (i32.xor (i32.rotr (local.get $e) (i32.const 6))
                (i32.rotr (local.get $e) (i32.const 11)))
              (i32.rotr (local.get $e) (i32.const 25))))
          (i32.add
            (i32.xor (local.get $g)
              (i32.and (local.get $e) (i32.xor (local.get $f) (local.get $g))))
            (i32.load (i32.add (global.get $w) (local.get $i))))))
      ;; Maj(a, b, c) = (a and b) or (c and (a or b)).
      (local.set $t2
        (i32.add
          (i32.xor
            (i32.xor (i32.rotr (local.get $a) (i32.const 2))
              (i32.rotr (local.get $a) (i32.const 13)))
            (i32.rotr (local.get $a) (i32.const 22)))
          (i32.or (i32.and (local.get $a) (local.get $b))
            (i32.and (local.get $c) (i32.or (local.get $a) (local.get $b))))))
      (local.set $h (local.get $g))
      (local.set $g (local.get $f))
      (local.set $f (local.get $e))
      (local.set $e (i32.add (local.get $d) (local.get $t1)))
      (local.set $d (local.get $c))
      (local.set $c (local.get $b))
      (local.set $b (local.get $a))
      (local.set $a (i32.add (local.get $t1) (local.get $t2)))
      (local.set $i (i32.add (local.get $i) (global.get $WORD)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $round (local.get $left)))
    (i32.store (global.get $state) (i32.add (i32.load (global.get $state)) (local.get $a)))
    (i32.store offset=4 (global.get $state)
      (i32.add (i32.load offset=4 (global.get $state)) (local.get $b)))
    (i32.store offset=8 (global.get $state)
      (i32.add (i32.load offset=8 (global.get $state)) (local.get $c)))
    (i32.store offset=12 (global.get $state)
      (i32.add (i32.load offset=12 (global.get $state)) (local.get $d)))
    (i32.store offset=16 (global.get $state)
      (i32.add (i32.load offset=16 (global.get $state)) (local.get $e)))
    (i32.store offset=20 (global.get $state)
      (i32.add (i32.load offset=20 (global.get $state)) (local.get $f)))
    (i32.store offset=24 (global.get $state)
      (i32.add (i32.load offset=24 (global.get $state)) (local.get $g)))
    (i32.store offset=28 (global.get $state)
      (i32.add (i32.load offset=28 (global.get $state)) (local.get $h))))

  ;; Pads the message's last $len bytes, which start at byte $at of the piece of input, with a 1
  ;; bit, zeros and the message's length in bits, its $total bytes times 8, big-endian, and
  ;; hashes the one or two blocks that makes. It runs once, so the padded blocks still hold
  ;; zeros.
  (func $finish (param $at i32) (param $len i32) (param $total i64)
    (local $length_at i32)
    (local $k i32)
    (local $bits i64)
    (local $left i32)
    (if (i32.gt_u (local.get $at) (global.get $LAST_BLOCK))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (if (i32.ge_u (local.get $len) (global.get $BLOCK))
      (then (call $assert_fail (i32.const 10)) unreachable))
    (memory.copy (global.get $padded) (i32.add (global.get $input) (local.get $at))
      (local.get $len))
    (i32.store8 (i32.add (global.get $padded) (local.get $len)) (global.get $PAD_BYTE))
    ;; The length ends the first block where 8 bytes are left after the 1 bit, else the second.
    (local.set $length_at
      (select (global.get $LENGTH_AT_ONE) (global.get $LENGTH_AT_TWO)
        (i32.lt_u (local.get $len) (global.get $LENGTH_AT_ONE))))
    ;; Its bytes from the most significant.
    (local.set $bits (i64.shl (local.get $total) (global.get $BITS_SHIFT)))
    (local.set $left (global.get $LENGTH_BYTES))
    (loop $byte  ;; bound: 8
      (if (i32.ge_u (local.get $k) (global.get $LENGTH_BYTES))
        (then (call $assert_fail (i32.const 11)) unreachable))
      (i32.store8 (i32.add (i32.add (global.get $padded) (local.get $length_at)) (local.get $k))
        (i32.wrap_i64 (i64.shr_u (local.get $bits) (global.get $TOP_BYTE_SHIFT))))
      (local.set $bits (i64.shl (local.get $bits) (global.get $BYTE_SHIFT)))
      (local.set $k (i32.add (local.get $k) (i32.const 1)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $byte (local.get $left)))
    (call $schedule (global.get $padded))
    (call $rounds)
    (if (i32.eq (local.get $length_at) (global.get $LENGTH_AT_TWO))
      (then
        (call $schedule (i32.add (global.get $padded) (global.get $BLOCK)))
        (call $rounds))))

  ;; Prints the state's eight words as 64 hexadecimal digits, each word big-endian, and a
  ;; newline. Gives 0, or an error number.
  (func $print_digest (result i32)  ;; returns: result
    (local $word i32)
    (local $value i32)
    (local $at i32)
    (local $words i32)
    (local $nibbles i32)
    (local.set $words (global.get $STATE_WORDS))
    (loop $next_word  ;; bound: 8
      (if (i32.gt_u (local.get $word) (global.get $LAST_WORD))
        (then (call $assert_fail (i32.const 12)) unreachable))
      (local.set $value (i32.load (i32.add (global.get $state) (local.get $word))))
      (local.set $nibbles (global.get $NIBBLES))
      (loop $nibble  ;; bound: 8
        ;; The word's digits from its most significant: turned left by 4, its low 4 bits.
        (local.set $value (i32.rotl (local.get $value) (global.get $NIBBLE_BITS)))
        (if (i32.ge_u (local.get $at) (global.get $HEX_DIGITS))
          (then (call $assert_fail (i32.const 13)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.load8_u
            (i32.add (global.get $hex) (i32.and (local.get $value) (global.get $NIBBLE_MASK)))))
        (local.set $at (i32.add (local.get $at) (i32.const 1)))
        (local.set $nibbles (i32.sub (local.get $nibbles) (i32.const 1)))
        (br_if $nibble (local.get $nibbles)))
      (local.set $word (i32.add (local.get $word) (global.get $WORD)))
      (local.set $words (i32.sub (local.get $words) (i32.const 1)))
      (br_if $next_word (local.get $words)))
    (i32.store8 (i32.add (global.get $out) (global.get $HEX_DIGITS)) (global.get $NEWLINE))
    (call $write_all (global.get $STDOUT) (global.get $out) (global.get $out.size)))
)
