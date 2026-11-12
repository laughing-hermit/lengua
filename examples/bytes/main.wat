;; bytes: a part of lengua's library. Its idea's contract offers one function to the programs
;; that build on it: $bytes_equal says if two runs of bytes are the same. It works only through
;; the two addresses it is lent and reads only the $BYTES_ constant below. The part's own program
;; says if the first two lines of standard input are the same, so its tests and its proofs go
;; through the function; it reads the input and finds the lines with $lines_read and $lines_end,
;; which it takes from the part lines. In the WebAssembly dialect 1
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
  ;; Where the first line starts.
  (global $START i32 (i32.const 0))

  ;; The part's constant, which its function reads. A program that takes the function defines
  ;; it as it is here: the most bytes one call compares.
  (global $BYTES_TEXT i32 (i32.const 4096))

  ;; The constants of the part lines, as that part defines them.
  (global $LINES_TEXT i32 (i32.const 4096))
  (global $LINES_ROOM i32 (i32.const 4097))
  (global $LINES_TO_END i32 (i32.const -1))
  (global $LINES_NEWLINE i32 (i32.const 10))

  ;; Regions (rule 3, M1), from 1024 up: the three lines it prints, read-only, then the text.
  (global $same i32 (i32.const 1024))
  (global $same.rosize i32 (i32.const 5))
  (data (i32.const 1024) "same\0a")
  (global $different i32 (i32.const 1040))
  (global $different.rosize i32 (i32.const 10))
  (data (i32.const 1040) "different\0a")
  (global $too_long i32 (i32.const 1056))
  (global $too_long.rosize i32 (i32.const 22))
  (data (i32.const 1056) "error: input too long\0a")
  ;; Standard input, which main lends $lines_read to write, and $lines_end and $bytes_equal to
  ;; read: LINES_TEXT bytes, and BYTES_TEXT bytes of room after the last place the second line
  ;; can start, rounded up.
  (global $text i32 (i32.const 2048))
  (global $text.size i32 (i32.const 8256))

  ;; Reads standard input and says if its first two lines are the same, or says why it can't.
  ;; Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $first i32)
    (local $from i32)
    (local $end i32)
    (block $write_failed
      (block $read_failed
        (block $too_long
          (block $differ
            (local.set $got (call $lines_read (global.get $STDIN) (global.get $text)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $LINES_ROOM))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (br_if $too_long (i32.gt_u (local.get $got) (global.get $LINES_TEXT)))
            ;; The first line: from the text's start to $first.
            (local.set $first
              (call $lines_end (global.get $text) (global.get $START) (local.get $got)))
            (if (i32.gt_u (local.get $first) (global.get $LINES_TEXT))
              (then (call $assert_fail (i32.const 2)) unreachable))
            ;; The second line starts after the first one's newline, or at the text's end when
            ;; the first line has none.
            (local.set $from (i32.add (local.get $first) (i32.const 1)))
            (if (i32.gt_u (local.get $from) (local.get $got))
              (then (local.set $from (local.get $got))))
            (if (i32.gt_u (local.get $from) (global.get $LINES_TEXT))
              (then (call $assert_fail (i32.const 3)) unreachable))
            (local.set $end
              (call $lines_end (global.get $text) (local.get $from) (local.get $got)))
            (if (i32.gt_u (local.get $end) (global.get $LINES_TEXT))
              (then (call $assert_fail (i32.const 4)) unreachable))
            ;; Lines of two lengths differ.
            (br_if $differ
              (i32.ne (local.get $first) (i32.sub (local.get $end) (local.get $from))))
            (br_if $differ
              (i32.eqz
                (call $bytes_equal (global.get $text)
                  (i32.add (global.get $text) (local.get $from)) (local.get $first))))
            (br_if $write_failed
              (call $write_all (global.get $STDOUT) (global.get $same)
                (global.get $same.rosize)))
            (call $proc_exit (global.get $EXIT_OK))
            unreachable)
          (br_if $write_failed
            (call $write_all (global.get $STDOUT) (global.get $different)
              (global.get $different.rosize)))
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

  ;; Gives 1 when the $length bytes at $a and the $length bytes at $b are the same, else 0. It
  ;; stops at the first byte that differs, so it isn't for bytes that are secret. $length is at
  ;; most BYTES_TEXT. Its caller lends $a and $b with room for BYTES_TEXT bytes each, whatever
  ;; $length is; the two may be the same bytes.
  (func $bytes_equal (param $a i32) (param $b i32) (param $length i32) (result i32)
    (local $i i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $BYTES_TEXT))
      (then (call $assert_fail (i32.const 841)) unreachable))
    (block $done
      (local.set $left (global.get $BYTES_TEXT))
      (loop $byte  ;; bound: 4096
        (br_if $done (i32.ge_u (local.get $i) (local.get $length)))
        (if (i32.ge_u (local.get $i) (global.get $BYTES_TEXT))
          (then (call $assert_fail (i32.const 842)) unreachable))
        (br_if $done
          (i32.ne (i32.load8_u (i32.add (local.get $a) (local.get $i)))
            (i32.load8_u (i32.add (local.get $b) (local.get $i)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    ;; It went past every byte only when none differed.
    (i32.ge_u (local.get $i) (local.get $length)))

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
)
