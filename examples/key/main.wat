;; key: waits until a person presses a key, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $fill and $write_all. Its idea
;; uses the terminal, so it may call lengua's term_raw: with RAW, standard input gives each key
;; as it is pressed, with no echo and no Enter; with AS_BEFORE, the terminal is as it was. Where
;; standard input is no terminal, as in a test, term_raw answers NOTTY and the program reads the
;; first byte of its input all the same. lengua puts the terminal back when the program ends,
;; however it ends; the program puts it back itself as soon as it has its key.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (import "lengua_term_1" "term_raw" (func $term_raw (param i32) (result i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_NO_KEY i32 (i32.const 1))
  (global $EXIT_IO_FAILED i32 (i32.const 4))
  ;; term_raw's modes.
  (global $AS_BEFORE i32 (i32.const 0))
  (global $RAW i32 (i32.const 1))
  ;; What $is_raw holds once the terminal gives keys as they're pressed.
  (global $YES i32 (i32.const 1))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then the key.
  (global $ask i32 (i32.const 1024))
  (global $ask.rosize i32 (i32.const 13))
  (data (i32.const 1024) "Press a key.\0a")
  (global $thanks i32 (i32.const 1040))
  (global $thanks.rosize i32 (i32.const 19))
  (data (i32.const 1040) "You pressed a key.\0a")
  (global $none i32 (i32.const 1064))
  (global $none.rosize i32 (i32.const 17))
  (data (i32.const 1064) "key: no key came\0a")
  ;; The key: one byte.
  (global $key i32 (i32.const 1088))
  (global $key.size i32 (i32.const 1))

  ;; Asks for a key, reads one, and says so, or says that none came. Never returns: every path
  ;; ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $got i32)
    (local $is_raw i32)
    (block $io_failed
      (block $no_key
        (br_if $io_failed
          (call $write_all (global.get $STDOUT) (global.get $ask) (global.get $ask.rosize)))
        ;; Keys as they're pressed, where standard input is a terminal; a line, or a pipe's
        ;; bytes, where it isn't.
        (block $as_it_is
          (local.set $err (call $term_raw (global.get $RAW)))
          (br_if $as_it_is (local.get $err))
          (local.set $is_raw (global.get $YES)))
        (local.set $got
          (call $fill (global.get $STDIN) (global.get $key) (global.get $key.size)
            (global.get $READ_TO_END)))
        (br_if $io_failed (i32.lt_s (local.get $got) (i32.const 0)))
        ;; $fill reads at most the room it is given.
        (if (i32.gt_u (local.get $got) (global.get $key.size))
          (then (call $assert_fail (i32.const 1)) unreachable))
        ;; The terminal is as it was before anything more is printed.
        (if (local.get $is_raw)
          (then
            (local.set $err (call $term_raw (global.get $AS_BEFORE)))
            (br_if $io_failed (local.get $err))))
        (br_if $no_key (i32.eqz (local.get $got)))
        ;; One key is one byte here.
        (if (i32.ne (local.get $got) (global.get $key.size))
          (then (call $assert_fail (i32.const 2)) unreachable))
        (br_if $io_failed
          (call $write_all (global.get $STDOUT) (global.get $thanks)
            (global.get $thanks.rosize)))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (br_if $io_failed
        (call $write_all (global.get $STDERR) (global.get $none) (global.get $none.rosize)))
      (call $proc_exit (global.get $EXIT_NO_KEY))
      unreachable)
    (call $proc_exit (global.get $EXIT_IO_FAILED))
    unreachable)
)
