;; total: reads a CSV from standard input and prints the total of its amount column. The
;; WebAssembly port of the native amounts, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): the whole input read with $fill, the header's
;; `amount` column found, and each row's field in that column read as exact cents and added up;
;; a row at fault is reported with its line. Every byte is read through $byte_at, which checks
;; its place and gives END past the input, so no scan reads past what came.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; 17 pages, 1114112 bytes: lengua's first 1024 bytes and the regions below, which end at
  ;; 1049857.
  (memory (export "memory") 17 17)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_NO_COLUMN i32 (i32.const 1))
  (global $EXIT_BAD_AMOUNT i32 (i32.const 2))
  (global $EXIT_MISSING_AMOUNT i32 (i32.const 3))
  (global $EXIT_IO_FAILED i32 (i32.const 4))
  ;; What the functions marked "returns: result" give: a row whose field isn't an amount, and a
  ;; row without the field.
  (global $NO_ERROR i32 (i32.const 0))
  (global $ERROR_BAD i32 (i32.const 28))
  (global $ERROR_MISSING i32 (i32.const 44))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; Sizes and limits: the longest input, the most bytes a scan reads and the end after them,
  ;; and the most rows.
  (global $MAX_INPUT i32 (i32.const 1048576))
  (global $SCAN_MAX i32 (i32.const 1048577))
  (global $MAX_ROWS i32 (i32.const 1048576))
  ;; What $find_column and $find_field give when there's no such column or field, and what
  ;; $byte_at gives at or past the input's end: 256, which no byte is.
  (global $NO_COLUMN i32 (i32.const -1))
  (global $NO_FIELD i32 (i32.const -1))
  (global $END i32 (i32.const 256))
  ;; An amount: at most 6 digits before its point, so the seventh is one too many, and 2 after
  ;; it; its cents are below CENTS_LIMIT.
  (global $WHOLE_SCAN i32 (i32.const 7))
  (global $ONE_DIGIT i32 (i32.const 1))
  (global $FRACTION_DIGITS i32 (i32.const 2))
  (global $CENTS_LIMIT i64 (i64.const 100000000))
  (global $ONE i32 (i32.const 1))
  ;; The header is line 1, so the first row is line 2.
  (global $FIRST_ROW_LINE i64 (i64.const 2))
  ;; Printing the total: the newline at the output's last byte, the cents' two digits and the
  ;; point before it, and at most 17 whole digits before them, as a 64-bit number of cents has.
  (global $NEWLINE_AT i32 (i32.const 23))
  (global $CENT_AT i32 (i32.const 22))
  (global $TENTH_AT i32 (i32.const 21))
  (global $POINT_AT i32 (i32.const 20))
  (global $MAX_WHOLE_DIGITS i32 (i32.const 17))
  ;; Printing a line's number after a message: at most 20 digits and a newline, from byte
  ;; MESSAGE_ROOM of the message at the latest.
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $MESSAGE_ROOM i32 (i32.const 43))
  (global $TEN i64 (i64.const 10))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))
  (global $COMMA i32 (i32.const 44))
  (global $MINUS i32 (i32.const 45))
  (global $POINT i32 (i32.const 46))
  (global $ZERO i32 (i32.const 48))
  (global $NINE i32 (i32.const 9))

  ;; Regions (rule 3, M1), from 1024 up: the messages and the column's name, read-only, then the
  ;; total and the fault's line, an amount's cents, the output, an error line and the input.
  (global $no_column i32 (i32.const 1024))
  (global $no_column.rosize i32 (i32.const 24))
  (data (i32.const 1024) "total: no amount column\0a")
  (global $bad i32 (i32.const 1056))
  (global $bad.rosize i32 (i32.const 26))
  (data (i32.const 1056) "total: bad amount on line ")
  (global $missing i32 (i32.const 1088))
  (global $missing.rosize i32 (i32.const 30))
  (data (i32.const 1088) "total: missing amount on line ")
  (global $amount i32 (i32.const 1120))
  (global $amount.rosize i32 (i32.const 6))
  (data (i32.const 1120) "amount")
  ;; The total in cents, then the line of the row at fault, which main lends sum_rows to write.
  (global $tally i32 (i32.const 1136))
  (global $tally.size i32 (i32.const 16))
  ;; One row's amount in cents, which sum_rows lends parse_amount to write.
  (global $cents i32 (i32.const 1152))
  (global $cents.size i32 (i32.const 8))
  ;; The total as it prints: a sign, 17 whole digits, a point, 2 digits and a newline.
  (global $out i32 (i32.const 1168))
  (global $out.size i32 (i32.const 24))
  ;; An error line: the longest message, a line's 20 digits at most and a newline, and room to
  ;; spare.
  (global $message i32 (i32.const 1216))  ;; shared: report copies the message, put_number the rest
  (global $message.size i32 (i32.const 64))
  ;; All of standard input, MAX_INPUT bytes and one more, which shows a longer input.
  (global $input i32 (i32.const 1280))  ;; shared: main fills it, and byte_at reads it
  (global $input.size i32 (i32.const 1048577))

  ;; Reads the CSV, finds the amount column, adds up the rows' amounts and prints the total, or
  ;; says what is at fault. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $column i32)
    (local $status i32)
    (block $io_failed
      (block $faulted
        (block $no_column
          (local.set $got
            (call $fill (global.get $STDIN) (global.get $input) (global.get $input.size)
              (global.get $READ_TO_END)))
          (br_if $io_failed (i32.lt_s (local.get $got) (i32.const 0)))
          (if (i32.gt_u (local.get $got) (global.get $input.size))
            (then (call $assert_fail (i32.const 1)) unreachable))
          ;; An input past the limit fills the buffer, and fails as a read that fails does.
          (br_if $io_failed (i32.gt_u (local.get $got) (global.get $MAX_INPUT)))
          (local.set $column (call $find_column (local.get $got)))
          (br_if $no_column (i32.eq (local.get $column) (global.get $NO_COLUMN)))
          ;; The rows start after the header's newline, which find_column found.
          (local.set $status
            (call $sum_rows (local.get $column) (call $skip_line (i32.const 0) (local.get $got))
              (local.get $got) (global.get $tally)))
          (br_if $faulted (local.get $status))
          (br_if $io_failed (call $write_cents (i64.load (global.get $tally))))
          (call $proc_exit (global.get $EXIT_OK))
          unreachable)
        (br_if $io_failed
          (call $write_all (global.get $STDERR) (global.get $no_column)
            (global.get $no_column.rosize)))
        (call $proc_exit (global.get $EXIT_NO_COLUMN))
        unreachable)
      (br_if $io_failed
        (call $report (local.get $status) (i64.load offset=8 (global.get $tally))))
      (if (i32.eq (local.get $status) (global.get $ERROR_MISSING))
        (then (call $proc_exit (global.get $EXIT_MISSING_AMOUNT)) unreachable))
      (call $proc_exit (global.get $EXIT_BAD_AMOUNT))
      unreachable)
    (call $proc_exit (global.get $EXIT_IO_FAILED))
    unreachable)

  ;; The byte at $at of the input, whose first $len bytes came, or END, which no byte is, at or
  ;; past $len.
  (func $byte_at (param $at i32) (param $len i32) (result i32)
    (local $byte i32)
    (if (i32.gt_u (local.get $len) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 2)) unreachable))
    (local.set $byte (global.get $END))
    (if (i32.lt_u (local.get $at) (local.get $len))
      (then
        (if (i32.ge_u (local.get $at) (global.get $input.size))
          (then (call $assert_fail (i32.const 3)) unreachable))
        (local.set $byte (i32.load8_u (i32.add (global.get $input) (local.get $at))))))
    (local.get $byte))

  ;; Where the line that byte $from of the input is in ends: just after its newline, or at the
  ;; input's end, $len.
  (func $skip_line (param $from i32) (param $len i32) (result i32)
    (local $at i32)
    (local $byte i32)
    (local $left i32)
    (if (i32.gt_u (local.get $from) (local.get $len))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (local.set $at (local.get $from))
    (block $done
      (local.set $left (global.get $SCAN_MAX))
      (loop $next  ;; bound: 1048577
        (local.set $byte (call $byte_at (local.get $at) (local.get $len)))
        (br_if $done (i32.eq (local.get $byte) (global.get $END)))
        (local.set $at (i32.add (local.get $at) (i32.const 1)))
        (br_if $done (i32.eq (local.get $byte) (global.get $NEWLINE)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $next (local.get $left)))
      ;; A line has at most MAX_INPUT bytes, and then the input's end.
      (call $assert_fail (i32.const 5))
      unreachable)
    (local.get $at))

  ;; Whether the field from byte $start to byte $end of the input is `amount`: 1 if it is, else
  ;; 0.
  (func $is_amount (param $start i32) (param $end i32) (param $len i32) (result i32)
    (local $k i32)
    (local $same i32)
    (local $left i32)
    (if (i32.gt_u (local.get $start) (local.get $end))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (local.set $same
      (i32.eq (i32.sub (local.get $end) (local.get $start)) (global.get $amount.rosize)))
    (block $compared
      (br_if $compared (i32.eqz (local.get $same)))
      (local.set $left (global.get $amount.rosize))
      (loop $byte  ;; bound: 6
        (if (i32.ge_u (local.get $k) (global.get $amount.rosize))
          (then (call $assert_fail (i32.const 7)) unreachable))
        (if (i32.ne (call $byte_at (i32.add (local.get $start) (local.get $k)) (local.get $len))
              (i32.load8_u (i32.add (global.get $amount) (local.get $k))))
          (then (local.set $same (i32.const 0))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $same))

  ;; Finds the column named `amount` in the header, the input's first line: gives the number of
  ;; the first such field, from 0, or NO_COLUMN when there is none or the header has no newline.
  ;; A comma or a newline ends a field.
  (func $find_column (param $len i32) (result i32)
    (local $at i32)
    (local $start i32)
    (local $field i32)
    (local $found i32)
    (local $column i32)
    (local $byte i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 8)) unreachable))
    (local.set $found (global.get $NO_COLUMN))
    (local.set $column (global.get $NO_COLUMN))
    (block $done
      (local.set $left (global.get $SCAN_MAX))
      (loop $next  ;; bound: 1048577
        (local.set $byte (call $byte_at (local.get $at) (local.get $len)))
        ;; The input ends inside the header: no column.
        (br_if $done (i32.eq (local.get $byte) (global.get $END)))
        (if (i32.or (i32.eq (local.get $byte) (global.get $COMMA))
              (i32.eq (local.get $byte) (global.get $NEWLINE)))
          (then
            (if (i32.and (i32.eq (local.get $found) (global.get $NO_COLUMN))
                  (call $is_amount (local.get $start) (local.get $at) (local.get $len)))
              (then (local.set $found (local.get $field))))
            (local.set $field (i32.add (local.get $field) (i32.const 1)))
            (local.set $start (i32.add (local.get $at) (i32.const 1)))
            ;; The header's newline.
            (if (i32.eq (local.get $byte) (global.get $NEWLINE))
              (then
                (local.set $column (local.get $found))
                (br $done)))))
        (local.set $at (i32.add (local.get $at) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $next (local.get $left)))
      ;; A header has at most MAX_INPUT bytes, and then the input's end.
      (call $assert_fail (i32.const 9))
      unreachable)
    (local.get $column))

  ;; Finds where field $column of the row from byte $start of the input begins: just after that
  ;; many commas. Gives the place, or NO_FIELD when the row or the input ends first.
  (func $find_field (param $column i32) (param $start i32) (param $len i32) (result i32)
    (local $at i32)
    (local $commas i32)
    (local $found i32)
    (local $byte i32)
    (local $left i32)
    (if (i32.ge_u (local.get $start) (local.get $len))
      (then (call $assert_fail (i32.const 10)) unreachable))
    (local.set $found (global.get $NO_FIELD))
    (local.set $at (local.get $start))
    (block $done
      ;; The first field starts with the row.
      (if (i32.eqz (local.get $column))
        (then
          (local.set $found (local.get $start))
          (br $done)))
      (local.set $left (global.get $SCAN_MAX))
      (loop $next  ;; bound: 1048577
        (local.set $byte (call $byte_at (local.get $at) (local.get $len)))
        (br_if $done (i32.eq (local.get $byte) (global.get $END)))
        (br_if $done (i32.eq (local.get $byte) (global.get $NEWLINE)))
        (local.set $at (i32.add (local.get $at) (i32.const 1)))
        (if (i32.eq (local.get $byte) (global.get $COMMA))
          (then
            (local.set $commas (i32.add (local.get $commas) (i32.const 1)))
            (if (i32.eq (local.get $commas) (local.get $column))
              (then
                (local.set $found (local.get $at))
                (br $done)))))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $next (local.get $left)))
      ;; A row has at most MAX_INPUT bytes, and then the input's end.
      (call $assert_fail (i32.const 11))
      unreachable)
    (local.get $found))

  ;; Reads an amount without its sign, from byte $at of the input: 1 to 6 digits, with no zero
  ;; at the start but a lone 0, a point and 2 digits, ended by a comma, a newline or the input's
  ;; end. Writes it in cents at $into, 8 bytes. Gives 0, or ERROR_BAD.
  (func $parse_cents  ;; returns: result
    (param $at i32) (param $len i32) (param $into i32) (result i32)
    (local $i i32)
    (local $digit i32)
    (local $digits i32)
    (local $byte i32)
    (local $cents i64)
    (local $status i32)
    (local $left i32)
    (if (i32.gt_u (local.get $at) (local.get $len))
      (then (call $assert_fail (i32.const 12)) unreachable))
    (local.set $status (global.get $ERROR_BAD))
    (local.set $i (local.get $at))
    (block $bad
      ;; The whole part, up to the point.
      (block $point
        (local.set $left (global.get $WHOLE_SCAN))
        (loop $whole  ;; bound: 7
          (local.set $byte (call $byte_at (local.get $i) (local.get $len)))
          (br_if $point (i32.eq (local.get $byte) (global.get $POINT)))
          ;; Below '0' wraps to a large number, so one unsigned test catches both sides.
          (local.set $digit (i32.sub (local.get $byte) (global.get $ZERO)))
          (br_if $bad (i32.gt_u (local.get $digit) (global.get $NINE)))
          (local.set $cents
            (i64.add (i64.mul (local.get $cents) (global.get $TEN))
              (i64.extend_i32_u (local.get $digit))))
          (local.set $digits (i32.add (local.get $digits) (i32.const 1)))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $whole (local.get $left)))
        ;; Seven digits and no point are more than an amount has.
        (br $bad))
      ;; 1 to 6 digits, the first not a zero but in a lone 0.
      (br_if $bad (i32.eqz (local.get $digits)))
      (br_if $bad
        (i32.and (i32.gt_u (local.get $digits) (global.get $ONE_DIGIT))
          (i32.eq (call $byte_at (local.get $at) (local.get $len)) (global.get $ZERO))))
      ;; The 2 digits after the point.
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (local.set $left (global.get $FRACTION_DIGITS))
      (loop $fraction  ;; bound: 2
        (local.set $digit
          (i32.sub (call $byte_at (local.get $i) (local.get $len)) (global.get $ZERO)))
        (br_if $bad (i32.gt_u (local.get $digit) (global.get $NINE)))
        (local.set $cents
          (i64.add (i64.mul (local.get $cents) (global.get $TEN))
            (i64.extend_i32_u (local.get $digit))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $fraction (local.get $left)))
      ;; The field ends there: at a comma, a newline or the input's end.
      (local.set $byte (call $byte_at (local.get $i) (local.get $len)))
      (br_if $bad
        (i32.and (i32.ne (local.get $byte) (global.get $COMMA))
          (i32.and (i32.ne (local.get $byte) (global.get $NEWLINE))
            (i32.ne (local.get $byte) (global.get $END)))))
      ;; At most 999999.99, so the cents are below 100000000.
      (if (i64.ge_u (local.get $cents) (global.get $CENTS_LIMIT))
        (then (call $assert_fail (i32.const 13)) unreachable))
      (i64.store (local.get $into) (local.get $cents))
      (local.set $status (global.get $NO_ERROR)))
    (local.get $status))

  ;; Reads the amount that starts at byte $at of the input: an optional minus sign, then the
  ;; cents. Writes it at $into, 8 bytes. Gives 0, or ERROR_BAD.
  (func $parse_amount  ;; returns: result
    (param $at i32) (param $len i32) (param $into i32) (result i32)
    (local $negative i32)
    (local $status i32)
    (if (i32.gt_u (local.get $at) (local.get $len))
      (then (call $assert_fail (i32.const 14)) unreachable))
    (local.set $negative
      (i32.eq (call $byte_at (local.get $at) (local.get $len)) (global.get $MINUS)))
    (local.set $status
      (call $parse_cents (i32.add (local.get $at) (local.get $negative)) (local.get $len)
        (local.get $into)))
    (if (i32.eqz (local.get $status))
      (then
        (if (local.get $negative)
          (then
            (i64.store (local.get $into) (i64.sub (i64.const 0) (i64.load (local.get $into))))))))
    ;; An amount is read, or it isn't one.
    (if (i32.and (i32.ne (local.get $status) (global.get $NO_ERROR))
          (i32.ne (local.get $status) (global.get $ERROR_BAD)))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (local.get $status))

  ;; Adds up the amount field of each row from byte $start of the input on. Writes the total in
  ;; cents at $into, 8 bytes, and the line of the last row it read 8 bytes on. Gives 0,
  ;; ERROR_MISSING for a row without the field, or ERROR_BAD for one whose field isn't an amount.
  (func $sum_rows  ;; returns: result
    (param $column i32) (param $start i32) (param $len i32) (param $into i32) (result i32)
    (local $at i32)
    (local $field i32)
    (local $line i64)
    (local $total i64)
    (local $status i32)
    (local $left i32)
    (if (i32.gt_u (local.get $len) (global.get $MAX_INPUT))
      (then (call $assert_fail (i32.const 16)) unreachable))
    (if (i32.gt_u (local.get $start) (local.get $len))
      (then (call $assert_fail (i32.const 17)) unreachable))
    (local.set $at (local.get $start))
    (local.set $line (global.get $FIRST_ROW_LINE))
    (block $fault
      (block $done
        (local.set $left (global.get $MAX_ROWS))
        (loop $row  ;; bound: 1048576
          (br_if $done (i32.ge_u (local.get $at) (local.get $len)))
          (local.set $field
            (call $find_field (local.get $column) (local.get $at) (local.get $len)))
          (local.set $status (global.get $ERROR_MISSING))
          (br_if $fault (i32.eq (local.get $field) (global.get $NO_FIELD)))
          (local.set $status
            (call $parse_amount (local.get $field) (local.get $len) (global.get $cents)))
          (br_if $fault (local.get $status))
          (local.set $total (i64.add (local.get $total) (i64.load (global.get $cents))))
          (local.set $at (call $skip_line (local.get $field) (local.get $len)))
          (local.set $line (i64.add (local.get $line) (i64.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $row (local.get $left)))
        ;; Each row takes a byte at least, after the header's, so the rows are fewer.
        (call $assert_fail (i32.const 18))
        unreachable)
      (i64.store (local.get $into) (local.get $total))
      (local.set $status (global.get $NO_ERROR)))
    (i64.store offset=8 (local.get $into) (local.get $line))
    (local.get $status))

  ;; Writes $cents as a decimal with two places, then a newline, to standard output: a minus sign
  ;; for a total below zero, and 0.00 for zero. Gives 0, or an error number.
  (func $write_cents (param $cents i64) (result i32)  ;; returns: result
    (local $magnitude i64)
    (local $at i32)
    (local $left i32)
    (local.set $magnitude (local.get $cents))
    (if (i64.lt_s (local.get $cents) (i64.const 0))
      (then (local.set $magnitude (i64.sub (i64.const 0) (local.get $cents)))))
    ;; The most negative number has no magnitude in 64 bits.
    (if (i64.lt_s (local.get $magnitude) (i64.const 0))
      (then (call $assert_fail (i32.const 19)) unreachable))
    ;; From the last: the newline, the cents' two digits, and the point before them.
    (i32.store8 (i32.add (global.get $out) (global.get $NEWLINE_AT)) (global.get $NEWLINE))
    (i32.store8 (i32.add (global.get $out) (global.get $CENT_AT))
      (i32.add (global.get $ZERO)
        (i32.wrap_i64 (i64.rem_u (local.get $magnitude) (global.get $TEN)))))
    (local.set $magnitude (i64.div_u (local.get $magnitude) (global.get $TEN)))
    (i32.store8 (i32.add (global.get $out) (global.get $TENTH_AT))
      (i32.add (global.get $ZERO)
        (i32.wrap_i64 (i64.rem_u (local.get $magnitude) (global.get $TEN)))))
    (local.set $magnitude (i64.div_u (local.get $magnitude) (global.get $TEN)))
    (i32.store8 (i32.add (global.get $out) (global.get $POINT_AT)) (global.get $POINT))
    ;; The whole part's digits, at least one, from the last.
    (local.set $at (global.get $POINT_AT))
    (block $written
      (local.set $left (global.get $MAX_WHOLE_DIGITS))
      (loop $digit  ;; bound: 17
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 20)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $magnitude) (global.get $TEN)))))
        (local.set $magnitude (i64.div_u (local.get $magnitude) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $magnitude)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; A 64-bit number of cents has at most 17 whole digits.
      (call $assert_fail (i32.const 21))
      unreachable)
    ;; The sign, for a total below zero.
    (if (i64.lt_s (local.get $cents) (i64.const 0))
      (then
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 22)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $MINUS))))
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))

  ;; Writes $number's decimal digits into the error line from byte $at on, and a newline after
  ;; them, and gives where the line ends.
  (func $put_number (param $number i64) (param $at i32) (result i32)
    (local $digits i32)
    (local $rest i64)
    (local $to i32)
    (local $left i32)
    (if (i32.gt_u (local.get $at) (global.get $MESSAGE_ROOM))
      (then (call $assert_fail (i32.const 23)) unreachable))
    ;; How many digits: one, and one more for each time ten goes into what is left.
    (local.set $rest (local.get $number))
    (block $counted
      (local.set $left (global.get $MAX_DIGITS))
      (loop $count  ;; bound: 20
        (local.set $digits (i32.add (local.get $digits) (i32.const 1)))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $counted (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $count (local.get $left)))
      ;; A 64-bit number has at most 20 digits.
      (call $assert_fail (i32.const 24))
      unreachable)
    ;; The digits from the last.
    (local.set $to (i32.add (local.get $at) (local.get $digits)))
    (local.set $rest (local.get $number))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $to (i32.sub (local.get $to) (i32.const 1)))
        (if (i32.ge_u (local.get $to) (global.get $message.size))
          (then (call $assert_fail (i32.const 25)) unreachable))
        (i32.store8 (i32.add (global.get $message) (local.get $to))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      (call $assert_fail (i32.const 26))
      unreachable)
    ;; The newline after them.
    (local.set $to (i32.add (local.get $at) (local.get $digits)))
    (if (i32.ge_u (local.get $to) (global.get $message.size))
      (then (call $assert_fail (i32.const 27)) unreachable))
    (i32.store8 (i32.add (global.get $message) (local.get $to)) (global.get $NEWLINE))
    (i32.add (local.get $to) (global.get $ONE)))

  ;; Prints the message for the fault $status, then the line's number $line and a newline, to
  ;; standard error. Gives 0, or an error number.
  (func $report (param $status i32) (param $line i64) (result i32)  ;; returns: result
    (local $length i32)
    (if (i32.eq (local.get $status) (global.get $ERROR_MISSING))
      (then
        (memory.copy (global.get $message) (global.get $missing) (global.get $missing.rosize))
        (local.set $length (global.get $missing.rosize)))
      (else
        ;; The faults are a row without the field and a field that isn't an amount.
        (if (i32.ne (local.get $status) (global.get $ERROR_BAD))
          (then (call $assert_fail (i32.const 28)) unreachable))
        (memory.copy (global.get $message) (global.get $bad) (global.get $bad.rosize))
        (local.set $length (global.get $bad.rosize))))
    (local.set $length (call $put_number (local.get $line) (local.get $length)))
    (if (i32.gt_u (local.get $length) (global.get $message.size))
      (then (call $assert_fail (i32.const 29)) unreachable))
    (call $write_all (global.get $STDERR) (global.get $message) (local.get $length)))
)
