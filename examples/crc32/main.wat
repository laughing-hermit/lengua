;; crc32: a part of lengua's library. Its idea's contract offers one function to the programs
;; that build on it: $crc32_update, the CRC-32 that zlib, gzip and PNG compute, a bit at a time
;; with no table, so it works only through the address it is lent and reads only the $CRC32_
;; constants below. The part's own program prints the CRC-32 of standard input, so its tests
;; and its proofs go through the function; it prints the number with $decimal_format, which it
;; takes from the part decimal. In the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md).
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_TOO_LONG i32 (i32.const 2))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  ;; The stop byte that has $fill read until its room is full or the input ends, and the bytes
  ;; it reads at most: one more than the text may have, which says the input is longer.
  (global $READ_TO_END i32 (i32.const -1))
  (global $READ_MAX i32 (i32.const 4097))
  ;; The CRC-32 of no bytes, which a CRC starts from.
  (global $CRC_OF_NOTHING i32 (i32.const 0))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))

  ;; The part's constants, which its function reads. A program that takes the function defines
  ;; each of them as it is here.
  ;; The most bytes one call takes.
  (global $CRC32_TEXT i32 (i32.const 4096))
  ;; The CRC-32's polynomial, its bits reflected, and the value the CRC is xored with at its
  ;; start and at its end: all ones.
  (global $CRC32_POLY i32 (i32.const 0xedb88320))
  (global $CRC32_ALL_ONES i32 (i32.const -1))
  ;; The bits of a byte.
  (global $CRC32_BITS i32 (i32.const 8))

  ;; The constants of the part decimal that $decimal_format reads, as that part defines them.
  (global $DECIMAL_TEXT i32 (i32.const 20))
  (global $DECIMAL_DIGITS i32 (i32.const 19))
  (global $DECIMAL_TEN i64 (i64.const 10))
  (global $DECIMAL_MINUS i32 (i32.const 45))
  (global $DECIMAL_ZERO i32 (i32.const 48))

  ;; Regions (rule 3, M1), from 1024 up: the message, read-only, then the text and the output.
  (global $too_long i32 (i32.const 1024))
  (global $too_long.rosize i32 (i32.const 22))
  (data (i32.const 1024) "error: input too long\0a")
  ;; Standard input, which main has $fill write and lends $crc32_update to read: READ_MAX
  ;; bytes, rounded up.
  (global $text i32 (i32.const 2048))
  (global $text.size i32 (i32.const 4160))
  ;; The output, which main lends $decimal_format to write: a number's text, then a newline.
  (global $out i32 (i32.const 6208))
  (global $out.size i32 (i32.const 21))

  ;; Reads standard input and prints its CRC-32, or says why it can't. Never returns: every path
  ;; ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $crc i32)
    (local $length i32)
    (block $write_failed
      (block $read_failed
        (block $too_long
          (local.set $got
            (call $fill (global.get $STDIN) (global.get $text) (global.get $READ_MAX)
              (global.get $READ_TO_END)))
          (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
          (if (i32.gt_u (local.get $got) (global.get $READ_MAX))
            (then (call $assert_fail (i32.const 1)) unreachable))
          (br_if $too_long (i32.gt_u (local.get $got) (global.get $CRC32_TEXT)))
          (local.set $crc
            (call $crc32_update (global.get $CRC_OF_NOTHING) (global.get $text) (local.get $got)))
          ;; A CRC-32 is 32 bits with no sign: at most 10 digits.
          (local.set $length
            (call $decimal_format (i64.extend_i32_u (local.get $crc)) (global.get $out)))
          (if (i32.gt_u (local.get $length) (global.get $DECIMAL_TEXT))
            (then (call $assert_fail (i32.const 2)) unreachable))
          (i32.store8 (i32.add (global.get $out) (local.get $length)) (global.get $NEWLINE))
          (br_if $write_failed
            (call $write_all (global.get $STDOUT) (global.get $out)
              (i32.add (local.get $length) (i32.const 1))))
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

  ;; Gives the CRC-32 of some bytes and then the $length bytes at $at, $crc being the CRC-32 of
  ;; the bytes before them, 0 when there are none: the CRC-32 of zlib, gzip and PNG. Each byte is
  ;; xored into the CRC, and then, eight times, the CRC moves one bit down and the polynomial
  ;; comes in when the bit that left was set, through a mask, with no branch on the data.
  ;; $length is at most CRC32_TEXT. Its caller lends $at with room for CRC32_TEXT bytes, whatever
  ;; $length is.
  (func $crc32_update (param $crc i32) (param $at i32) (param $length i32) (result i32)
    (local $i i32)
    (local $c i32)
    (local $bit i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $CRC32_TEXT))
      (then (call $assert_fail (i32.const 831)) unreachable))
    (local.set $c (i32.xor (local.get $crc) (global.get $CRC32_ALL_ONES)))
    (block $done
      (local.set $left (global.get $CRC32_TEXT))
      (loop $byte  ;; bound: 4096
        (br_if $done (i32.ge_u (local.get $i) (local.get $length)))
        (if (i32.ge_u (local.get $i) (global.get $CRC32_TEXT))
          (then (call $assert_fail (i32.const 832)) unreachable))
        (local.set $c
          (i32.xor (local.get $c) (i32.load8_u (i32.add (local.get $at) (local.get $i)))))
        (local.set $bit (global.get $CRC32_BITS))
        (loop $shift  ;; bound: 8
          ;; All ones when the lowest bit is set, else none: what of the polynomial comes in.
          (local.set $c
            (i32.xor (i32.shr_u (local.get $c) (i32.const 1))
              (i32.and (global.get $CRC32_POLY)
                (i32.sub (i32.const 0) (i32.and (local.get $c) (i32.const 1))))))
          (local.set $bit (i32.sub (local.get $bit) (i32.const 1)))
          (br_if $shift (local.get $bit)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (i32.xor (local.get $c) (global.get $CRC32_ALL_ONES)))

  ;; Writes $value in decimal at $into: a minus sign when it is negative, then its digits, with
  ;; no zero at the start but for 0 itself; at most DECIMAL_TEXT bytes. Gives how many bytes it
  ;; wrote. $value is never the most negative number, which has no magnitude in 64 bits.
  ;; from: ../decimal
  (func $decimal_format (param $value i64) (param $into i32) (result i32)
    (local $magnitude i64)
    (local $rest i64)
    (local $length i32)
    (local $at i32)
    (local $left i32)
    (local.set $magnitude (local.get $value))
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then
        (local.set $magnitude (i64.sub (i64.const 0) (local.get $value)))
        ;; The minus sign's byte.
        (local.set $length (i32.add (local.get $length) (i32.const 1)))))
    (if (i64.lt_s (local.get $magnitude) (i64.const 0))
      (then (call $assert_fail (i32.const 802)) unreachable))
    ;; How many digits: one, and one more for each time the magnitude divides by ten.
    (local.set $rest (local.get $magnitude))
    (block $counted
      (local.set $left (global.get $DECIMAL_DIGITS))
      (loop $count  ;; bound: 19
        (local.set $length (i32.add (local.get $length) (i32.const 1)))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $DECIMAL_TEN)))
        (br_if $counted (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $count (local.get $left)))
      ;; A magnitude below 2 to the 63rd has at most 19 digits.
      (call $assert_fail (i32.const 803))
      unreachable)
    ;; The digits, from the last to the first.
    (local.set $at (local.get $length))
    (block $written
      (local.set $left (global.get $DECIMAL_DIGITS))
      (loop $digit  ;; bound: 19
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $DECIMAL_TEXT))
          (then (call $assert_fail (i32.const 804)) unreachable))
        (i32.store8 (i32.add (local.get $into) (local.get $at))
          (i32.add (global.get $DECIMAL_ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $magnitude) (global.get $DECIMAL_TEN)))))
        (local.set $magnitude (i64.div_u (local.get $magnitude) (global.get $DECIMAL_TEN)))
        (br_if $written (i64.eqz (local.get $magnitude)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; The loop above counted these digits.
      (call $assert_fail (i32.const 805))
      unreachable)
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then (i32.store8 (local.get $into) (global.get $DECIMAL_MINUS))))
    (local.get $length))
)
