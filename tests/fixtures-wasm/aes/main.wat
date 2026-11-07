;; aes: encrypts blocks of hexadecimal data with AES-128 (FIPS 197) in ECB mode, in portable code.
;; The WebAssembly twin of examples/aes, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md). No step looks up a table with a byte of the key or of
;; the data, and none of the encryption branches on one: a block is one 128-bit vector, its byte
;; i at row i mod 4 and column i / 4 as FIPS 197 lays out its state. SubBytes computes each byte's
;; inverse in GF(2^8) with masked shifts and xors, 16 bytes at once, then the affine map; ShiftRows
;; and the turns of MixColumns are shuffles of the vector's lanes, with constant lane numbers.
;; Hexadecimal digits are read with compares that give 1 or 0, and written with arithmetic.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; 6 pages, 393216 bytes: lengua's first 1024 bytes and the regions below, which end at 329025.
  (memory (export "memory") 6 6)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_IO_FAILED i32 (i32.const 2))
  (global $EXIT_BAD_INPUT i32 (i32.const 3))
  ;; What the functions marked "returns: result" give.
  (global $NO_ERROR i32 (i32.const 0))
  (global $ERROR_INVALID i32 (i32.const 28))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The input: the key's KEY_HEX digits from byte KEY_AT, a newline at byte KEY_HEX, then the
  ;; data's digits from byte DATA_AT, a multiple of 32, from 32 to HEX_MAX, and a last newline
  ;; that may be absent; MIN_INPUT bytes at least.
  (global $KEY_AT i32 (i32.const 0))
  (global $KEY_HEX i32 (i32.const 32))
  (global $DATA_AT i32 (i32.const 33))
  (global $MIN_INPUT i32 (i32.const 65))
  (global $HEX_MAX i32 (i32.const 131072))
  (global $BLOCK_MASK i32 (i32.const 31))
  (global $DIGITS_SHIFT i32 (i32.const 5))
  ;; Where a character pair starts at the latest.
  (global $LAST_PAIR i32 (i32.const 131105))
  ;; The bytes: the key's 16 at byte KEY_TO, then the data's blocks at byte DATA_TO, up to
  ;; MAX_BLOCKS of 16 bytes, MAX_BYTES; a block starts at a multiple of 16, at LAST_BLOCK at most.
  (global $KEY_TO i32 (i32.const 0))
  (global $DATA_TO i32 (i32.const 16))
  (global $MAX_BLOCKS i32 (i32.const 4096))
  (global $MAX_BYTES i32 (i32.const 65536))
  (global $BLOCK i32 (i32.const 16))
  (global $BLOCK_SHIFT i32 (i32.const 4))
  (global $BLOCK_ALIGN i32 (i32.const 15))
  (global $LAST_BLOCK i32 (i32.const 65536))
  ;; A hexadecimal digit: 0 to 9 from `0`, 10 to 15 from `a` or `A`, where the case bit makes `A`
  ;; to `F` the same as `a` to `f`; INVALID marks a character that is neither, beside the value's
  ;; 4 bits. A byte is BYTE_MAX at most.
  (global $BYTE_MAX i32 (i32.const 255))
  (global $ZERO i32 (i32.const 48))
  (global $SMALL_A i32 (i32.const 97))
  (global $CASE_BIT i32 (i32.const 32))
  (global $DIGITS i32 (i32.const 10))
  (global $LETTERS i32 (i32.const 6))
  (global $INVALID i32 (i32.const 16))
  (global $NIBBLE_BITS i32 (i32.const 4))
  ;; The key's expansion: 10 round keys after the key, made from the key before at byte
  ;; LAST_EXPAND at most; RotWord turns a word by BYTE_BITS, and the round constant starts at 1
  ;; and doubles in GF(2^8), whose polynomial x^8 + x^4 + x^3 + x + 1 leaves AES_POLY once the
  ;; top bit, bit TOP_BIT, moves out.
  (global $EXPAND_STEPS i32 (i32.const 10))
  (global $LAST_EXPAND i32 (i32.const 144))
  (global $BYTE_BITS i32 (i32.const 8))
  (global $FIRST_RCON i32 (i32.const 1))
  (global $TOP_BIT i32 (i32.const 7))
  (global $AES_POLY i32 (i32.const 27))
  ;; The rounds: 9 with MixColumns, their keys from byte 16 to LAST_ROUND_KEY, then the last.
  (global $MIDDLE_ROUNDS i32 (i32.const 9))
  (global $LAST_ROUND_KEY i32 (i32.const 144))
  ;; SubBytes: an inverse is x to the 254th, 6 steps of squaring and multiplying by x, then a
  ;; square; a product takes 8 bits; and the affine map adds AFFINE.
  (global $INVERSE_STEPS i32 (i32.const 6))
  (global $BITS i32 (i32.const 8))
  (global $AFFINE i32 (i32.const 99))
  ;; The output: two digits a byte, `0` plus the digit, and LETTER_GAP more past 9, the last
  ;; pair at byte LAST_PAIR_OUT at most, then a newline.
  (global $NIBBLE_MASK i32 (i32.const 15))
  (global $NINE i32 (i32.const 9))
  (global $LETTER_GAP i32 (i32.const 39))
  (global $LAST_PAIR_OUT i32 (i32.const 131071))
  (global $NEWLINE i32 (i32.const 10))

  ;; Regions (rule 3, M1), from 1024 up: the message, read-only, then the round keys, the key's
  ;; and the data's bytes, the input and the output.
  (global $bad_input i32 (i32.const 1024))
  (global $bad_input.rosize i32 (i32.const 15))
  (data (i32.const 1024) "aes: bad input\0a")
  ;; The 11 round keys of 16 bytes.
  (global $round_keys i32 (i32.const 1040))  ;; shared: expand_key writes it, encrypt_block reads it
  (global $round_keys.size i32 (i32.const 176))
  ;; The key's 16 bytes, then the data's blocks, which are encrypted in place.
  (global $bytes i32 (i32.const 1216))  ;; shared: decode writes it, the others read and encrypt it
  (global $bytes.size i32 (i32.const 65552))
  ;; All of standard input, the longest and one byte more, which shows a longer input.
  (global $input i32 (i32.const 66816))  ;; shared: main fills it, and parse and decode read it
  (global $input.size i32 (i32.const 131107))
  ;; The ciphertext in hexadecimal and a newline.
  (global $out i32 (i32.const 197952))
  (global $out.size i32 (i32.const 131073))

  ;; Reads the key and the data, encrypts every block and prints the ciphertext, or says the
  ;; input isn't of the form. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $blocks i32)
    (local $b i32)
    (local $left i32)
    (block $failed
      (block $bad
        (local.set $got
          (call $fill (global.get $STDIN) (global.get $input) (global.get $input.size)
            (global.get $READ_TO_END)))
        (br_if $failed (i32.lt_s (local.get $got) (i32.const 0)))
        (if (i32.gt_u (local.get $got) (global.get $input.size))
          (then (call $assert_fail (i32.const 1)) unreachable))
        (local.set $blocks (call $parse (local.get $got)))
        (br_if $bad (i32.eqz (local.get $blocks)))
        (if (i32.gt_u (local.get $blocks) (global.get $MAX_BLOCKS))
          (then (call $assert_fail (i32.const 2)) unreachable))
        (call $expand_key)
        ;; Each block on its own, ECB, in place.
        (block $encrypted
          (local.set $left (global.get $MAX_BLOCKS))
          (loop $next  ;; bound: 4096
            (br_if $encrypted (i32.ge_u (local.get $b) (local.get $blocks)))
            (call $encrypt_block
              (i32.add (global.get $DATA_TO) (i32.shl (local.get $b) (global.get $BLOCK_SHIFT))))
            (local.set $b (i32.add (local.get $b) (i32.const 1)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $next (local.get $left))))
        ;; Every block is encrypted: MAX_BLOCKS steps are all it takes.
        (if (i32.ne (local.get $b) (local.get $blocks))
          (then (call $assert_fail (i32.const 3)) unreachable))
        (br_if $failed (call $print_hex (local.get $blocks)))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (br_if $failed
        (call $write_all (global.get $STDERR) (global.get $bad_input)
          (global.get $bad_input.rosize)))
      (call $proc_exit (global.get $EXIT_BAD_INPUT))
      unreachable)
    (call $proc_exit (global.get $EXIT_IO_FAILED))
    unreachable)

  ;; Checks the input's layout and decodes the key and the data into $bytes. Gives how many
  ;; blocks the data holds, or 0 when the input isn't a key and blocks of hexadecimal digits.
  (func $parse (param $len i32) (result i32)
    (local $digits i32)
    (local $blocks i32)
    (if (i32.gt_u (local.get $len) (global.get $input.size))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (block $bad
      ;; An input that fills the buffer is one byte too long.
      (br_if $bad (i32.ge_u (local.get $len) (global.get $input.size)))
      (br_if $bad (i32.lt_u (local.get $len) (global.get $MIN_INPUT)))
      (br_if $bad
        (i32.ne (i32.load8_u (i32.add (global.get $input) (global.get $KEY_HEX)))
          (global.get $NEWLINE)))
      (local.set $digits (i32.sub (local.get $len) (global.get $DATA_AT)))
      ;; The last newline may be absent.
      (if (i32.eq
            (i32.load8_u (i32.add (global.get $input) (i32.sub (local.get $len) (i32.const 1))))
            (global.get $NEWLINE))
        (then (local.set $digits (i32.sub (local.get $digits) (i32.const 1)))))
      (br_if $bad (i32.and (local.get $digits) (global.get $BLOCK_MASK)))
      (br_if $bad (i32.gt_u (local.get $digits) (global.get $HEX_MAX)))
      (br_if $bad (call $decode (global.get $KEY_AT) (global.get $KEY_HEX) (global.get $KEY_TO)))
      (br_if $bad (call $decode (global.get $DATA_AT) (local.get $digits) (global.get $DATA_TO)))
      (local.set $blocks (i32.shr_u (local.get $digits) (global.get $DIGITS_SHIFT)))
      ;; Data of HEX_MAX digits at most is MAX_BLOCKS blocks at most.
      (if (i32.gt_u (local.get $blocks) (global.get $MAX_BLOCKS))
        (then (call $assert_fail (i32.const 5)) unreachable)))
    (local.get $blocks))

  ;; Turns the $count hexadecimal digits at byte $from of the input into bytes, two digits a
  ;; byte, in $bytes from byte $to on. Every digit takes the same steps, a character that isn't
  ;; one too; one test at the end says whether all were digits. Gives 0, or ERROR_INVALID.
  (func $decode  ;; returns: result
    (param $from i32) (param $count i32) (param $to i32) (result i32)
    (local $k i32)
    (local $at i32)
    (local $high i32)
    (local $low i32)
    (local $invalid i32)
    (local $status i32)
    (local $left i32)
    (if (i32.gt_u (local.get $count) (global.get $HEX_MAX))
      (then (call $assert_fail (i32.const 6)) unreachable))
    ;; The key's bytes go at KEY_TO, the data's at DATA_TO.
    (if (i32.gt_u (local.get $to) (global.get $DATA_TO))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (block $done
      (local.set $left (global.get $MAX_BYTES))
      (loop $pair  ;; bound: 65536
        (br_if $done (i32.ge_u (i32.shl (local.get $k) (i32.const 1)) (local.get $count)))
        (local.set $at (i32.add (local.get $from) (i32.shl (local.get $k) (i32.const 1))))
        (if (i32.gt_u (local.get $at) (global.get $LAST_PAIR))
          (then (call $assert_fail (i32.const 8)) unreachable))
        (local.set $high
          (call $hex_value (i32.load8_u (i32.add (global.get $input) (local.get $at)))))
        (local.set $low
          (call $hex_value (i32.load8_u offset=1 (i32.add (global.get $input) (local.get $at)))))
        (local.set $invalid
          (i32.or (local.get $invalid) (i32.or (local.get $high) (local.get $low))))
        (local.set $at (i32.add (local.get $to) (local.get $k)))
        (if (i32.ge_u (local.get $at) (global.get $bytes.size))
          (then (call $assert_fail (i32.const 9)) unreachable))
        (i32.store8 (i32.add (global.get $bytes) (local.get $at))
          (i32.or (i32.shl (local.get $high) (global.get $NIBBLE_BITS)) (local.get $low)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $pair (local.get $left))))
    (local.set $status (global.get $NO_ERROR))
    ;; A character that isn't a digit leaves INVALID in the values taken together.
    (if (i32.and (local.get $invalid) (global.get $INVALID))
      (then (local.set $status (global.get $ERROR_INVALID))))
    (local.get $status))

  ;; The value of the hexadecimal digit $c, a byte, 0 to 15, with INVALID added where $c is no
  ;; digit. Each compare gives 1 or 0, and 0 less that is a mask of all ones or all zeros, so no
  ;; character takes a branch: a key's character takes no assertion's either.
  (func $hex_value (param $c i32) (result i32)
    (local $digit i32)
    (local $letter i32)
    (local $is_digit i32)
    (local $is_letter i32)
    ;; Below `0` or `a` wraps to a large number, so one unsigned compare finds each range.
    (local.set $digit (i32.sub (local.get $c) (global.get $ZERO)))
    (local.set $is_digit (i32.lt_u (local.get $digit) (global.get $DIGITS)))
    (local.set $letter
      (i32.sub (i32.or (local.get $c) (global.get $CASE_BIT)) (global.get $SMALL_A)))
    (local.set $is_letter (i32.lt_u (local.get $letter) (global.get $LETTERS)))
    (i32.or
      (i32.or
        (i32.and (local.get $digit) (i32.sub (i32.const 0) (local.get $is_digit)))
        (i32.and (i32.add (local.get $letter) (global.get $DIGITS))
          (i32.sub (i32.const 0) (local.get $is_letter))))
      (i32.mul (global.get $INVALID)
        (i32.eqz (i32.or (local.get $is_digit) (local.get $is_letter))))))

  ;; Expands the key, $bytes' first 16 bytes, into the 11 round keys, a word at a time: each
  ;; round key's first word is the last key's first word xor SubWord(RotWord(its last word)) xor
  ;; the round constant, and each other word the word before it xor the one four words back.
  ;; SubWord is $sub_bytes on the word's 4 bytes.
  (func $expand_key
    (local $at i32)
    (local $rcon i32)
    (local $word i32)
    (local $left i32)
    (memory.copy (global.get $round_keys) (global.get $bytes) (global.get $BLOCK))
    (local.set $rcon (global.get $FIRST_RCON))
    (local.set $left (global.get $EXPAND_STEPS))
    (loop $round  ;; bound: 10
      (if (i32.gt_u (local.get $at) (global.get $LAST_EXPAND))
        (then (call $assert_fail (i32.const 10)) unreachable))
      (if (i32.gt_u (local.get $rcon) (global.get $BYTE_MAX))
        (then (call $assert_fail (i32.const 11)) unreachable))
      ;; RotWord turns the last word's bytes one place, its first byte last.
      (local.set $word
        (i32.xor
          (i32.xor
            (i32x4.extract_lane 0
              (call $sub_bytes
                (i32x4.splat
                  (i32.rotr
                    (i32.load offset=12 (i32.add (global.get $round_keys) (local.get $at)))
                    (global.get $BYTE_BITS)))))
            (local.get $rcon))
          (i32.load (i32.add (global.get $round_keys) (local.get $at)))))
      (i32.store offset=16 (i32.add (global.get $round_keys) (local.get $at)) (local.get $word))
      (local.set $word
        (i32.xor (local.get $word)
          (i32.load offset=4 (i32.add (global.get $round_keys) (local.get $at)))))
      (i32.store offset=20 (i32.add (global.get $round_keys) (local.get $at)) (local.get $word))
      (local.set $word
        (i32.xor (local.get $word)
          (i32.load offset=8 (i32.add (global.get $round_keys) (local.get $at)))))
      (i32.store offset=24 (i32.add (global.get $round_keys) (local.get $at)) (local.get $word))
      (local.set $word
        (i32.xor (local.get $word)
          (i32.load offset=12 (i32.add (global.get $round_keys) (local.get $at)))))
      (i32.store offset=28 (i32.add (global.get $round_keys) (local.get $at)) (local.get $word))
      ;; The next round constant: doubled, the polynomial added where the top bit moves out.
      (local.set $rcon
        (i32.and
          (i32.xor (i32.shl (local.get $rcon) (i32.const 1))
            (i32.mul (global.get $AES_POLY) (i32.shr_u (local.get $rcon) (global.get $TOP_BIT))))
          (global.get $BYTE_MAX)))
      (local.set $at (i32.add (local.get $at) (global.get $BLOCK)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $round (local.get $left))))

  ;; SubBytes on the 16 bytes of $x at once: the inverse in GF(2^8), 0 for 0, then the affine map
  ;; of FIPS 197, b xor b turned left by 1, 2, 3 and 4 bits, xor AFFINE. The inverse is x to the
  ;; 254th: from x, each step squares and multiplies by x, so 6 steps make x to the 127th, and a
  ;; square makes the 254th.
  (func $sub_bytes (param $x v128) (result v128)
    (local $power v128)
    (local $b v128)
    (local $left i32)
    (local.set $power (local.get $x))
    (local.set $left (global.get $INVERSE_STEPS))
    (loop $step  ;; bound: 6
      (local.set $power
        (call $multiply (call $multiply (local.get $power) (local.get $power)) (local.get $x)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $step (local.get $left)))
    (local.set $b (call $multiply (local.get $power) (local.get $power)))
    (v128.xor
      (v128.xor
        (v128.xor (local.get $b)
          (v128.or (i8x16.shl (local.get $b) (i32.const 1))
            (i8x16.shr_u (local.get $b) (i32.const 7))))
        (v128.xor
          (v128.or (i8x16.shl (local.get $b) (i32.const 2))
            (i8x16.shr_u (local.get $b) (i32.const 6)))
          (v128.or (i8x16.shl (local.get $b) (i32.const 3))
            (i8x16.shr_u (local.get $b) (i32.const 5)))))
      (v128.xor
        (v128.or (i8x16.shl (local.get $b) (i32.const 4))
          (i8x16.shr_u (local.get $b) (i32.const 4)))
        (i8x16.splat (global.get $AFFINE)))))

  ;; The product of $a and $b in GF(2^8), with FIPS 197's polynomial x^8 + x^4 + x^3 + x + 1, for
  ;; each of the 16 bytes at once: for each bit of b, from the lowest, a is added where the bit
  ;; is 1, then a is doubled. Two shifts make a bit a mask of its byte, so no byte takes a branch.
  (func $multiply (param $a v128) (param $b v128) (result v128)
    (local $product v128)
    (local $poly v128)
    (local $left i32)
    (local.set $poly (i8x16.splat (global.get $AES_POLY)))
    (local.set $left (global.get $BITS))
    (loop $bit  ;; bound: 8
      ;; b's lowest bit moved to the top, and spread over its byte by the signed shift.
      (local.set $product
        (v128.xor (local.get $product)
          (v128.and (local.get $a)
            (i8x16.shr_s (i8x16.shl (local.get $b) (i32.const 7)) (i32.const 7)))))
      ;; a times x: shifted up, the polynomial added where its top bit moves out.
      (local.set $a
        (v128.xor (i8x16.shl (local.get $a) (i32.const 1))
          (v128.and (i8x16.shr_s (local.get $a) (i32.const 7)) (local.get $poly))))
      (local.set $b (i8x16.shr_u (local.get $b) (i32.const 1)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $bit (local.get $left)))
    (local.get $product))

  ;; Encrypts the block at byte $at of $bytes in place: the first round key added, 9 rounds of
  ;; SubBytes, ShiftRows, MixColumns and the round's key, then the last round, which has no
  ;; MixColumns. ShiftRows moves row r r columns to the left: byte r + 4c takes byte
  ;; r + 4((c + r) mod 4). MixColumns makes each column's byte r 2 a(r) xor 3 a(r+1) xor a(r+2)
  ;; xor a(r+3), which is 2 (a(r) xor a(r+1)) xor a(r+1) xor a(r+2) xor a(r+3): the column turned
  ;; by one, two and three bytes.
  (func $encrypt_block (param $at i32)
    (local $state v128)
    (local $turned v128)
    (local $twice v128)
    (local $poly v128)
    (local $key i32)
    (local $left i32)
    (if (i32.gt_u (local.get $at) (global.get $LAST_BLOCK))
      (then (call $assert_fail (i32.const 12)) unreachable))
    (if (i32.and (local.get $at) (global.get $BLOCK_ALIGN))
      (then (call $assert_fail (i32.const 13)) unreachable))
    (local.set $state
      (v128.xor (v128.load (i32.add (global.get $bytes) (local.get $at)))
        (v128.load (global.get $round_keys))))
    (local.set $poly (i8x16.splat (global.get $AES_POLY)))
    (local.set $key (global.get $BLOCK))
    (local.set $left (global.get $MIDDLE_ROUNDS))
    (loop $round  ;; bound: 9
      (if (i32.gt_u (local.get $key) (global.get $LAST_ROUND_KEY))
        (then (call $assert_fail (i32.const 14)) unreachable))
      (local.set $state (call $sub_bytes (local.get $state)))
      (local.set $state
        (i8x16.shuffle 0 5 10 15 4 9 14 3 8 13 2 7 12 1 6 11 (local.get $state) (local.get $state)))
      (local.set $turned
        (i8x16.shuffle 1 2 3 0 5 6 7 4 9 10 11 8 13 14 15 12 (local.get $state) (local.get $state)))
      (local.set $twice (v128.xor (local.get $state) (local.get $turned)))
      (local.set $state
        (v128.xor
          (v128.xor
            (v128.xor (i8x16.shl (local.get $twice) (i32.const 1))
              (v128.and (i8x16.shr_s (local.get $twice) (i32.const 7)) (local.get $poly)))
            (local.get $turned))
          (v128.xor
            (i8x16.shuffle 2 3 0 1 6 7 4 5 10 11 8 9 14 15 12 13
              (local.get $state) (local.get $state))
            (i8x16.shuffle 3 0 1 2 7 4 5 6 11 8 9 10 15 12 13 14
              (local.get $state) (local.get $state)))))
      (local.set $state
        (v128.xor (local.get $state)
          (v128.load (i32.add (global.get $round_keys) (local.get $key)))))
      (local.set $key (i32.add (local.get $key) (global.get $BLOCK)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $round (local.get $left)))
    ;; The last round: SubBytes, ShiftRows and the last key, at byte 160.
    (local.set $state (call $sub_bytes (local.get $state)))
    (v128.store (i32.add (global.get $bytes) (local.get $at))
      (v128.xor
        (i8x16.shuffle 0 5 10 15 4 9 14 3 8 13 2 7 12 1 6 11 (local.get $state) (local.get $state))
        (v128.load offset=160 (global.get $round_keys)))))

  ;; Prints the $blocks encrypted blocks, from byte DATA_TO of $bytes on, as lowercase
  ;; hexadecimal digits and a newline: a digit is `0` plus its value, and LETTER_GAP more for a
  ;; to f. Gives 0, or an error number.
  (func $print_hex (param $blocks i32) (result i32)  ;; returns: result
    (local $k i32)
    (local $count i32)
    (local $at i32)
    (local $to i32)
    (local $byte i32)
    (local $digit i32)
    (local $left i32)
    (if (i32.gt_u (local.get $blocks) (global.get $MAX_BLOCKS))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (local.set $count (i32.shl (local.get $blocks) (global.get $BLOCK_SHIFT)))
    ;; MAX_BLOCKS blocks are MAX_BYTES bytes.
    (if (i32.gt_u (local.get $count) (global.get $MAX_BYTES))
      (then (call $assert_fail (i32.const 16)) unreachable))
    (block $done
      (local.set $left (global.get $MAX_BYTES))
      (loop $next  ;; bound: 65536
        (br_if $done (i32.ge_u (local.get $k) (local.get $count)))
        (local.set $at (i32.add (global.get $DATA_TO) (local.get $k)))
        (if (i32.ge_u (local.get $at) (global.get $bytes.size))
          (then (call $assert_fail (i32.const 17)) unreachable))
        (local.set $byte (i32.load8_u (i32.add (global.get $bytes) (local.get $at))))
        (local.set $to (i32.shl (local.get $k) (i32.const 1)))
        (if (i32.gt_u (local.get $to) (global.get $LAST_PAIR_OUT))
          (then (call $assert_fail (i32.const 18)) unreachable))
        (local.set $digit (i32.shr_u (local.get $byte) (global.get $NIBBLE_BITS)))
        (i32.store8 (i32.add (global.get $out) (local.get $to))
          (i32.add (i32.add (local.get $digit) (global.get $ZERO))
            (i32.mul (i32.gt_u (local.get $digit) (global.get $NINE)) (global.get $LETTER_GAP))))
        (local.set $digit (i32.and (local.get $byte) (global.get $NIBBLE_MASK)))
        (i32.store8 offset=1 (i32.add (global.get $out) (local.get $to))
          (i32.add (i32.add (local.get $digit) (global.get $ZERO))
            (i32.mul (i32.gt_u (local.get $digit) (global.get $NINE)) (global.get $LETTER_GAP))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $next (local.get $left))))
    ;; The newline after the digits.
    (local.set $to (i32.shl (local.get $count) (i32.const 1)))
    (if (i32.ge_u (local.get $to) (global.get $out.size))
      (then (call $assert_fail (i32.const 19)) unreachable))
    (i32.store8 (i32.add (global.get $out) (local.get $to)) (global.get $NEWLINE))
    (call $write_all (global.get $STDOUT) (global.get $out)
      (i32.add (local.get $to) (i32.const 1))))
)
