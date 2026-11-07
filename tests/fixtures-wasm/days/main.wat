;; clock: prints the current UTC time as hh:mm:ss, or the number of days from today to the date in
;; its argument. The WebAssembly counterpart of tests/fixtures/days, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $assert_fail and $write_all.
;; Text not in a date's form is the usage line's, and a date in that form that names no day is
;; shown back in its message. A date's day is counted from 1970-01-01 by Howard Hinnant's
;; days_from_civil; the clock reads in nanoseconds, of which the program keeps the whole seconds.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "clock_time_get"
    (func $clock_time_get (param i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; Three pages: lengua's first 1024 bytes, the messages, the months' days and the small
  ;; regions, then the arguments' text from 4096.
  (memory (export "memory") 3 3)

  ;; Descriptors and clocks: the time of day, read to the microsecond.
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  (global $REALTIME i32 (i32.const 0))
  (global $PRECISION i64 (i64.const 1000))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_NO_DATE i32 (i32.const 2))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  ;; What the functions marked "returns: result" give: text not in a date's form, and a date in
  ;; that form that names no day.
  (global $NO_ERROR i32 (i32.const 0))
  (global $ERROR_INVALID i32 (i32.const 28))
  (global $ERROR_NO_DATE i32 (i32.const 44))
  ;; Argument counts, the program's name included, and the mark of a date given.
  (global $ARGC_TIME i32 (i32.const 1))
  (global $ARGC_DATE i32 (i32.const 2))
  (global $A_DATE i32 (i32.const 1))
  ;; Time.
  (global $NANOSECONDS i64 (i64.const 1000000000))
  (global $SECONDS_PER_DAY i64 (i64.const 86400))
  (global $SECONDS_PER_HOUR i64 (i64.const 3600))
  (global $SECONDS_PER_MINUTE i64 (i64.const 60))
  (global $HOURS_PER_DAY i64 (i64.const 24))
  (global $MINUTES_PER_HOUR i64 (i64.const 60))
  ;; A date, YYYY-MM-DD: ten characters, its dashes at 4 and 7; and the last place in $args a
  ;; date may start at.
  (global $DATE_LEN i32 (i32.const 10))
  (global $FIRST_DASH_AT i32 (i32.const 4))
  (global $SECOND_DASH_AT i32 (i32.const 7))
  (global $LAST_DATE_AT i32 (i32.const 131062))
  ;; The calendar: February has 29 days in a leap year, which 4 divides, but a century only
  ;; where 400 divides it too.
  (global $MIN_YEAR i64 (i64.const 1970))
  (global $MONTHS i64 (i64.const 12))
  (global $FEBRUARY i64 (i64.const 2))
  (global $LEAP_FEBRUARY i64 (i64.const 29))
  (global $FOUR i64 (i64.const 4))
  (global $HUNDRED i64 (i64.const 100))
  (global $FOUR_HUNDRED i64 (i64.const 400))
  (global $ONE i64 (i64.const 1))
  ;; days_from_civil's numbers: a 400-year era's days and a year's; a month's days from March
  ;; on, 153 times its place from March, plus 2, divided by 5; and the days from 0000-03-01 to
  ;; 1970-01-01.
  (global $DAYS_PER_ERA i64 (i64.const 146097))
  (global $DAYS_PER_YEAR i64 (i64.const 365))
  (global $MARCH_SHIFT i64 (i64.const 9))
  (global $MONTH_DAYS_FACTOR i64 (i64.const 153))
  (global $MONTH_DAYS_ROUND i64 (i64.const 2))
  (global $MONTH_DAYS_DIVISOR i64 (i64.const 5))
  (global $EPOCH_DAYS i64 (i64.const 719468))
  ;; The time's line, hh:mm:ss and a newline: where the minutes, the seconds, each colon and the
  ;; newline go; the hours are at its start.
  (global $FIRST_COLON_AT i32 (i32.const 2))
  (global $MINUTES_AT i32 (i32.const 3))
  (global $SECOND_COLON_AT i32 (i32.const 5))
  (global $SECONDS_AT i32 (i32.const 6))
  (global $TIME_NEWLINE_AT i32 (i32.const 8))
  ;; The days' line: a sign, 19 digits and a newline.
  (global $MAX_DIGITS i32 (i32.const 19))
  (global $NEWLINE_AT i32 (i32.const 20))
  (global $TEN i64 (i64.const 10))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))
  (global $MINUS i32 (i32.const 45))
  (global $DASH i32 (i32.const 45))
  (global $COLON i32 (i32.const 58))
  (global $ZERO i32 (i32.const 48))
  (global $NINE i32 (i32.const 9))

  ;; Regions (rule 3, M1), from 1024 up: the messages and the months' days, read-only, then what
  ;; the host writes, the date's day, the lines printed, and the arguments' text.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 26))
  (data (i32.const 1024) "usage: clock [YYYY-MM-DD]\0a")
  (global $no_date i32 (i32.const 1056))
  (global $no_date.rosize i32 (i32.const 21))
  (data (i32.const 1056) "clock: no such date: ")
  (global $newline i32 (i32.const 1080))
  (global $newline.rosize i32 (i32.const 1))
  (data (i32.const 1080) "\0a")
  ;; The days of each month of a year that isn't a leap year, January's first.
  (global $month_days i32 (i32.const 1104))
  (global $month_days.rosize i32 (i32.const 12))
  (data (i32.const 1104) "\1f\1c\1f\1e\1f\1e\1f\1f\1e\1f\1e\1f")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 2048))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then the date.
  (global $argv i32 (i32.const 2064))
  (global $argv.size i32 (i32.const 8))
  ;; What the clock writes, and the date's day from 1970-01-01, which main lends parse_date to
  ;; write.
  (global $time i32 (i32.const 2080))
  (global $time.size i32 (i32.const 8))
  (global $day_number i32 (i32.const 2096))
  (global $day_number.size i32 (i32.const 8))
  ;; The time's line, which print_time lends put_two a part at a time, and the days' line.
  (global $time_line i32 (i32.const 2112))
  (global $time_line.size i32 (i32.const 9))
  (global $out i32 (i32.const 2128))
  (global $out.size i32 (i32.const 21))
  ;; The arguments' text, each ended by a NUL: 128 KiB, more than any test's arguments take.
  (global $args i32 (i32.const 4096))  ;; shared: main has the host fill it; parse_date reads it
  (global $args.size i32 (i32.const 131072))

  ;; With no argument prints the time of day; with a date, the days from today to it; with other
  ;; text or more arguments, the usage line. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $count i32)
    (local $at i32)
    (local $dated i32)
    (local $seconds i64)
    (block $write_failed
      (block $no_date
        (block $usage
          (local.set $err
            (call $args_sizes_get (global.get $sizes) (i32.add (global.get $sizes) (i32.const 4))))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 1)) unreachable))
          (local.set $count (i32.load (global.get $sizes)))
          (block $read_clock
            (br_if $read_clock (i32.eq (local.get $count) (global.get $ARGC_TIME)))
            (br_if $usage (i32.ne (local.get $count) (global.get $ARGC_DATE)))
            ;; Text longer than $args holds isn't a date.
            (br_if $usage
              (i32.gt_u (i32.load offset=4 (global.get $sizes)) (global.get $args.size)))
            (local.set $err (call $args_get (global.get $argv) (global.get $args)))
            (if (local.get $err)
              (then (call $assert_fail (i32.const 2)) unreachable))
            ;; The date starts where argv[1] points: an offset into $args, which parse_date
            ;; checks.
            (local.set $at (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args)))
            (local.set $err (call $parse_date (local.get $at) (global.get $day_number)))
            (br_if $usage (i32.eq (local.get $err) (global.get $ERROR_INVALID)))
            (br_if $no_date (local.get $err))
            (local.set $dated (global.get $A_DATE)))
          ;; The clock, in whole seconds since 1970: an idea that names the clock can read it.
          (local.set $err
            (call $clock_time_get (global.get $REALTIME) (global.get $PRECISION)
              (global.get $time)))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 3)) unreachable))
          (local.set $seconds (i64.div_u (i64.load (global.get $time)) (global.get $NANOSECONDS)))
          (if (local.get $dated)
            (then
              ;; The days from the UTC date today to the date.
              (br_if $write_failed
                (call $write_i64
                  (i64.sub (i64.load (global.get $day_number))
                    (i64.div_u (local.get $seconds) (global.get $SECONDS_PER_DAY))))))
            (else (br_if $write_failed (call $print_time (local.get $seconds)))))
          (call $proc_exit (global.get $EXIT_OK))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
        (call $proc_exit (global.get $EXIT_USAGE))
        unreachable)
      ;; The date back, DATE_LEN characters at $at, where parse_date read them and the NUL after.
      (br_if $write_failed
        (call $write_all (global.get $STDERR) (global.get $no_date) (global.get $no_date.rosize)))
      (if (i32.gt_u (local.get $at) (global.get $LAST_DATE_AT))
        (then (call $assert_fail (i32.const 4)) unreachable))
      (br_if $write_failed
        (call $write_all (global.get $STDERR) (i32.add (global.get $args) (local.get $at))
          (global.get $DATE_LEN)))
      (br_if $write_failed
        (call $write_all (global.get $STDERR) (global.get $newline) (global.get $newline.rosize)))
      (call $proc_exit (global.get $EXIT_NO_DATE))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Reads the date that starts at byte $at of $args, four digits, a dash, two digits, a dash and
  ;; two digits, then the argument's NUL, and writes its day from 1970-01-01 at $into, 8 bytes.
  ;; Gives 0; ERROR_INVALID when the text isn't in that form; or ERROR_NO_DATE when it names no
  ;; day from 1970-01-01 to 9999-12-31 on the calendar.
  (func $parse_date (param $at i32) (param $into i32) (result i32)  ;; returns: result
    (local $k i32)
    (local $i i32)
    (local $c i32)
    (local $field i64)
    (local $year i64)
    (local $month i64)
    (local $left i32)
    (local $status i32)
    (local.set $status (global.get $ERROR_INVALID))
    (block $rejected
      ;; The ten characters: a dash at each dash's place and a digit elsewhere. A dash ends a
      ;; field, the year and then the month, and the day is the last.
      (local.set $left (global.get $DATE_LEN))
      (loop $char  ;; bound: 10
        (local.set $i (i32.add (local.get $at) (local.get $k)))
        (if (i32.ge_u (local.get $i) (global.get $args.size))
          (then (call $assert_fail (i32.const 5)) unreachable))
        (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $i))))
        (if (i32.or (i32.eq (local.get $k) (global.get $FIRST_DASH_AT))
              (i32.eq (local.get $k) (global.get $SECOND_DASH_AT)))
          (then
            (br_if $rejected (i32.ne (local.get $c) (global.get $DASH)))
            (local.set $year (local.get $month))
            (local.set $month (local.get $field))
            (local.set $field (i64.const 0)))
          (else
            ;; Below '0' wraps to a large number, so one unsigned test catches both sides.
            (local.set $c (i32.sub (local.get $c) (global.get $ZERO)))
            (br_if $rejected (i32.gt_u (local.get $c) (global.get $NINE)))
            (local.set $field
              (i64.add (i64.mul (local.get $field) (global.get $TEN))
                (i64.extend_i32_u (local.get $c))))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $char (local.get $left)))
      ;; Then the argument ends.
      (local.set $i (i32.add (local.get $at) (global.get $DATE_LEN)))
      (if (i32.ge_u (local.get $i) (global.get $args.size))
        (then (call $assert_fail (i32.const 6)) unreachable))
      (br_if $rejected (i32.load8_u (i32.add (global.get $args) (local.get $i))))
      ;; A real day from 1970-01-01: a year of four digits is at most 9999.
      (local.set $status (global.get $ERROR_NO_DATE))
      (br_if $rejected (i64.lt_u (local.get $year) (global.get $MIN_YEAR)))
      (br_if $rejected (i64.eqz (local.get $month)))
      (br_if $rejected (i64.gt_u (local.get $month) (global.get $MONTHS)))
      (br_if $rejected (i64.eqz (local.get $field)))
      (br_if $rejected
        (i64.gt_u (local.get $field) (call $days_in_month (local.get $year) (local.get $month))))
      (i64.store (local.get $into)
        (call $civil_days (local.get $year) (local.get $month) (local.get $field)))
      (local.set $status (global.get $NO_ERROR)))
    (local.get $status))

  ;; Gives how many days month $month, from 1 to 12, of year $year, from 1970 on, has: 28 to 31.
  (func $days_in_month (param $year i64) (param $month i64) (result i64)
    (local $index i32)
    (local $days i64)
    (if (i64.lt_u (local.get $year) (global.get $MIN_YEAR))
      (then (call $assert_fail (i32.const 7)) unreachable))
    ;; The month's place in the table, January's 0.
    (local.set $index (i32.wrap_i64 (i64.sub (local.get $month) (global.get $ONE))))
    (if (i32.ge_u (local.get $index) (global.get $month_days.rosize))
      (then (call $assert_fail (i32.const 8)) unreachable))
    (local.set $days (i64.load8_u (i32.add (global.get $month_days) (local.get $index))))
    ;; February of a leap year.
    (if (i64.eq (local.get $month) (global.get $FEBRUARY))
      (then
        (if (i32.and (i64.eqz (i64.rem_u (local.get $year) (global.get $FOUR)))
              (i32.or (i64.ne (i64.rem_u (local.get $year) (global.get $HUNDRED)) (i64.const 0))
                (i64.eqz (i64.rem_u (local.get $year) (global.get $FOUR_HUNDRED)))))
          (then (local.set $days (global.get $LEAP_FEBRUARY))))))
    (local.get $days))

  ;; Counts the days from 1970-01-01 to day $day of month $month of year $year, a day on the
  ;; calendar from 1970-01-01 on, by Howard Hinnant's days_from_civil: its years start in March,
  ;; so a leap day is a year's last.
  (func $civil_days (param $year i64) (param $month i64) (param $day i64) (result i64)
    (local $shifted i64)
    (local $of_era i64)
    (local $days i64)
    (if (i64.lt_u (local.get $year) (global.get $MIN_YEAR))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (if (i64.eqz (local.get $month))
      (then (call $assert_fail (i32.const 10)) unreachable))
    (if (i64.gt_u (local.get $month) (global.get $MONTHS))
      (then (call $assert_fail (i32.const 11)) unreachable))
    ;; January and February count with the year before.
    (local.set $shifted (local.get $year))
    (if (i64.le_u (local.get $month) (global.get $FEBRUARY))
      (then (local.set $shifted (i64.sub (local.get $year) (global.get $ONE)))))
    (local.set $of_era (i64.rem_u (local.get $shifted) (global.get $FOUR_HUNDRED)))
    ;; The eras' days, then the era's years', with a leap day every 4 years but every 100.
    (local.set $days
      (i64.add
        (i64.mul (i64.div_u (local.get $shifted) (global.get $FOUR_HUNDRED))
          (global.get $DAYS_PER_ERA))
        (i64.sub
          (i64.add (i64.mul (local.get $of_era) (global.get $DAYS_PER_YEAR))
            (i64.div_u (local.get $of_era) (global.get $FOUR)))
          (i64.div_u (local.get $of_era) (global.get $HUNDRED)))))
    ;; Then the months' days from March, and the days before the day.
    (local.set $days
      (i64.add (local.get $days)
        (i64.sub
          (i64.add
            (i64.div_u
              (i64.add
                (i64.mul (global.get $MONTH_DAYS_FACTOR)
                  (i64.rem_u (i64.add (local.get $month) (global.get $MARCH_SHIFT))
                    (global.get $MONTHS)))
                (global.get $MONTH_DAYS_ROUND))
              (global.get $MONTH_DAYS_DIVISOR))
            (local.get $day))
          (global.get $ONE))))
    (if (i64.lt_u (local.get $days) (global.get $EPOCH_DAYS))
      (then (call $assert_fail (i32.const 12)) unreachable))
    (i64.sub (local.get $days) (global.get $EPOCH_DAYS)))

  ;; Writes the time of day of $seconds since 1970, in UTC, as hh:mm:ss and a newline, to standard
  ;; output. Gives 0, or an error number.
  (func $print_time (param $seconds i64) (result i32)  ;; returns: result
    (local $of_day i64)
    (local $hours i64)
    (local $minutes i64)
    (local.set $of_day (i64.rem_u (local.get $seconds) (global.get $SECONDS_PER_DAY)))
    (local.set $hours (i64.div_u (local.get $of_day) (global.get $SECONDS_PER_HOUR)))
    (if (i64.ge_u (local.get $hours) (global.get $HOURS_PER_DAY))
      (then (call $assert_fail (i32.const 13)) unreachable))
    (local.set $minutes
      (i64.div_u (i64.rem_u (local.get $of_day) (global.get $SECONDS_PER_HOUR))
        (global.get $SECONDS_PER_MINUTE)))
    (if (i64.ge_u (local.get $minutes) (global.get $MINUTES_PER_HOUR))
      (then (call $assert_fail (i32.const 14)) unreachable))
    (call $put_two (local.get $hours) (global.get $time_line))
    (i32.store8 (i32.add (global.get $time_line) (global.get $FIRST_COLON_AT)) (global.get $COLON))
    (call $put_two (local.get $minutes) (i32.add (global.get $time_line) (global.get $MINUTES_AT)))
    (i32.store8 (i32.add (global.get $time_line) (global.get $SECOND_COLON_AT)) (global.get $COLON))
    (call $put_two (i64.rem_u (local.get $of_day) (global.get $SECONDS_PER_MINUTE))
      (i32.add (global.get $time_line) (global.get $SECONDS_AT)))
    (i32.store8 (i32.add (global.get $time_line) (global.get $TIME_NEWLINE_AT))
      (global.get $NEWLINE))
    (call $write_all (global.get $STDOUT) (global.get $time_line) (global.get $time_line.size)))

  ;; Writes $value, from 0 to 99, as two decimal digits at $at, which print_time lends it: the
  ;; tens, then the ones, so a value below ten has a zero first.
  (func $put_two (param $value i64) (param $at i32)
    (local $tens i64)
    (if (i64.ge_u (local.get $value) (global.get $HUNDRED))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (local.set $tens (i64.div_u (local.get $value) (global.get $TEN)))
    (if (i64.ge_u (local.get $tens) (global.get $TEN))
      (then (call $assert_fail (i32.const 16)) unreachable))
    (i32.store8 (local.get $at) (i32.add (global.get $ZERO) (i32.wrap_i64 (local.get $tens))))
    (i32.store8 offset=1 (local.get $at)
      (i32.add (global.get $ZERO)
        (i32.wrap_i64 (i64.rem_u (local.get $value) (global.get $TEN))))))

  ;; Writes $value in decimal, then a newline, to standard output. Gives 0, or an error number.
  (func $write_i64 (param $value i64) (result i32)  ;; returns: result
    (local $magnitude i64)
    (local $at i32)
    (local $left i32)
    (local.set $magnitude (local.get $value))
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then (local.set $magnitude (i64.sub (i64.const 0) (local.get $value)))))
    ;; The most negative number has no magnitude in 64 bits.
    (if (i64.lt_s (local.get $magnitude) (i64.const 0))
      (then (call $assert_fail (i32.const 17)) unreachable))
    ;; The newline last, the digits before it from the last, then the sign.
    (local.set $at (global.get $NEWLINE_AT))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (block $sign
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 19
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 18)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $magnitude) (global.get $TEN)))))
        (local.set $magnitude (i64.div_u (local.get $magnitude) (global.get $TEN)))
        (br_if $sign (i64.eqz (local.get $magnitude)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; 19 digits are all a magnitude has.
      (call $assert_fail (i32.const 19))
      unreachable)
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 20)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $MINUS))))
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
