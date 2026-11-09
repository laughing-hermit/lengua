;; secret-equal: prints whether the two lines of standard input are the same, with no branch on
;; their bytes. A fixture of the secret rule, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md and references/constant-time.md): lengua's library
;; gives $assert_fail, $fill and $write_all. Standard input is secret, so $differ folds every byte
;; it reads into one word of differences, and the answer is chosen from that word with a mask.
;; How many bytes came isn't secret: the lines are found from it, and every branch tests it.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_READ_FAILED i32 (i32.const 2))
  (global $EXIT_WRITE_FAILED i32 (i32.const 3))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; Sizes and limits: two lines of at most 64 characters, each with its newline.
  (global $MAX_INPUT i32 (i32.const 130))
  (global $MAX_HALF i32 (i32.const 65))
  ;; The bit an odd count has, and the shift that halves a count.
  (global $ODD i32 (i32.const 1))
  (global $HALVE i32 (i32.const 1))
  ;; Every bit set: what a mask's xor with it turns into the other mask.
  (global $ALL_ONES i32 (i32.const -1))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then the input and the answer.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 36))
  (data (i32.const 1024) "usage: two lines of the same length\0a")
  (global $same i32 (i32.const 1088))
  (global $same.rosize i32 (i32.const 4))
  (data (i32.const 1088) "same")
  (global $diff i32 (i32.const 1104))
  (global $diff.rosize i32 (i32.const 4))
  (data (i32.const 1104) "diff")
  ;; The input, one byte past the longest, so that input that is too long shows.
  (global $text i32 (i32.const 2048))  ;; shared: main has $fill fill it; differ reads it
  (global $text.size i32 (i32.const 131))
  ;; The answer: a word and a newline.
  (global $out i32 (i32.const 2304))
  (global $out.size i32 (i32.const 5))

  ;; Reads the two lines and prints whether they are the same, or the usage line. Never returns:
  ;; every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $half i32)
    (local $mask i32)
    (block $write_failed
      (block $read_failed
        (block $usage
          (local.set $got
            (call $fill (global.get $STDIN) (global.get $text) (global.get $text.size)
              (global.get $READ_TO_END)))
          (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
          (if (i32.gt_u (local.get $got) (global.get $text.size))
            (then (call $assert_fail (i32.const 1)) unreachable))
          ;; Two lines of the same length are an even count of bytes, at most 130.
          (br_if $usage (i32.eqz (local.get $got)))
          (br_if $usage (i32.and (local.get $got) (global.get $ODD)))
          (br_if $usage (i32.gt_u (local.get $got) (global.get $MAX_INPUT)))
          (local.set $half (i32.shr_u (local.get $got) (global.get $HALVE)))
          (if (i32.gt_u (local.get $half) (global.get $MAX_HALF))
            (then (call $assert_fail (i32.const 2)) unreachable))
          ;; 0 - 1 is all ones, the mask that keeps "same"; 0 - 0 is the mask that keeps "diff".
          (local.set $mask
            (i32.sub (i32.const 0) (i32.eqz (call $differ (local.get $half)))))
          (i32.store (global.get $out)
            (i32.or
              (i32.and (i32.load (global.get $same)) (local.get $mask))
              (i32.and (i32.load (global.get $diff))
                (i32.xor (local.get $mask) (global.get $ALL_ONES)))))
          (i32.store8 offset=4 (global.get $out) (global.get $NEWLINE))
          (br_if $write_failed
            (call $write_all (global.get $STDOUT) (global.get $out) (global.get $out.size)))
          (call $proc_exit (global.get $EXIT_OK))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
        (call $proc_exit (global.get $EXIT_USAGE))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Gives the or of the xor of each of the first $half bytes of $text with the byte $half places
  ;; after it: 0 when the two halves are the same. It reads every byte the same way, so no byte
  ;; decides a branch, an address or how many times the loop runs.
  (func $differ (param $half i32) (result i32)
    (local $i i32)
    (local $bits i32)
    (local $left i32)
    (if (i32.gt_u (local.get $half) (global.get $MAX_HALF))
      (then (call $assert_fail (i32.const 3)) unreachable))
    (block $done
      (local.set $left (global.get $MAX_HALF))
      (loop $byte  ;; bound: 65
        (br_if $done (i32.ge_u (local.get $i) (local.get $half)))
        (if (i32.ge_u (local.get $i) (global.get $MAX_HALF))
          (then (call $assert_fail (i32.const 4)) unreachable))
        (local.set $bits
          (i32.or (local.get $bits)
            (i32.xor
              (i32.load8_u (i32.add (global.get $text) (local.get $i)))
              (i32.load8_u
                (i32.add (global.get $text) (i32.add (local.get $half) (local.get $i)))))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $bits))
)
