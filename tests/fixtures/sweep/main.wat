;; sweep: deletes the empty files in the current folder and says how many it deleted. The
;; WebAssembly port of the native sweep, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $write_all. It lists the working
;; folder a batch of entries at a time with fd_readdir, each batch from the cookie of the last
;; whole entry before it; it reads each file's size with path_filestat_get, and deletes an empty
;; one with path_unlink_file. lengua's host lists a folder as it was when the listing began, so a
;; file deleted on the way changes no later batch.
(module
  (import "wasi_snapshot_preview1" "fd_readdir"
    (func $fd_readdir (param i32 i32 i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_filestat_get"
    (func $path_filestat_get (param i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_unlink_file"
    (func $path_unlink_file (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors: standard output, and the working folder, which lengua opens as 3.
  (global $STDOUT i32 (i32.const 1))
  (global $FOLDER i32 (i32.const 3))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_FAILED i32 (i32.const 3))
  ;; What delete_if_empty gives, and what it and the sweep give when a call fails.
  (global $KEPT i32 (i32.const 0))
  (global $DELETED i32 (i32.const 1))
  (global $FAILED i32 (i32.const -1))
  ;; path_filestat_get's flags: none, so a link at the end of a name isn't followed.
  (global $NO_FLAGS i32 (i32.const 0))
  ;; A folder's entries, as fd_readdir writes them: each a dirent of 24 bytes, the next entry's
  ;; cookie at 0, the name's length at 16 and its kind at 20, then the name. A file is kind 4.
  ;; LAST_DIRENT is the last place a whole dirent fits in the entries.
  (global $DIRENT i32 (i32.const 24))
  (global $LAST_DIRENT i32 (i32.const 4072))
  (global $REGULAR_FILE i32 (i32.const 4))
  ;; Sizes and limits: the batches it takes at most, the entries a batch holds at most, each 24
  ;; bytes at least, the files it deletes at most, and the longest name every system takes.
  (global $MAX_BATCHES i32 (i32.const 64))
  (global $MAX_ENTRIES i32 (i32.const 171))
  (global $MAX_DELETED i32 (i32.const 10944))
  (global $NAME_MAX i32 (i32.const 255))
  ;; Printing: a count of up to 20 digits, then a newline.
  (global $MAX_DIGITS i32 (i32.const 20))
  (global $NEWLINE_AT i32 (i32.const 20))
  (global $NEWLINE i32 (i32.const 10))
  (global $ZERO i32 (i32.const 48))
  (global $TEN i64 (i64.const 10))

  ;; Regions (rule 3, M1), from 1024 up: the message, read-only, then what the host writes, the
  ;; line it prints, a file's name and a batch of entries.
  (global $deleted_message i32 (i32.const 1024))
  (global $deleted_message.rosize i32 (i32.const 8))
  (data (i32.const 1024) "deleted ")
  ;; How many bytes of entries fd_readdir wrote.
  (global $listed i32 (i32.const 1040))
  (global $listed.size i32 (i32.const 4))
  ;; Where the next batch starts: the cookie of the last whole entry.
  (global $next i32 (i32.const 1048))  ;; shared: sweep_batch writes it; sweep_folder reads it
  (global $next.size i32 (i32.const 8))
  ;; A file's details, as path_filestat_get writes them: its size at 32.
  (global $stat i32 (i32.const 1056))
  (global $stat.size i32 (i32.const 64))
  ;; The line it prints, after its message.
  (global $out i32 (i32.const 1120))
  (global $out.size i32 (i32.const 21))
  ;; A file's name, copied from its entry to a start of its own.
  (global $name i32 (i32.const 1152))
  (global $name.size i32 (i32.const 256))
  ;; One batch of the folder's entries.
  (global $entries i32 (i32.const 1408))  ;; shared: sweep_folder lists into it; the others read it
  (global $entries.size i32 (i32.const 4096))

  ;; Deletes the empty files in the working folder and says how many. Never returns: every path
  ;; ends in proc_exit.
  (func $main (export "_start")
    (local $deleted i32)
    (block $failed
      (local.set $deleted (call $sweep_folder))
      (br_if $failed (i32.lt_s (local.get $deleted) (i32.const 0)))
      (if (i32.gt_u (local.get $deleted) (global.get $MAX_DELETED))
        (then (call $assert_fail (i32.const 1)) unreachable))
      (br_if $failed
        (call $write_all (global.get $STDOUT) (global.get $deleted_message)
          (global.get $deleted_message.rosize)))
      (br_if $failed (call $write_count (i64.extend_i32_u (local.get $deleted))))
      (call $proc_exit (global.get $EXIT_OK))
      unreachable)
    (call $proc_exit (global.get $EXIT_FAILED))
    unreachable)

  ;; Deletes the empty files directly in the working folder, a batch of its entries at a time,
  ;; and gives how many it deleted, or FAILED when it can't list, examine or delete them.
  (func $sweep_folder (result i32)
    (local $err i32)
    (local $used i32)
    (local $found i32)
    (local $deleted i32)
    (local $result i32)
    (local $cookie i64)
    (local $left i32)
    (local.set $result (global.get $FAILED))
    (block $failed
      (block $ended
        (local.set $left (global.get $MAX_BATCHES))
        (loop $batch  ;; bound: 64
          ;; Where a batch holds no whole entry, the next starts where this one does.
          (i64.store (global.get $next) (local.get $cookie))
          (local.set $err
            (call $fd_readdir (global.get $FOLDER) (global.get $entries)
              (global.get $entries.size) (local.get $cookie) (global.get $listed)))
          (br_if $failed (local.get $err))
          (local.set $used (i32.load (global.get $listed)))
          (if (i32.gt_u (local.get $used) (global.get $entries.size))
            (then (call $assert_fail (i32.const 2)) unreachable))
          (local.set $found (call $sweep_batch (local.get $used)))
          (br_if $failed (i32.lt_s (local.get $found) (i32.const 0)))
          (if (i32.gt_u (local.get $found) (global.get $MAX_ENTRIES))
            (then (call $assert_fail (i32.const 3)) unreachable))
          (local.set $deleted (i32.add (local.get $deleted) (local.get $found)))
          ;; A batch that doesn't fill the entries is the folder's last.
          (br_if $ended (i32.lt_u (local.get $used) (global.get $entries.size)))
          (local.set $cookie (i64.load (global.get $next)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $batch (local.get $left)))
        ;; A folder of more entries than 64 batches hold is more than the sweep takes on.
        (br $failed))
      (local.set $result (local.get $deleted)))
    (local.get $result))

  ;; Deletes the empty files that the first $used bytes of the entries name, and gives how many
  ;; it deleted, or FAILED. Each whole entry's cookie goes in $next; the last entry may be cut
  ;; short, and the next batch starts with it.
  (func $sweep_batch (param $used i32) (result i32)
    (local $at i32)
    (local $length i32)
    (local $end i32)
    (local $found i32)
    (local $deleted i32)
    (local $result i32)
    (local $left i32)
    (if (i32.gt_u (local.get $used) (global.get $entries.size))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (local.set $result (global.get $FAILED))
    (block $failed
      (block $done
        (local.set $left (global.get $MAX_ENTRIES))
        (loop $entry  ;; bound: 171
          ;; A whole dirent is left.
          (br_if $done (i32.gt_u (i32.add (local.get $at) (global.get $DIRENT)) (local.get $used)))
          (if (i32.gt_u (local.get $at) (global.get $LAST_DIRENT))
            (then (call $assert_fail (i32.const 5)) unreachable))
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
              (local.set $found
                (call $delete_if_empty (i32.add (local.get $at) (global.get $DIRENT))
                  (local.get $length)))
              (br_if $failed (i32.lt_s (local.get $found) (i32.const 0)))
              (local.set $deleted (i32.add (local.get $deleted) (local.get $found)))))
          (i64.store (global.get $next)
            (i64.load (i32.add (global.get $entries) (local.get $at))))
          (local.set $at (local.get $end))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $entry (local.get $left)))
        ;; Each entry takes 24 bytes at least, so 171 of them pass the entries' end.
        (call $assert_fail (i32.const 6))
        unreachable)
      (local.set $result (local.get $deleted)))
    (local.get $result))

  ;; Deletes the file whose name is the $length bytes of the entries from byte $from, when the
  ;; file is empty. Gives DELETED when it deleted the file, KEPT when it kept it, or FAILED.
  (func $delete_if_empty (param $from i32) (param $length i32) (result i32)
    (local $k i32)
    (local $at i32)
    (local $err i32)
    (local $result i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $NAME_MAX))
      (then (call $assert_fail (i32.const 7)) unreachable))
    ;; The name, to a start of its own, as a path starts where its region does.
    (block $copied
      (local.set $left (global.get $NAME_MAX))
      (loop $byte  ;; bound: 255
        (br_if $copied (i32.ge_u (local.get $k) (local.get $length)))
        (local.set $at (i32.add (local.get $from) (local.get $k)))
        (if (i32.ge_u (local.get $at) (global.get $entries.size))
          (then (call $assert_fail (i32.const 8)) unreachable))
        (if (i32.ge_u (local.get $k) (global.get $name.size))
          (then (call $assert_fail (i32.const 9)) unreachable))
        (i32.store8 (i32.add (global.get $name) (local.get $k))
          (i32.load8_u (i32.add (global.get $entries) (local.get $at))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.set $result (global.get $FAILED))
    (block $done
      (local.set $err
        (call $path_filestat_get (global.get $FOLDER) (global.get $NO_FLAGS) (global.get $name)
          (local.get $length) (global.get $stat)))
      (br_if $done (local.get $err))
      (local.set $result (global.get $KEPT))
      ;; A file with contents stays.
      (br_if $done (i64.ne (i64.load offset=32 (global.get $stat)) (i64.const 0)))
      (local.set $result (global.get $FAILED))
      (local.set $err
        (call $path_unlink_file (global.get $FOLDER) (global.get $name) (local.get $length)))
      (br_if $done (local.get $err))
      (local.set $result (global.get $DELETED)))
    (local.get $result))

  ;; Writes $count in decimal, then a newline, to standard output: the newline last, the digits
  ;; before it from the last. Gives 0, or an error number.
  (func $write_count (param $count i64) (result i32)  ;; returns: result
    (local $rest i64)
    (local $at i32)
    (local $left i32)
    (local.set $rest (local.get $count))
    (local.set $at (global.get $NEWLINE_AT))
    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
    (block $written
      (local.set $left (global.get $MAX_DIGITS))
      (loop $digit  ;; bound: 20
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 10)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.add (global.get $ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $rest) (global.get $TEN)))))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $TEN)))
        (br_if $written (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; 20 digits are all a 64-bit count has.
      (call $assert_fail (i32.const 11))
      unreachable)
    (call $write_all (global.get $STDOUT) (i32.add (global.get $out) (local.get $at))
      (i32.sub (global.get $out.size) (local.get $at))))
)
