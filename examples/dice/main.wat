;; dice: rolls a fair six-sided die and prints the roll. The WebAssembly counterpart of
;; examples/dice, in the WebAssembly dialect 1 (references/power-of-10-webassembly.md): lengua's
;; library gives $assert_fail and $write_all. It draws 64 random bytes at once and rolls with the
;; first below 252, 42 times the faces, so each face has the same chance.
(module
  (import "wasi_snapshot_preview1" "random_get"
    (func $random_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_WRITE_FAILED i32 (i32.const 2))
  (global $EXIT_NO_RANDOM i32 (i32.const 3))
  ;; The bytes from SET_ASIDE_FROM to 255 are set aside, so every face has 42 bytes.
  (global $SET_ASIDE_FROM i32 (i32.const 252))
  (global $FACES i32 (i32.const 6))
  ;; Characters: the first face's digit, `1`, and the newline; where the roll's digit and the
  ;; newline go in the line.
  (global $FIRST_FACE i32 (i32.const 49))
  (global $NEWLINE i32 (i32.const 10))
  (global $ROLL_AT i32 (i32.const 7))
  (global $NEWLINE_AT i32 (i32.const 8))

  ;; Regions (rule 3, M1), from 1024 up: the line's start, read-only, then the random bytes drawn
  ;; for one roll and the line printed.
  (global $rolled i32 (i32.const 1024))
  (global $rolled.rosize i32 (i32.const 7))
  (data (i32.const 1024) "rolled ")
  (global $random i32 (i32.const 1040))
  (global $random.size i32 (i32.const 64))
  (global $line i32 (i32.const 1104))
  (global $line.size i32 (i32.const 9))

  ;; Draws the random bytes, rolls with the first below SET_ASIDE_FROM and prints the roll, or
  ;; exits with EXIT_NO_RANDOM when there is none. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $i i32)
    (local $byte i32)
    (local $face i32)
    (local $left i32)
    (block $write_failed
      (block $no_random
        (local.set $err (call $random_get (global.get $random) (global.get $random.size)))
        (br_if $no_random (local.get $err))
        (block $found
          (local.set $left (global.get $random.size))
          (loop $draw  ;; bound: 64
            (if (i32.ge_u (local.get $i) (global.get $random.size))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (local.set $byte (i32.load8_u (i32.add (global.get $random) (local.get $i))))
            (br_if $found (i32.lt_u (local.get $byte) (global.get $SET_ASIDE_FROM)))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $draw (local.get $left)))
          ;; Every byte drawn was set aside.
          (br $no_random))
        ;; The roll: the byte's remainder by FACES, plus 1.
        (local.set $face (i32.rem_u (local.get $byte) (global.get $FACES)))
        (if (i32.ge_u (local.get $face) (global.get $FACES))
          (then (call $assert_fail (i32.const 2)) unreachable))
        (memory.copy (global.get $line) (global.get $rolled) (global.get $rolled.rosize))
        (i32.store8 (i32.add (global.get $line) (global.get $ROLL_AT))
          (i32.add (global.get $FIRST_FACE) (local.get $face)))
        (i32.store8 (i32.add (global.get $line) (global.get $NEWLINE_AT)) (global.get $NEWLINE))
        (br_if $write_failed
          (call $write_all (global.get $STDOUT) (global.get $line) (global.get $line.size)))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_NO_RANDOM))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)
)
