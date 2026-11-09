;; total: reads the CSV file of expenses that its argument names and prints the total amount with
;; two decimals. The WebAssembly twin of tests/fixtures/total, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $fill and $write_all. It copies
;; the file's name from its argument to a region of its own, opens the file in the working folder,
;; reads all of it with $fill, and adds each line's amount exactly, in whole cents: a line ends at
;; its newline, its date at its first comma, and its amount is what lies between the two.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; 17 pages: the small regions in the first, the file in the other 16.
  (memory (export "memory") 17 17)

  ;; Descriptors: standard output and error, and the working folder, which lengua opens as 3.
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  (global $FOLDER i32 (i32.const 3))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_CANNOT_OPEN i32 (i32.const 2))
  (global $EXIT_BAD_LINE i32 (i32.const 3))
  (global $EXIT_READ_FAILED i32 (i32.const 4))
  (global $EXIT_WRITE_FAILED i32 (i32.const 5))
  ;; What the functions marked "returns: result" give, as references/host.md names it.
  (global $ERROR_NAMETOOLONG i32 (i32.const 37))
  ;; path_open's flags and rights: none, and the right to read.
  (global $NO_FLAGS i32 (i32.const 0))
  (global $NO_RIGHTS i64 (i64.const 0))
  (global $READ_RIGHTS i64 (i64.const 2))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; Sizes and limits: a name of at most NAME_MAX bytes, the longest path a program may give,
  ;; and the lines a file of FILE_MAX bytes holds at most, one more than its bytes.
  (global $ARGC_EXPECTED i32 (i32.const 2))
  (global $NAME_MAX i32 (i32.const 1023))
  (global $NAME_SCAN i32 (i32.const 1024))
  (global $FILE_MAX i32 (i32.const 1048576))
  (global $MAX_LINES i32 (i32.const 1048577))
  ;; An amount: 1 to 5 digits, a point and 2 more, so 4 to 8 bytes, its point 3 from its end.
  (global $AMOUNT_MIN i32 (i32.const 4))
  (global $AMOUNT_MAX i32 (i32.const 8))
  (global $POINT_FROM_END i32 (i32.const 3))
  (global $NOT_AN_AMOUNT i64 (i64.const -1))
  ;; Printing: the places of cents and of a line's number, and a number's digits at most.
  (global $CENT_PLACES i32 (i32.const 2))
  (global $NO_PLACES i32 (i32.const 0))
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $NEWLINE_AT i32 (i32.const 22))
  (global $TEN i64 (i64.const 10))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))
  (global $COMMA i32 (i32.const 44))
  (global $POINT i32 (i32.const 46))
  (global $ZERO i32 (i32.const 48))
  (global $NINE i32 (i32.const 9))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then what the host writes, the
  ;; line it prints, the file's name, the arguments' text and the file.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 24))
  (data (i32.const 1024) "usage: total <file.csv>\0a")
  (global $open_failed i32 (i32.const 1056))
  (global $open_failed.rosize i32 (i32.const 24))
  (data (i32.const 1056) "total: cannot open file\0a")
  (global $bad i32 (i32.const 1088))
  (global $bad.rosize i32 (i32.const 16))
  (data (i32.const 1088) "total: bad line ")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 1104))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then the file's.
  (global $argv i32 (i32.const 1112))
  (global $argv.size i32 (i32.const 8))
  ;; The descriptor path_open gives the file.
  (global $opened i32 (i32.const 1120))  ;; shared: open_file has the host write it; main reads it
  (global $opened.size i32 (i32.const 4))
  ;; The line it prints: up to 20 digits, a point and a newline.
  (global $out i32 (i32.const 1136))
  (global $out.size i32 (i32.const 23))
  ;; The file's name: NAME_MAX bytes, and one more, which shows a longer name.
  (global $name i32 (i32.const 1216))  ;; shared: copy_name writes it; open_file opens it
  (global $name.size i32 (i32.const 1024))
  ;; The arguments' text, each ended by a NUL: 32 KiB, more than a test's command line takes.
  (global $args i32 (i32.const 2240))  ;; shared: main has the host fill it; copy_name reads it
  (global $args.size i32 (i32.const 32768))
  ;; The whole file, FILE_MAX bytes.
  (global $file i32 (i32.const 65536))  ;; shared: main fills it; find and amount_of read it
  (global $file.size i32 (i32.const 1048576))

  ;; Opens the file that the argument names, reads it, adds its amounts and prints the total, or
  ;; says why it can't. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $fd i32)
    (local $got i32)
    (local $total i64)
    (block $write_failed
      (block $read_failed
        (block $bad_line
          (block $cannot_open
            (block $usage
              (local.set $err
                (call $args_sizes_get (global.get $sizes)
                  (i32.add (global.get $sizes) (i32.const 4))))
              (if (local.get $err)
                (then (call $assert_fail (i32.const 1)) unreachable))
              (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
              ;; Text longer than $args holds is a name longer than a path can be.
              (br_if $cannot_open
                (i32.gt_u (i32.load offset=4 (global.get $sizes)) (global.get $args.size)))
              (local.set $err (call $args_get (global.get $argv) (global.get $args)))
              (if (local.get $err)
                (then (call $assert_fail (i32.const 2)) unreachable))
              ;; The name starts where argv[1] points: an offset into $args, which copy_name
              ;; checks.
              (local.set $err
                (call $open_file
                  (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args))))
              (br_if $cannot_open (local.get $err))
              (local.set $fd (i32.load (global.get $opened)))
              (local.set $got
                (call $fill (local.get $fd) (global.get $file) (global.get $file.size)
                  (global.get $READ_TO_END)))
              (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
              (if (i32.gt_u (local.get $got) (global.get $file.size))
                (then (call $assert_fail (i32.const 3)) unreachable))
              (local.set $total (call $sum_lines (local.get $got)))
              (br_if $bad_line (i64.lt_s (local.get $total) (i64.const 0)))
              (br_if $write_failed
                (call $write_decimal (global.get $STDOUT) (local.get $total)
                  (global.get $CENT_PLACES)))
              (call $proc_exit (global.get $EXIT_OK))
              unreachable)
            (br_if $write_failed
              (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
            (call $proc_exit (global.get $EXIT_USAGE))
            unreachable)
          (br_if $write_failed
            (call $write_all (global.get $STDERR) (global.get $open_failed)
              (global.get $open_failed.rosize)))
          (call $proc_exit (global.get $EXIT_CANNOT_OPEN))
          unreachable)
        ;; A bad line: the total's negative is its number.
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $bad) (global.get $bad.rosize)))
        (br_if $write_failed
          (call $write_decimal (global.get $STDERR) (i64.sub (i64.const 0) (local.get $total))
            (global.get $NO_PLACES)))
        (call $proc_exit (global.get $EXIT_BAD_LINE))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Opens, to read, the file that the argument at byte $at of $args names: its descriptor goes
  ;; in $opened. Gives 0, or why it can't: NAMETOOLONG for a name longer than a path can be, or
  ;; what path_open gives.
  (func $open_file (param $at i32) (result i32)  ;; returns: result
    (local $len i32)
    (local $err i32)
    (local.set $len (call $copy_name (local.get $at)))
    (if (i32.gt_u (local.get $len) (global.get $NAME_SCAN))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (local.set $err (global.get $ERROR_NAMETOOLONG))
    (block $done
      (br_if $done (i32.gt_u (local.get $len) (global.get $NAME_MAX)))
      (local.set $err
        (call $path_open (global.get $FOLDER) (global.get $NO_FLAGS) (global.get $name)
          (local.get $len) (global.get $NO_FLAGS) (global.get $READ_RIGHTS)
          (global.get $NO_RIGHTS) (global.get $NO_FLAGS) (global.get $opened)))
      (br_if $done (local.get $err)))
    (local.get $err))

  ;; Copies the argument that starts at byte $at of $args, up to its NUL, to $name, and gives its
  ;; length; or NAME_SCAN, when it is longer than a path can be.
  (func $copy_name (param $at i32) (result i32)
    (local $k i32)
    (local $from i32)
    (local $c i32)
    (local $left i32)
    (block $ended
      (local.set $left (global.get $NAME_SCAN))
      (loop $byte  ;; bound: 1024
        (local.set $from (i32.add (local.get $at) (local.get $k)))
        (if (i32.ge_u (local.get $from) (global.get $args.size))
          (then (call $assert_fail (i32.const 5)) unreachable))
        (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $from))))
        (br_if $ended (i32.eqz (local.get $c)))
        (if (i32.ge_u (local.get $k) (global.get $name.size))
          (then (call $assert_fail (i32.const 6)) unreachable))
        (i32.store8 (i32.add (global.get $name) (local.get $k)) (local.get $c))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $k))

  ;; Adds the amounts of the lines in the file's first $len bytes. Gives the total in cents; or,
  ;; for the first line that isn't a date, a comma and an amount, the negative of its number.
  (func $sum_lines (param $len i32) (result i64)
    (local $at i32)
    (local $end i32)
    (local $comma i32)
    (local $cents i64)
    (local $total i64)
    (local $line i64)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $FILE_MAX))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (block $done
      (block $bad
        (local.set $left (global.get $MAX_LINES))
        (loop $next  ;; bound: 1048577
          (br_if $done (i32.ge_u (local.get $at) (local.get $len)))
          (local.set $line (i64.add (local.get $line) (i64.const 1)))
          ;; The line ends at its newline, or where the file does.
          (local.set $end (call $find (local.get $at) (local.get $len) (global.get $NEWLINE)))
          ;; The date is any text up to the line's first comma.
          (local.set $comma (call $find (local.get $at) (local.get $end) (global.get $COMMA)))
          (br_if $bad (i32.ge_u (local.get $comma) (local.get $end)))
          (local.set $cents
            (call $amount_of (i32.add (local.get $comma) (i32.const 1)) (local.get $end)))
          (br_if $bad (i64.lt_s (local.get $cents) (i64.const 0)))
          (local.set $total (i64.add (local.get $total) (local.get $cents)))
          (local.set $at (i32.add (local.get $end) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $next (local.get $left)))
        ;; Each line takes a byte at least, so the bytes run out first.
        (call $assert_fail (i32.const 8))
        unreachable)
      ;; The bad line's number, as a negative.
      (local.set $total (i64.sub (i64.const 0) (local.get $line))))
    (local.get $total))

  ;; Gives the place of the first byte $byte among the file's bytes from $from to $to, or $to when
  ;; none of them is.
  (func $find (param $from i32) (param $to i32) (param $byte i32) (result i32)
    (local $i i32)
    (local $found i32)
    (local $left i32)
    (if (i32.gt_u (local.get $to) (global.get $FILE_MAX))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (local.set $found (local.get $to))
    (local.set $i (local.get $from))
    (block $done
      (local.set $left (global.get $FILE_MAX))
      (loop $scan  ;; bound: 1048576
        (br_if $done (i32.ge_u (local.get $i) (local.get $to)))
        (if (i32.ge_u (local.get $i) (global.get $file.size))
          (then (call $assert_fail (i32.const 10)) unreachable))
        (if (i32.eq (i32.load8_u (i32.add (global.get $file) (local.get $i))) (local.get $byte))
          (then
            (local.set $found (local.get $i))
            (br $done)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $scan (local.get $left))))
    (local.get $found))

  ;; Gives the amount in cents that the file's bytes from $from to $to write: 1 to 5 digits, a
  ;; point and 2 digits. Gives NOT_AN_AMOUNT when they write none.
  (func $amount_of (param $from i32) (param $to i32) (result i64)
    (local $i i32)
    (local $point i32)
    (local $c i32)
    (local $cents i64)
    (local $amount i64)
    (local $left i32)
    (if (i32.gt_u (local.get $to) (global.get $FILE_MAX))
      (then (call $assert_fail (i32.const 11)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $to))
      (then (call $assert_fail (i32.const 12)) unreachable))
    (local.set $amount (global.get $NOT_AN_AMOUNT))
    (block $bad
      (br_if $bad
        (i32.lt_u (i32.sub (local.get $to) (local.get $from)) (global.get $AMOUNT_MIN)))
      (br_if $bad
        (i32.gt_u (i32.sub (local.get $to) (local.get $from)) (global.get $AMOUNT_MAX)))
      (local.set $point (i32.sub (local.get $to) (global.get $POINT_FROM_END)))
      (local.set $i (local.get $from))
      (block $parsed
        (local.set $left (global.get $AMOUNT_MAX))
        (loop $char  ;; bound: 8
          (br_if $parsed (i32.ge_u (local.get $i) (local.get $to)))
          (if (i32.ge_u (local.get $i) (global.get $file.size))
            (then (call $assert_fail (i32.const 13)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $file) (local.get $i))))
          (if (i32.eq (local.get $i) (local.get $point))
            (then (br_if $bad (i32.ne (local.get $c) (global.get $POINT))))
            (else
              ;; Below `0` wraps to a large number, so one unsigned test finds a digit.
              (local.set $c (i32.sub (local.get $c) (global.get $ZERO)))
              (br_if $bad (i32.gt_u (local.get $c) (global.get $NINE)))
              (local.set $cents
                (i64.add (i64.mul (local.get $cents) (global.get $TEN))
                  (i64.extend_i32_u (local.get $c))))))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $char (local.get $left))))
      (local.set $amount (local.get $cents)))
    (local.get $amount))

  ;; Writes $units, a whole number of units, in decimal with $places places, 0 or 2, then a
  ;; newline, to $fd: the newline last, the digits before it from the last, and the point before
  ;; the last $places of them. Gives 0, or an error number.
  (func $write_decimal  ;; returns: result
    (param $fd i32) (param $units i64) (param $places i32) (result i32)
    (local $rest i64)
    (local $at i32)
    (local $digits i32)
    (local $left i32)
    (if (i32.gt_u (local.get $places) (global.get $CENT_PLACES))
      (then (call $assert_fail (i32.const 14)) unreachable))
    (local.set $rest (local.get $units))
    (local.set $at (global.get $NEWLINE_AT))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 15)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (local.set $digits (i32.add (local.get $digits) (i32.const 1)))
        ;; The point, before the last $places digits.
        (if (i32.eq (local.get $digits) (local.get $places))
          (then
            (local.set $at (i32.sub (local.get $at) (i32.const 1)))
            (if (i32.ge_u (local.get $at) (global.get $out.size))
              (then (call $assert_fail (i32.const 16)) unreachable))
            (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $POINT))))
        ;; Every digit is out, and a digit at least before the point.
        (br_if $written
          (i32.and (i64.eqz (local.get $rest))
            (i32.gt_u (local.get $digits) (local.get $places))))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; A 64-bit number has 20 digits at most, and the point comes after 2 of them.
      (call $assert_fail (i32.const 17))
      unreachable)
    (call $write_all (local.get $fd) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
