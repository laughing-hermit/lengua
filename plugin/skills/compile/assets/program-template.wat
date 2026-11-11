;; computer: prints the name of the computer and the name of its user, which the programs
;; hostname and whoami give, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $write_all. The program starts
;; each of the two with lengua's program_run: its idea grants each by name, with no arguments, and
;; lengua starts nothing else. A command is the program's name and each argument, each ended by
;; a NUL; lengua gives back what the program printed, how many bytes that is, and its exit code.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (import "lengua_run_1" "program_run"
    (func $program_run (param i32 i32 i32 i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors: standard output.
  (global $STDOUT i32 (i32.const 1))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_IO_FAILED i32 (i32.const 4))
  ;; A started program gets no input.
  (global $NO_INPUT i32 (i32.const 0))
  ;; The fewest bytes a write takes.
  (global $ONE_BYTE i32 (i32.const 1))

  ;; Regions (rule 3, M1), from 1024 up: the two commands and the messages, read-only, then
  ;; what lengua writes.
  (global $hostname i32 (i32.const 1024))
  (global $hostname.rosize i32 (i32.const 9))
  (data (i32.const 1024) "hostname\00")
  (global $whoami i32 (i32.const 1040))
  (global $whoami.rosize i32 (i32.const 7))
  (data (i32.const 1040) "whoami\00")
  (global $hostname_absent i32 (i32.const 1056))
  (global $hostname_absent.rosize i32 (i32.const 20))
  (data (i32.const 1056) "hostname: not found\0a")
  (global $hostname_failed i32 (i32.const 1080))
  (global $hostname_failed.rosize i32 (i32.const 17))
  (data (i32.const 1080) "hostname: failed\0a")
  (global $whoami_absent i32 (i32.const 1104))
  (global $whoami_absent.rosize i32 (i32.const 18))
  (data (i32.const 1104) "whoami: not found\0a")
  (global $whoami_failed i32 (i32.const 1128))
  (global $whoami_failed.rosize i32 (i32.const 15))
  (data (i32.const 1128) "whoami: failed\0a")
  ;; The input a started program gets: none of these bytes.
  (global $nothing i32 (i32.const 1152))
  (global $nothing.size i32 (i32.const 4))
  ;; How many bytes a started program printed, and its exit code, as program_run writes them.
  (global $results i32 (i32.const 1160))
  (global $results.size i32 (i32.const 8))
  ;; What a started program printed.
  (global $said i32 (i32.const 2048))
  (global $said.size i32 (i32.const 256))

  ;; Starts hostname and then whoami, and prints what each printed, or that the computer does
  ;; not have it, or that it failed. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $size i32)
    (block $io_failed
      (block $hostname_done
        (block $hostname_is_absent
          (block $hostname_has_failed
            (local.set $err
              (call $program_run (global.get $hostname) (global.get $hostname.rosize)
                (global.get $nothing) (global.get $NO_INPUT)
                (global.get $said) (global.get $said.size)
                (global.get $results) (i32.add (global.get $results) (i32.const 4))))
            (br_if $hostname_is_absent (local.get $err))
            (br_if $hostname_has_failed (i32.load offset=4 (global.get $results)))
            (local.set $size (i32.load (global.get $results)))
            ;; lengua writes at most the room it is lent.
            (if (i32.gt_u (local.get $size) (global.get $said.size))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (br_if $hostname_done (i32.lt_u (local.get $size) (global.get $ONE_BYTE)))
            (br_if $io_failed
              (call $write_all (global.get $STDOUT) (global.get $said) (local.get $size)))
            (br $hostname_done))
          (br_if $io_failed
            (call $write_all (global.get $STDOUT) (global.get $hostname_failed)
              (global.get $hostname_failed.rosize)))
          (br $hostname_done))
        (br_if $io_failed
          (call $write_all (global.get $STDOUT) (global.get $hostname_absent)
            (global.get $hostname_absent.rosize))))
      (block $whoami_done
        (block $whoami_is_absent
          (block $whoami_has_failed
            (local.set $err
              (call $program_run (global.get $whoami) (global.get $whoami.rosize)
                (global.get $nothing) (global.get $NO_INPUT)
                (global.get $said) (global.get $said.size)
                (global.get $results) (i32.add (global.get $results) (i32.const 4))))
            (br_if $whoami_is_absent (local.get $err))
            (br_if $whoami_has_failed (i32.load offset=4 (global.get $results)))
            (local.set $size (i32.load (global.get $results)))
            ;; lengua writes at most the room it is lent.
            (if (i32.gt_u (local.get $size) (global.get $said.size))
              (then (call $assert_fail (i32.const 2)) unreachable))
            (br_if $whoami_done (i32.lt_u (local.get $size) (global.get $ONE_BYTE)))
            (br_if $io_failed
              (call $write_all (global.get $STDOUT) (global.get $said) (local.get $size)))
            (br $whoami_done))
          (br_if $io_failed
            (call $write_all (global.get $STDOUT) (global.get $whoami_failed)
              (global.get $whoami_failed.rosize)))
          (br $whoami_done))
        (br_if $io_failed
          (call $write_all (global.get $STDOUT) (global.get $whoami_absent)
            (global.get $whoami_absent.rosize))))
      (call $proc_exit (global.get $EXIT_OK))
      unreachable)
    (call $proc_exit (global.get $EXIT_IO_FAILED))
    unreachable)
)
