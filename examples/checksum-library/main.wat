;; checksum-library: prints the CRC-32 of standard input, which another lengua program computes:
;; crc32-library, whose idea offers two functions with an `- Exports:` line. The idea's
;; `- Calls:` line links to it. lengua starts its module as an instance of its own, with a
;; memory of its own and no function of the host: main copies its bytes there with `give`, to
;; the address $buffer gives, and $crc32 gives the CRC-32 back as a number. lengua checked the
;; library on this computer. main prints the number with $decimal_format, which it takes from
;; the part decimal. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md).
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (import "lengua_call_1" "give" (func $give (param i32 i32 i32 i32) (result i32)))
  (import "crc32-library" "buffer" (func $buffer (result i32)))
  (import "crc32-library" "crc32" (func $crc32 (param i32) (result i32)))
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
  (global $EXIT_CALL_FAILED i32 (i32.const 5))
  ;; The stop byte that has $fill read until its room is full or the input ends, and the bytes
  ;; it reads at most: one more than the text may have, which says the input is longer.
  (global $READ_TO_END i32 (i32.const -1))
  (global $READ_MAX i32 (i32.const 4097))
  ;; The most bytes of text: what the called module's buffer holds.
  (global $TEXT_MAX i32 (i32.const 4096))
  ;; The called module's number for `give`: the idea's first `- Calls:` line is number 0.
  (global $CRC32_MODULE i32 (i32.const 0))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))

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
  ;; Standard input, which main has $fill write and lends `give` to read: READ_MAX bytes,
  ;; rounded up.
  (global $text i32 (i32.const 2048))
  (global $text.size i32 (i32.const 4160))
  ;; The output, which main lends $decimal_format to write: a number's text, then a newline.
  (global $out i32 (i32.const 6208))
  (global $out.size i32 (i32.const 21))

  ;; Reads standard input, has the called module compute its CRC-32 and prints it, or says why
  ;; it can't. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $crc i32)
    (local $length i32)
    (block $call_failed
      (block $write_failed
        (block $read_failed
          (block $too_long
            (local.set $got
              (call $fill (global.get $STDIN) (global.get $text) (global.get $READ_MAX)
                (global.get $READ_TO_END)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $READ_MAX))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (br_if $too_long (i32.gt_u (local.get $got) (global.get $TEXT_MAX)))
            ;; The bytes go to the called module by copy, to the address it gives: its memory
            ;; is its own. Then the call passes a number, and gets a number back.
            (br_if $call_failed
              (call $give (global.get $CRC32_MODULE) (call $buffer) (global.get $text)
                (local.get $got)))
            (local.set $crc (call $crc32 (local.get $got)))
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
    (call $proc_exit (global.get $EXIT_CALL_FAILED))
    unreachable)

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
