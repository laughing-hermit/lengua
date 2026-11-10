;; files: reads every file in the working folder and prints how many files there are and how many
;; bytes they hold. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md): lengua's
;; library gives $write_all. It lists the working folder a batch of entries at a time with
;; fd_readdir, each batch from the cookie of the last whole entry before it, as sweep does; it
;; opens each file with path_open, reads it to its end a 64 KiB buffer-full at a time with
;; fd_read, and closes it. On a local disk, where the benchmark's folders are, a read of a file
;; gives every byte it asks for up to the file's end (references/host.md), so a read that gives
;; less than a buffer-full is the file's last, and a file shorter than the buffer takes one read
;; where $fill would take a second to find its end; on a network or FUSE mount a read may give
;; less, and the program would count fewer bytes. Folders are neither counted nor read.
(module
  (import "wasi_snapshot_preview1" "fd_readdir"
    (func $fd_readdir (param i32 i32 i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_read" (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; Three pages: lengua's first 1024 bytes, then the regions below, which end at 132480.
  (memory (export "memory") 3 3)

  ;; Descriptors: standard output, and the working folder, which lengua opens as 3.
  (global $STDOUT i32 (i32.const 1))
  (global $FOLDER i32 (i32.const 3))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_WRITE_FAILED i32 (i32.const 2))
  (global $EXIT_FAILED i32 (i32.const 3))
  ;; What the counting gives: OK, or FAILED when a call fails; and a file's size when it can't be
  ;; read.
  (global $OK i32 (i32.const 0))
  (global $FAILED i32 (i32.const -1))
  (global $UNREADABLE i64 (i64.const -1))
  ;; path_open's flags: none; and its rights: none, or to read.
  (global $NO_FLAGS i32 (i32.const 0))
  (global $NO_RIGHTS i64 (i64.const 0))
  (global $READ_RIGHTS i64 (i64.const 2))
  ;; The buffers a read of a file fills: one, the whole of the buffer for a file's content.
  (global $ONE_BUFFER i32 (i32.const 1))
  ;; The buffer-fulls a file takes at most: 2147483647 of 64 KiB, 128 TiB.
  (global $MAX_PIECES i32 (i32.const 2147483647))
  ;; A folder's entries, as fd_readdir writes them: each a dirent of 24 bytes, the next entry's
  ;; cookie at 0, the name's length at 16 and its kind at 20, then the name. A file is kind 4.
  ;; LAST_DIRENT is the last place a whole dirent fits in the entries.
  (global $DIRENT i32 (i32.const 24))
  (global $LAST_DIRENT i32 (i32.const 65512))
  (global $REGULAR_FILE i32 (i32.const 4))
  ;; Sizes and limits: the batches a folder of 65536 entries takes at most, each entry 279 bytes
  ;; at most, so 234 a batch at least; the entries a batch holds at most, each 24 bytes at least;
  ;; and the longest name every system takes.
  (global $MAX_BATCHES i32 (i32.const 512))
  (global $MAX_ENTRIES i32 (i32.const 2731))
  (global $NAME_MAX i32 (i32.const 255))
  ;; Printing: two counts of up to 20 digits, a space between them and a newline after, the
  ;; newline at the output's last byte; the second count starts at FIRST_ROOM or later.
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $NEWLINE_AT i32 (i32.const 41))
  (global $FIRST_ROOM i32 (i32.const 21))
  (global $TEN i64 (i64.const 10))
  (global $NEWLINE i32 (i32.const 10))
  (global $SPACE i32 (i32.const 32))
  (global $ZERO i32 (i32.const 48))

  ;; Regions (rule 3, M1), from 1024 up: what the host writes, the line it prints, a file's name,
  ;; the iovec of a file's reads and the count they give, a batch of entries and a buffer-full of
  ;; a file.
  ;; How many bytes of entries fd_readdir wrote.
  (global $listed i32 (i32.const 1024))
  (global $listed.size i32 (i32.const 4))
  ;; Where the next batch starts: the cookie of the last whole entry.
  (global $next i32 (i32.const 1032))  ;; shared: count_batch writes it; count_folder reads it
  (global $next.size i32 (i32.const 8))
  ;; The descriptor path_open gives a file.
  (global $opened i32 (i32.const 1040))
  (global $opened.size i32 (i32.const 4))
  ;; The line it prints: two counts of up to 20 digits, a space and a newline.
  (global $out i32 (i32.const 1048))  ;; shared: put_digits writes digits, print_counts the rest
  (global $out.size i32 (i32.const 42))
  ;; A file's name, copied from its entry to a start of its own.
  (global $name i32 (i32.const 1104))
  (global $name.size i32 (i32.const 256))
  ;; The iovec of a read of a file, the content's address and size, and the count fd_read writes.
  (global $iovec i32 (i32.const 1360))
  (global $iovec.size i32 (i32.const 8))
  (global $nread i32 (i32.const 1368))
  (global $nread.size i32 (i32.const 4))
  ;; One batch of the folder's entries.
  (global $entries i32 (i32.const 1408))  ;; shared: count_folder lists into it; the others read it
  (global $entries.size i32 (i32.const 65536))
  ;; A buffer-full of a file, read and not kept.
  (global $content i32 (i32.const 66944))
  (global $content.size i32 (i32.const 65536))

  ;; Counts the files and their bytes and prints both. Never returns: every path ends in
  ;; proc_exit.
  (func $main (export "_start")
    (local $status i32)
    (local $files i64)
    (local $bytes i64)
    (block $write_failed
      (block $failed
        (call $count_folder)
        (local.set $bytes)
        (local.set $files)
        (local.set $status)
        (br_if $failed (local.get $status))
        ;; A count never goes below zero.
        (if (i64.lt_s (local.get $files) (i64.const 0))
          (then (call $assert_fail (i32.const 1)) unreachable))
        (if (i64.lt_s (local.get $bytes) (i64.const 0))
          (then (call $assert_fail (i32.const 2)) unreachable))
        (br_if $write_failed (call $print_counts (local.get $files) (local.get $bytes)))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (call $proc_exit (global.get $EXIT_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Counts the files directly in the working folder and their bytes, a batch of its entries at a
  ;; time. Gives OK or FAILED, the files and the bytes.
  (func $count_folder (result i32 i64 i64)
    (local $err i32)
    (local $used i32)
    (local $status i32)
    (local $found i64)
    (local $held i64)
    (local $files i64)
    (local $bytes i64)
    (local $cookie i64)
    (local $result i32)
    (local $left i32)
    (local.set $result (global.get $FAILED))
    (block $failed
      (block $ended
        (local.set $left (global.get $MAX_BATCHES))
        (loop $batch  ;; bound: 512
          ;; Where a batch holds no whole entry, the next starts where this one does.
          (i64.store (global.get $next) (local.get $cookie))
          (local.set $err
            (call $fd_readdir (global.get $FOLDER) (global.get $entries)
              (global.get $entries.size) (local.get $cookie) (global.get $listed)))
          (br_if $failed (local.get $err))
          (local.set $used (i32.load (global.get $listed)))
          (if (i32.gt_u (local.get $used) (global.get $entries.size))
            (then (call $assert_fail (i32.const 3)) unreachable))
          (call $count_batch (local.get $used))
          (local.set $held)
          (local.set $found)
          (local.set $status)
          (br_if $failed (local.get $status))
          (local.set $files (i64.add (local.get $files) (local.get $found)))
          (local.set $bytes (i64.add (local.get $bytes) (local.get $held)))
          ;; A batch that doesn't fill the entries is the folder's last.
          (br_if $ended (i32.lt_u (local.get $used) (global.get $entries.size)))
          (local.set $cookie (i64.load (global.get $next)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $batch (local.get $left)))
        ;; A folder of more entries than 512 batches hold is more than the program takes on.
        (br $failed))
      (local.set $result (global.get $OK)))
    (if (i64.lt_s (local.get $files) (i64.const 0))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (local.get $result)
    (local.get $files)
    (local.get $bytes))

  ;; Counts the files that the first $used bytes of the entries name, and their bytes. Each whole
  ;; entry's cookie goes in $next; the last entry may be cut short, and the next batch starts with
  ;; it. Gives OK or FAILED, the files and the bytes.
  (func $count_batch (param $used i32) (result i32 i64 i64)
    (local $at i32)
    (local $length i32)
    (local $end i32)
    (local $size i64)
    (local $files i64)
    (local $bytes i64)
    (local $status i32)
    (local $left i32)
    (if (i32.gt_u (local.get $used) (global.get $entries.size))
      (then (call $assert_fail (i32.const 5)) unreachable))
    (local.set $status (global.get $FAILED))
    (block $failed
      (block $done
        (local.set $left (global.get $MAX_ENTRIES))
        (loop $entry  ;; bound: 2731
          ;; A whole dirent is left.
          (br_if $done (i32.gt_u (i32.add (local.get $at) (global.get $DIRENT)) (local.get $used)))
          (if (i32.gt_u (local.get $at) (global.get $LAST_DIRENT))
            (then (call $assert_fail (i32.const 6)) unreachable))
          (local.set $length (i32.load offset=16 (i32.add (global.get $entries) (local.get $at))))
          ;; A name longer than every system takes is a listing that went wrong.
          (br_if $failed (i32.gt_u (local.get $length) (global.get $NAME_MAX)))
          (local.set $end
            (i32.add (i32.add (local.get $at) (global.get $DIRENT)) (local.get $length)))
          ;; The entry is cut short: the next batch starts with it.
          (br_if $done (i32.gt_u (local.get $end) (local.get $used)))
          (if (i32.eq (i32.load8_u offset=20 (i32.add (global.get $entries) (local.get $at)))
                (global.get $REGULAR_FILE))
            (then
              (local.set $size
                (call $file_size (i32.add (local.get $at) (global.get $DIRENT))
                  (local.get $length)))
              (br_if $failed (i64.lt_s (local.get $size) (i64.const 0)))
              (local.set $files (i64.add (local.get $files) (i64.const 1)))
              (local.set $bytes (i64.add (local.get $bytes) (local.get $size)))))
          (i64.store (global.get $next)
            (i64.load (i32.add (global.get $entries) (local.get $at))))
          (local.set $at (local.get $end))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $entry (local.get $left)))
        ;; Each entry takes 24 bytes at least, so 2731 of them pass the entries' end.
        (call $assert_fail (i32.const 7))
        unreachable)
      (local.set $status (global.get $OK)))
    (local.get $status)
    (local.get $files)
    (local.get $bytes))

  ;; Reads the file whose name is the $length bytes of the entries from byte $from, to its end,
  ;; and closes it. Gives how many bytes it holds, or UNREADABLE when it can't be opened, read or
  ;; closed.
  (func $file_size (param $from i32) (param $length i32) (result i64)
    (local $k i32)
    (local $at i32)
    (local $err i32)
    (local $fd i32)
    (local $size i64)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $NAME_MAX))
      (then (call $assert_fail (i32.const 8)) unreachable))
    ;; The name, to a start of its own, as a path starts where its region does.
    (block $copied
      (local.set $left (global.get $NAME_MAX))
      (loop $byte  ;; bound: 255
        (br_if $copied (i32.ge_u (local.get $k) (local.get $length)))
        (local.set $at (i32.add (local.get $from) (local.get $k)))
        (if (i32.ge_u (local.get $at) (global.get $entries.size))
          (then (call $assert_fail (i32.const 9)) unreachable))
        (if (i32.ge_u (local.get $k) (global.get $name.size))
          (then (call $assert_fail (i32.const 10)) unreachable))
        (i32.store8 (i32.add (global.get $name) (local.get $k))
          (i32.load8_u (i32.add (global.get $entries) (local.get $at))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.set $size (global.get $UNREADABLE))
    (block $done
      (local.set $err
        (call $path_open (global.get $FOLDER) (global.get $NO_FLAGS) (global.get $name)
          (local.get $length) (global.get $NO_FLAGS) (global.get $READ_RIGHTS)
          (global.get $NO_RIGHTS) (global.get $NO_FLAGS) (global.get $opened)))
      (br_if $done (local.get $err))
      (local.set $fd (i32.load (global.get $opened)))
      ;; A file's descriptor comes after the working folder's.
      (if (i32.le_u (local.get $fd) (global.get $FOLDER))
        (then (call $assert_fail (i32.const 11)) unreachable))
      (local.set $size (call $read_all (local.get $fd)))
      (local.set $err (call $fd_close (local.get $fd)))
      (br_if $done (i32.eqz (local.get $err)))
      (local.set $size (global.get $UNREADABLE)))
    (local.get $size))

  ;; Reads the file open as $fd to its end, a buffer-full at a time, and gives how many bytes it
  ;; read, or UNREADABLE when a read fails. Each read asks for the whole buffer, as what the one
  ;; before gave isn't kept.
  (func $read_all (param $fd i32) (result i64)
    (local $err i32)
    (local $got i32)
    (local $total i64)
    (local $result i64)
    (local $left i32)
    (if (i32.le_u (local.get $fd) (global.get $FOLDER))
      (then (call $assert_fail (i32.const 12)) unreachable))
    (local.set $result (global.get $UNREADABLE))
    (block $failed
      (block $done
        (local.set $left (global.get $MAX_PIECES))
        (loop $piece  ;; bound: 2147483647
          (i32.store (global.get $iovec) (global.get $content))
          (i32.store offset=4 (global.get $iovec) (global.get $content.size))
          (local.set $err
            (call $fd_read (local.get $fd) (global.get $iovec) (global.get $ONE_BUFFER)
              (global.get $nread)))
          (br_if $failed (local.get $err))
          (local.set $got (i32.load (global.get $nread)))
          (if (i32.gt_u (local.get $got) (global.get $content.size))
            (then (call $assert_fail (i32.const 13)) unreachable))
          (local.set $total (i64.add (local.get $total) (i64.extend_i32_u (local.get $got))))
          ;; On a local disk a read of a file gives every byte it asks for up to the file's end:
          ;; one that doesn't fill the buffer has reached it.
          (br_if $done (i32.lt_u (local.get $got) (global.get $content.size)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $piece (local.get $left)))
        ;; 128 TiB is all it takes.
        (call $assert_fail (i32.const 14))
        unreachable)
      (local.set $result (local.get $total)))
    (local.get $result))

  ;; Writes $value's decimal digits into the output, the last just before byte $end, and gives
  ;; where the first is.
  (func $put_digits (param $value i64) (param $end i32) (result i32)
    (local $at i32)
    (local $rest i64)
    (local $left i32)
    (if (i32.gt_u (local.get $end) (global.get $out.size))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (local.set $at (local.get $end))
    (local.set $rest (local.get $value))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 16)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; A 64-bit number has at most 20 digits.
      (call $assert_fail (i32.const 17))
      unreachable)
    (local.get $at))

  ;; Prints $files, a space, $bytes and a newline to standard output: the newline last, the bytes'
  ;; digits before it, then the space and the files' digits. Gives 0, or an error number.
  (func $print_counts (param $files i64) (param $bytes i64) (result i32)  ;; returns: result
    (local $at i32)
    (i32.store8 (i32.add (global.get $out) (global.get $NEWLINE_AT)) (global.get $NEWLINE))
    (local.set $at (call $put_digits (local.get $bytes) (global.get $NEWLINE_AT)))
    ;; Room before the bytes' digits for the space and the files' 20 digits at most.
    (if (i32.lt_u (local.get $at) (global.get $FIRST_ROOM))
      (then (call $assert_fail (i32.const 18)) unreachable))
    (local.set $at (i32.sub (local.get $at) (i32.const 1)))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $SPACE))
    (local.set $at (call $put_digits (local.get $files) (local.get $at)))
    (if (i32.ge_u (local.get $at) (global.get $out.size))
      (then (call $assert_fail (i32.const 19)) unreachable))
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
