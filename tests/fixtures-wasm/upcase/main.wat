;; upcase: copies a text file to another file with its ASCII letters in capitals. The WebAssembly
;; twin of examples/upcase, in the WebAssembly dialect 1 (references/power-of-10-webassembly.md):
;; lengua's library gives $fill and $write_all. It copies each file's name from its argument to a
;; region of its own, so each path starts where its region does; it reads the whole input file
;; with $fill, capitalizes it a byte at a time, as stream-template.wat does, where the native twin
;; takes 16 bytes at a time with SSE2, and writes it to the output file, which it makes or empties
;; first.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; 18 pages: the small regions in the first, the input file in the next 17.
  (memory (export "memory") 18 18)

  ;; Descriptors: standard error, and the working folder, which lengua opens as 3.
  (global $STDERR i32 (i32.const 2))
  (global $FOLDER i32 (i32.const 3))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_CANNOT_OPEN i32 (i32.const 2))
  (global $EXIT_TOO_LARGE i32 (i32.const 3))
  (global $EXIT_IO_FAILED i32 (i32.const 4))
  ;; path_open's flags: none, or make the file and empty it; and its rights: none, to read, or to
  ;; write.
  (global $NO_FLAGS i32 (i32.const 0))
  (global $MAKE_AND_EMPTY i32 (i32.const 9))
  (global $NO_RIGHTS i64 (i64.const 0))
  (global $READ_RIGHTS i64 (i64.const 2))
  (global $WRITE_RIGHTS i64 (i64.const 64))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; Sizes and limits: a name of at most NAME_MAX bytes, the longest path a program may give, and
  ;; the largest input file.
  (global $ARGC_EXPECTED i32 (i32.const 3))
  (global $NAME_MAX i32 (i32.const 1023))
  (global $NAME_SCAN i32 (i32.const 1024))
  (global $MAX_INPUT i32 (i32.const 1048576))
  ;; Characters: a to z are the 26 bytes from 97, and each capital is 32 below its letter.
  (global $SMALL_A i32 (i32.const 97))
  (global $LETTERS i32 (i32.const 26))
  (global $CASE_BIT i32 (i32.const 32))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then what the host writes, the
  ;; files' names, the arguments' text and the input file.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 31))
  (data (i32.const 1024) "usage: upcase <input> <output>\0a")
  (global $open_failed i32 (i32.const 1056))
  (global $open_failed.rosize i32 (i32.const 20))
  (data (i32.const 1056) "upcase: cannot open ")
  (global $newline i32 (i32.const 1080))
  (global $newline.rosize i32 (i32.const 1))
  (data (i32.const 1080) "\0a")
  (global $large_prefix i32 (i32.const 1088))
  (global $large_prefix.rosize i32 (i32.const 8))
  (data (i32.const 1088) "upcase: ")
  (global $large_suffix i32 (i32.const 1096))
  (global $large_suffix.rosize i32 (i32.const 30))
  (data (i32.const 1096) " is larger than 1048576 bytes\0a")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 1128))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then the two files'.
  (global $argv i32 (i32.const 1136))
  (global $argv.size i32 (i32.const 12))
  ;; The descriptor path_open gives a file.
  (global $opened i32 (i32.const 1152))  ;; shared: open has the host write it; the others read it
  (global $opened.size i32 (i32.const 4))
  ;; Each file's name: NAME_MAX bytes, and one more, which shows a longer name.
  (global $input_name i32 (i32.const 1216))  ;; shared: main opens it; say_input writes it out
  (global $input_name.size i32 (i32.const 1024))
  (global $output_name i32 (i32.const 2240))  ;; shared: main copies it; save_output opens it
  (global $output_name.size i32 (i32.const 1024))
  ;; The arguments' text, each ended by a NUL: 32 KiB, more than a test's command line takes.
  (global $args i32 (i32.const 3264))  ;; shared: main has the host fill it; copy_argument reads it
  (global $args.size i32 (i32.const 32768))
  ;; The input file: MAX_INPUT bytes, and one more, which shows a larger file.
  (global $buffer i32 (i32.const 65536))  ;; shared: load_input fills it; the others use it
  (global $buffer.size i32 (i32.const 1048577))

  ;; Reads the input file, capitalizes it and writes the output file, or says why it can't.
  ;; Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $in_len i32)
    (local $out_len i32)
    (local $got i32)
    (block $io_failed
      (block $too_large
        (block $cannot_open
          (block $usage
            (local.set $err
              (call $args_sizes_get (global.get $sizes)
                (i32.add (global.get $sizes) (i32.const 4))))
            (if (local.get $err)
              (then (call $assert_fail (i32.const 1)) unreachable))
            (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
            ;; Text longer than $args holds is longer than two names.
            (br_if $usage
              (i32.gt_u (i32.load offset=4 (global.get $sizes)) (global.get $args.size)))
            (local.set $err (call $args_get (global.get $argv) (global.get $args)))
            (if (local.get $err)
              (then (call $assert_fail (i32.const 2)) unreachable))
            ;; Each name starts where its argv entry points: an offset into $args, which
            ;; copy_argument checks.
            (local.set $in_len
              (call $copy_argument
                (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args))
                (global.get $input_name)))
            (br_if $usage (i32.gt_u (local.get $in_len) (global.get $NAME_MAX)))
            (local.set $out_len
              (call $copy_argument
                (i32.sub (i32.load offset=8 (global.get $argv)) (global.get $args))
                (global.get $output_name)))
            (br_if $usage (i32.gt_u (local.get $out_len) (global.get $NAME_MAX)))
            (local.set $err
              (call $open (global.get $input_name) (local.get $in_len) (global.get $NO_FLAGS)
                (global.get $READ_RIGHTS)))
            (br_if $cannot_open (local.get $err))
            (local.set $got (call $load_input))
            (br_if $io_failed (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $buffer.size))
              (then (call $assert_fail (i32.const 3)) unreachable))
            (br_if $too_large (i32.gt_u (local.get $got) (global.get $MAX_INPUT)))
            (call $shout (global.get $buffer) (local.get $got))
            (br_if $io_failed (call $save_output (local.get $out_len) (local.get $got)))
            (call $proc_exit (global.get $EXIT_OK))
            unreachable)
          (br_if $io_failed
            (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
          (call $proc_exit (global.get $EXIT_USAGE))
          unreachable)
        (br_if $io_failed
          (call $write_all (global.get $STDERR) (global.get $open_failed)
            (global.get $open_failed.rosize)))
        (br_if $io_failed (call $say_input (local.get $in_len)))
        (br_if $io_failed
          (call $write_all (global.get $STDERR) (global.get $newline) (global.get $newline.rosize)))
        (call $proc_exit (global.get $EXIT_CANNOT_OPEN))
        unreachable)
      (br_if $io_failed
        (call $write_all (global.get $STDERR) (global.get $large_prefix)
          (global.get $large_prefix.rosize)))
      (br_if $io_failed (call $say_input (local.get $in_len)))
      (br_if $io_failed
        (call $write_all (global.get $STDERR) (global.get $large_suffix)
          (global.get $large_suffix.rosize)))
      (call $proc_exit (global.get $EXIT_TOO_LARGE))
      unreachable)
    (call $proc_exit (global.get $EXIT_IO_FAILED))
    unreachable)

  ;; Copies the argument that starts at byte $at of $args, up to its NUL, to $into, which main
  ;; lends it with room for NAME_SCAN bytes, and gives its length; or NAME_SCAN, when it is longer
  ;; than a path can be.
  (func $copy_argument (param $at i32) (param $into i32) (result i32)
    (local $k i32)
    (local $from i32)
    (local $c i32)
    (local $left i32)
    (block $ended
      (local.set $left (global.get $NAME_SCAN))
      (loop $byte  ;; bound: 1024
        (local.set $from (i32.add (local.get $at) (local.get $k)))
        (if (i32.ge_u (local.get $from) (global.get $args.size))
          (then (call $assert_fail (i32.const 4)) unreachable))
        (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $from))))
        (br_if $ended (i32.eqz (local.get $c)))
        (if (i32.ge_u (local.get $k) (global.get $NAME_SCAN))
          (then (call $assert_fail (i32.const 5)) unreachable))
        (i32.store8 (i32.add (local.get $into) (local.get $k)) (local.get $c))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $k))

  ;; Opens the file that the $length bytes at $name name, which its caller lends it, with
  ;; path_open's flags $how and rights $rights: its descriptor goes in $opened. Gives 0, or what
  ;; path_open gives.
  (func $open  ;; returns: result
    (param $name i32) (param $length i32) (param $how i32) (param $rights i64) (result i32)
    (local $err i32)
    (if (i32.gt_u (local.get $length) (global.get $NAME_MAX))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (if (i32.gt_u (local.get $how) (global.get $MAKE_AND_EMPTY))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (block $done
      (local.set $err
        (call $path_open (global.get $FOLDER) (global.get $NO_FLAGS) (local.get $name)
          (local.get $length) (local.get $how) (local.get $rights) (global.get $NO_RIGHTS)
          (global.get $NO_FLAGS) (global.get $opened)))
      (br_if $done (local.get $err))
      ;; A file's descriptor comes after the working folder's.
      (if (i32.le_u (i32.load (global.get $opened)) (global.get $FOLDER))
        (then (call $assert_fail (i32.const 8)) unreachable)))
    (local.get $err))

  ;; Reads all of the file whose descriptor is in $opened into $buffer, as far as the buffer's
  ;; size, and closes it. Gives how many bytes it read, or the negative of an error number.
  (func $load_input (result i32)
    (local $fd i32)
    (local $got i32)
    (local $err i32)
    (local.set $fd (i32.load (global.get $opened)))
    (if (i32.le_u (local.get $fd) (global.get $FOLDER))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (block $done
      (local.set $got
        (call $fill (local.get $fd) (global.get $buffer) (global.get $buffer.size)
          (global.get $READ_TO_END)))
      (br_if $done (i32.lt_s (local.get $got) (i32.const 0)))
      (if (i32.gt_u (local.get $got) (global.get $buffer.size))
        (then (call $assert_fail (i32.const 10)) unreachable))
      (local.set $err (call $fd_close (local.get $fd)))
      (br_if $done (i32.eqz (local.get $err)))
      (local.set $got (i32.sub (i32.const 0) (local.get $err))))
    (local.get $got))

  ;; Makes or empties the file that the $length bytes of $output_name name, writes the first
  ;; $count bytes of $buffer to it, and closes it. Gives 0, or the error of the call that failed.
  (func $save_output (param $length i32) (param $count i32) (result i32)  ;; returns: result
    (local $fd i32)
    (local $err i32)
    (if (i32.gt_u (local.get $length) (global.get $NAME_MAX))
      (then (call $assert_fail (i32.const 11)) unreachable))
    (if (i32.gt_u (local.get $count) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 12)) unreachable))
    (block $done
      (local.set $err
        (call $open (global.get $output_name) (local.get $length) (global.get $MAKE_AND_EMPTY)
          (global.get $WRITE_RIGHTS)))
      (br_if $done (local.get $err))
      (local.set $fd (i32.load (global.get $opened)))
      (local.set $err (call $write_all (local.get $fd) (global.get $buffer) (local.get $count)))
      (br_if $done (local.get $err))
      (local.set $err (call $fd_close (local.get $fd)))
      (br_if $done (local.get $err)))
    (local.get $err))

  ;; Capitalizes the $len bytes at $at, which main lends it: a to z become A to Z, and the other
  ;; bytes stay as they are, those that aren't ASCII too. Each byte's change is arithmetic, not a
  ;; branch.
  (func $shout (param $at i32) (param $len i32)
    (local $i i32)
    (local $c i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 13)) unreachable))
    (block $done
      (local.set $left (global.get $MAX_INPUT))
      (loop $byte  ;; bound: 1048576
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (if (i32.ge_u (local.get $i) (global.get $MAX_INPUT))
          (then (call $assert_fail (i32.const 14)) unreachable))
        (local.set $c (i32.load8_u (i32.add (local.get $at) (local.get $i))))
        ;; Below `a` wraps to a large number, so one unsigned test finds a to z, and the test's 1
        ;; times the case bit is what comes off.
        (i32.store8 (i32.add (local.get $at) (local.get $i))
          (i32.sub (local.get $c)
            (i32.mul (global.get $CASE_BIT)
              (i32.lt_u (i32.sub (local.get $c) (global.get $SMALL_A)) (global.get $LETTERS)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left)))))

  ;; Writes the $length bytes of $input_name to standard error. Gives 0, or an error number.
  (func $say_input (param $length i32) (result i32)  ;; returns: result
    (if (i32.gt_u (local.get $length) (global.get $NAME_MAX))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (call $write_all (global.get $STDERR) (global.get $input_name) (local.get $length)))
)
