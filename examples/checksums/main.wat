;; checksums: prints the CRC-32 of each line of standard input. It is made from three parts of
;; lengua's library, each function copied word for word under its `;; from:` line, with the
;; constants it reads: $lines_read and $lines_end from the part lines, $crc32_update from the
;; part crc32, and $decimal_format from the part decimal. check holds each copy to its part. In
;; the WebAssembly dialect 1 (references/power-of-10-webassembly.md).
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
  ;; The lines a text of LINES_TEXT bytes has at most, and one more turn to see its end.
  (global $MAX_LINES i32 (i32.const 4097))
  ;; The CRC-32 of no bytes, which a CRC starts from.
  (global $CRC_OF_NOTHING i32 (i32.const 0))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))

  ;; The constants of the part lines, as that part defines them.
  (global $LINES_TEXT i32 (i32.const 4096))
  (global $LINES_ROOM i32 (i32.const 4097))
  (global $LINES_TO_END i32 (i32.const -1))
  (global $LINES_NEWLINE i32 (i32.const 10))

  ;; The constants of the part crc32, as that part defines them.
  (global $CRC32_TEXT i32 (i32.const 4096))
  (global $CRC32_POLY i32 (i32.const 0xedb88320))
  (global $CRC32_ALL_ONES i32 (i32.const -1))
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
  ;; Standard input, which main lends $lines_read to write, and $lines_end and $crc32_update to
  ;; read: LINES_TEXT bytes, and CRC32_TEXT bytes of room after the last place a line can start,
  ;; rounded up.
  (global $text i32 (i32.const 2048))
  (global $text.size i32 (i32.const 8256))
  ;; The output, which print_number lends $decimal_format to write: a number's text, then a
  ;; newline.
  (global $out i32 (i32.const 10304))
  (global $out.size i32 (i32.const 21))

  ;; Reads standard input and prints the CRC-32 of each of its lines, or says why it can't.
  ;; Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $from i32)
    (local $end i32)
    (local $crc i32)
    (local $left i32)
    (block $write_failed
      (block $read_failed
        (block $too_long
          (local.set $got (call $lines_read (global.get $STDIN) (global.get $text)))
          (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
          (if (i32.gt_u (local.get $got) (global.get $LINES_ROOM))
            (then (call $assert_fail (i32.const 1)) unreachable))
          (br_if $too_long (i32.gt_u (local.get $got) (global.get $LINES_TEXT)))
          (block $done
            (local.set $left (global.get $MAX_LINES))
            (loop $line  ;; bound: 4097
              ;; No byte is left: the line before was the last.
              (br_if $done (i32.ge_u (local.get $from) (local.get $got)))
              ;; A line starts inside the text, so the CRC32_TEXT bytes $crc32_update may read
              ;; from there are inside $text.
              (if (i32.ge_u (local.get $from) (global.get $LINES_TEXT))
                (then (call $assert_fail (i32.const 2)) unreachable))
              (local.set $end
                (call $lines_end (global.get $text) (local.get $from) (local.get $got)))
              (if (i32.gt_u (local.get $end) (global.get $LINES_TEXT))
                (then (call $assert_fail (i32.const 3)) unreachable))
              (local.set $crc
                (call $crc32_update (global.get $CRC_OF_NOTHING)
                  (i32.add (global.get $text) (local.get $from))
                  (i32.sub (local.get $end) (local.get $from))))
              (br_if $write_failed (call $print_number (i64.extend_i32_u (local.get $crc))))
              ;; The next line starts after this one's newline.
              (local.set $from (i32.add (local.get $end) (i32.const 1)))
              (local.set $left (i32.sub (local.get $left) (i32.const 1)))
              (br_if $line (local.get $left))))
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

  ;; Writes $number in decimal, then a newline, to standard output. Gives 0, or an error number.
  (func $print_number (param $number i64) (result i32)  ;; returns: result
    (local $length i32)
    (local.set $length (call $decimal_format (local.get $number) (global.get $out)))
    (if (i32.gt_u (local.get $length) (global.get $DECIMAL_TEXT))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (i32.store8 (i32.add (global.get $out) (local.get $length)) (global.get $NEWLINE))
    (call $write_all (global.get $STDOUT) (global.get $out)
      (i32.add (local.get $length) (i32.const 1))))

  ;; from: ../lines
  (func $lines_read (param $fd i32) (param $text i32) (result i32)
    (local $got i32)
    (block $failed
      (local.set $got
        (call $fill (local.get $fd) (local.get $text) (global.get $LINES_ROOM)
          (global.get $LINES_TO_END)))
      ;; A negative count is an error number, which the caller gets as it is.
      (br_if $failed (i32.lt_s (local.get $got) (i32.const 0)))
      (if (i32.gt_u (local.get $got) (global.get $LINES_ROOM))
        (then (call $assert_fail (i32.const 824)) unreachable)))
    (local.get $got))

  ;; from: ../lines
  (func $lines_end (param $text i32) (param $from i32) (param $length i32) (result i32)
    (local $i i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $LINES_TEXT))
      (then (call $assert_fail (i32.const 821)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $length))
      (then (call $assert_fail (i32.const 822)) unreachable))
    (local.set $i (local.get $from))
    (block $found
      (local.set $left (global.get $LINES_TEXT))
      (loop $byte  ;; bound: 4096
        (br_if $found (i32.ge_u (local.get $i) (local.get $length)))
        (if (i32.ge_u (local.get $i) (global.get $LINES_TEXT))
          (then (call $assert_fail (i32.const 823)) unreachable))
        (br_if $found
          (i32.eq (i32.load8_u (i32.add (local.get $text) (local.get $i)))
            (global.get $LINES_NEWLINE)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $i))

  ;; from: ../crc32
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
