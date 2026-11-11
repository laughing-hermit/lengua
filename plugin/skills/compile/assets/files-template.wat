;; copy: copies the file its first argument names to the file its second argument names, in the
;; WebAssembly dialect 1 (references/power-of-10-webassembly.md): lengua's library gives $fill and
;; $write_all. The program never sees a path. Its idea grants the file each argument names, and
;; it opens each with lengua's file_open, by the argument's place: lengua reads the argument,
;; which the user typed, and opens what it names, the first to read and the second to write.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (import "lengua_fs_1" "file_open"
    (func $file_open (param i32 i32 i32) (result i32)))
  ;; 3 pages: the small regions in the first, the input file in the next two.
  (memory (export "memory") 3 3)

  ;; Descriptors: standard error.
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_CANNOT_OPEN i32 (i32.const 2))
  (global $EXIT_TOO_LARGE i32 (i32.const 3))
  (global $EXIT_IO_FAILED i32 (i32.const 4))
  ;; The arguments by their place, which is all file_open takes of them.
  (global $INPUT i32 (i32.const 1))
  (global $OUTPUT i32 (i32.const 2))
  ;; file_open's modes: to read; and to write, make the file when it isn't there, and empty it
  ;; (2 + 4 + 8).
  (global $READ i32 (i32.const 1))
  (global $MAKE_AND_EMPTY i32 (i32.const 14))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The program's name and its two arguments.
  (global $ARGC_EXPECTED i32 (i32.const 3))
  ;; The largest input file, and the fewest bytes a write takes.
  (global $MAX_INPUT i32 (i32.const 65536))
  (global $ONE_BYTE i32 (i32.const 1))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then what the host writes, and
  ;; the input file.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 29))
  (data (i32.const 1024) "usage: copy <input> <output>\0a")
  (global $no_input i32 (i32.const 1056))
  (global $no_input.rosize i32 (i32.const 33))
  (data (i32.const 1056) "copy: cannot open the input file\0a")
  (global $no_output i32 (i32.const 1096))
  (global $no_output.rosize i32 (i32.const 34))
  (data (i32.const 1096) "copy: cannot open the output file\0a")
  (global $too_large i32 (i32.const 1136))
  (global $too_large.rosize i32 (i32.const 48))
  (data (i32.const 1136) "copy: the input file is larger than 65536 bytes\0a")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 1192))
  (global $sizes.size i32 (i32.const 8))
  ;; The descriptor file_open gives a file.
  (global $opened i32 (i32.const 1200))
  (global $opened.size i32 (i32.const 4))
  ;; The input file: MAX_INPUT bytes, and one more, which shows a larger file.
  (global $buffer i32 (i32.const 65536))
  (global $buffer.size i32 (i32.const 65537))

  ;; Reads the input file and writes it to the output file, or says why it can't. Never
  ;; returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $in i32)
    (local $out i32)
    (local $got i32)
    (block $io_failed
      (block $input_too_large
        (block $output_unopened
          (block $input_unopened
            (block $usage_shown
              (local.set $err
                (call $args_sizes_get (global.get $sizes)
                  (i32.add (global.get $sizes) (i32.const 4))))
              (if (local.get $err)
                (then (call $assert_fail (i32.const 1)) unreachable))
              (br_if $usage_shown
                (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
              ;; The input file: the one the first argument names, to read.
              (local.set $err
                (call $file_open (global.get $INPUT) (global.get $READ) (global.get $opened)))
              (br_if $input_unopened (local.get $err))
              (local.set $in (i32.load (global.get $opened)))
              (local.set $got
                (call $fill (local.get $in) (global.get $buffer) (global.get $buffer.size)
                  (global.get $READ_TO_END)))
              (br_if $io_failed (i32.lt_s (local.get $got) (i32.const 0)))
              ;; $fill reads at most the room it is given.
              (if (i32.gt_u (local.get $got) (global.get $buffer.size))
                (then (call $assert_fail (i32.const 2)) unreachable))
              (local.set $err (call $fd_close (local.get $in)))
              (br_if $io_failed (local.get $err))
              (br_if $input_too_large (i32.gt_u (local.get $got) (global.get $MAX_INPUT)))
              ;; The output file: the one the second argument names, made or emptied, to write.
              (local.set $err
                (call $file_open (global.get $OUTPUT) (global.get $MAKE_AND_EMPTY)
                  (global.get $opened)))
              (br_if $output_unopened (local.get $err))
              (local.set $out (i32.load (global.get $opened)))
              ;; An empty input leaves an empty output.
              (if (local.get $got)
                (then
                  ;; A write here takes at least one byte.
                  (if (i32.lt_u (local.get $got) (global.get $ONE_BYTE))
                    (then (call $assert_fail (i32.const 3)) unreachable))
                  (br_if $io_failed
                    (call $write_all (local.get $out) (global.get $buffer) (local.get $got)))))
              (local.set $err (call $fd_close (local.get $out)))
              (br_if $io_failed (local.get $err))
              (call $proc_exit (global.get $EXIT_OK))
              unreachable)
            (br_if $io_failed
              (call $write_all (global.get $STDERR) (global.get $usage)
                (global.get $usage.rosize)))
            (call $proc_exit (global.get $EXIT_USAGE))
            unreachable)
          (br_if $io_failed
            (call $write_all (global.get $STDERR) (global.get $no_input)
              (global.get $no_input.rosize)))
          (call $proc_exit (global.get $EXIT_CANNOT_OPEN))
          unreachable)
        (br_if $io_failed
          (call $write_all (global.get $STDERR) (global.get $no_output)
            (global.get $no_output.rosize)))
        (call $proc_exit (global.get $EXIT_CANNOT_OPEN))
        unreachable)
      (br_if $io_failed
        (call $write_all (global.get $STDERR) (global.get $too_large)
          (global.get $too_large.rosize)))
      (call $proc_exit (global.get $EXIT_TOO_LARGE))
      unreachable)
    (call $proc_exit (global.get $EXIT_IO_FAILED))
    unreachable)
)
