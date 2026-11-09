;; archive: moves a file into the archive folder and makes the folder if it does not exist. The
;; WebAssembly twin of examples/archive, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $write_all. It copies the file's
;; name from its argument to a region of its own, so each path starts where its region does; it
;; looks for the file with path_filestat_get before it makes the folder, so a name with no file
;; makes nothing; and it moves the file with path_rename.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_filestat_get"
    (func $path_filestat_get (param i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_create_directory"
    (func $path_create_directory (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_rename"
    (func $path_rename (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors: standard output and error, and the working folder, which lengua opens as 3.
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  (global $FOLDER i32 (i32.const 3))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_MISSING i32 (i32.const 2))
  (global $EXIT_FAILED i32 (i32.const 3))
  ;; Error numbers, as references/host.md names them.
  (global $NO_ERROR i32 (i32.const 0))
  (global $ERROR_EXIST i32 (i32.const 20))
  (global $ERROR_NOENT i32 (i32.const 44))
  ;; path_filestat_get's flags: none.
  (global $NO_FLAGS i32 (i32.const 0))
  ;; Sizes and limits: a name of 1 to NAME_MAX bytes, read from NAME_SCAN bytes at most.
  (global $ARGC_EXPECTED i32 (i32.const 2))
  (global $NAME_MAX i32 (i32.const 64))
  (global $NAME_SCAN i32 (i32.const 65))
  ;; Characters.
  (global $SLASH i32 (i32.const 47))

  ;; Regions (rule 3, M1), from 1024 up: the messages and the folder's names, read-only, then
  ;; what the host writes, the file's name and where it goes, and the arguments' text.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 22))
  (data (i32.const 1024) "usage: archive <file>\0a")
  (global $archived i32 (i32.const 1056))
  (global $archived.rosize i32 (i32.const 9))
  (data (i32.const 1056) "archived ")
  (global $missing i32 (i32.const 1072))
  (global $missing.rosize i32 (i32.const 17))
  (data (i32.const 1072) "archive: no file ")
  (global $newline i32 (i32.const 1096))
  (global $newline.rosize i32 (i32.const 1))
  (data (i32.const 1096) "\0a")
  (global $folder i32 (i32.const 1104))
  (global $folder.rosize i32 (i32.const 7))
  (data (i32.const 1104) "archive")
  (global $prefix i32 (i32.const 1112))
  (global $prefix.rosize i32 (i32.const 8))
  (data (i32.const 1112) "archive/")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 1120))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then the file's.
  (global $argv i32 (i32.const 1128))
  (global $argv.size i32 (i32.const 8))
  ;; The file's details, as path_filestat_get writes them, which only show that it is there.
  (global $stat i32 (i32.const 1136))
  (global $stat.size i32 (i32.const 64))
  ;; The file's name, 1 to NAME_MAX bytes.
  (global $name i32 (i32.const 1216))  ;; shared: copy_name writes it; the others read it
  (global $name.size i32 (i32.const 64))
  ;; Where the file goes: `archive/` and its name.
  (global $target i32 (i32.const 1280))
  (global $target.size i32 (i32.const 72))
  ;; The arguments' text, each ended by a NUL: 32 KiB, more than a test's command line takes.
  (global $args i32 (i32.const 1408))  ;; shared: main has the host fill it; copy_name reads it
  (global $args.size i32 (i32.const 32768))

  ;; Moves the file that the argument names into the folder `archive`, and says so, or says why
  ;; it can't. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $len i32)
    (block $failed
      (block $missing_file
        (block $usage
          (local.set $err
            (call $args_sizes_get (global.get $sizes)
              (i32.add (global.get $sizes) (i32.const 4))))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 1)) unreachable))
          (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
          ;; Text longer than $args holds is longer than a name.
          (br_if $usage
            (i32.gt_u (i32.load offset=4 (global.get $sizes)) (global.get $args.size)))
          (local.set $err (call $args_get (global.get $argv) (global.get $args)))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 2)) unreachable))
          ;; The name starts where argv[1] points: an offset into $args, which copy_name checks.
          (local.set $len
            (call $copy_name (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args))))
          (br_if $usage (i32.eqz (local.get $len)))
          (if (i32.gt_u (local.get $len) (global.get $NAME_MAX))
            (then (call $assert_fail (i32.const 3)) unreachable))
          (local.set $err (call $archive_file (local.get $len)))
          (br_if $missing_file (i32.eq (local.get $err) (global.get $ERROR_NOENT)))
          (br_if $failed (local.get $err))
          (br_if $failed
            (call $write_all (global.get $STDOUT) (global.get $archived)
              (global.get $archived.rosize)))
          (br_if $failed (call $say_name (global.get $STDOUT) (local.get $len)))
          (call $proc_exit (global.get $EXIT_OK))
          unreachable)
        (br_if $failed
          (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
        (call $proc_exit (global.get $EXIT_USAGE))
        unreachable)
      (br_if $failed
        (call $write_all (global.get $STDERR) (global.get $missing) (global.get $missing.rosize)))
      (br_if $failed (call $say_name (global.get $STDERR) (local.get $len)))
      (call $proc_exit (global.get $EXIT_MISSING))
      unreachable)
    (call $proc_exit (global.get $EXIT_FAILED))
    unreachable)

  ;; Copies the argument that starts at byte $at of $args, up to its NUL, to $name, and gives its
  ;; length, 1 to NAME_MAX; or 0, when it isn't a name the program takes: an empty one, a longer
  ;; one, or one with a slash.
  (func $copy_name (param $at i32) (result i32)
    (local $k i32)
    (local $from i32)
    (local $c i32)
    (local $length i32)
    (local $left i32)
    (block $invalid
      (block $ended
        (local.set $left (global.get $NAME_SCAN))
        (loop $byte  ;; bound: 65
          (local.set $from (i32.add (local.get $at) (local.get $k)))
          (if (i32.ge_u (local.get $from) (global.get $args.size))
            (then (call $assert_fail (i32.const 4)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $from))))
          (br_if $ended (i32.eqz (local.get $c)))
          (br_if $invalid (i32.eq (local.get $c) (global.get $SLASH)))
          ;; One byte more than a name has.
          (br_if $invalid (i32.ge_u (local.get $k) (global.get $name.size)))
          (i32.store8 (i32.add (global.get $name) (local.get $k)) (local.get $c))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $byte (local.get $left)))
        ;; The name's NUL, or its byte past NAME_MAX, comes within NAME_SCAN bytes.
        (call $assert_fail (i32.const 5))
        unreachable)
      (local.set $length (local.get $k)))
    (local.get $length))

  ;; Moves the file that the $length bytes of $name name into the folder `archive`, and makes the
  ;; folder first when it isn't there. Gives 0, NOENT when there is no such file, or another
  ;; error number.
  (func $archive_file (param $length i32) (result i32)  ;; returns: result
    (local $err i32)
    (if (i32.eqz (local.get $length))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (if (i32.gt_u (local.get $length) (global.get $NAME_MAX))
      (then (call $assert_fail (i32.const 7)) unreachable))
    (block $done
      ;; The file first, so a name with no file makes no folder.
      (local.set $err
        (call $path_filestat_get (global.get $FOLDER) (global.get $NO_FLAGS) (global.get $name)
          (local.get $length) (global.get $stat)))
      (br_if $done (local.get $err))
      (local.set $err
        (call $path_create_directory (global.get $FOLDER) (global.get $folder)
          (global.get $folder.rosize)))
      ;; A folder that is there already is the archive.
      (if (i32.eq (local.get $err) (global.get $ERROR_EXIST))
        (then (local.set $err (global.get $NO_ERROR))))
      (br_if $done (local.get $err))
      ;; Where the file goes: `archive/` and its name.
      (memory.copy (global.get $target) (global.get $prefix) (global.get $prefix.rosize))
      (memory.copy (i32.add (global.get $target) (global.get $prefix.rosize)) (global.get $name)
        (local.get $length))
      (local.set $err
        (call $path_rename (global.get $FOLDER) (global.get $name) (local.get $length)
          (global.get $FOLDER) (global.get $target)
          (i32.add (global.get $prefix.rosize) (local.get $length))))
      (br_if $done (local.get $err)))
    (local.get $err))

  ;; Writes the $length bytes of $name, then a newline, to $fd. Gives 0, or the error of the write
  ;; that failed.
  (func $say_name (param $fd i32) (param $length i32) (result i32)  ;; returns: result
    (local $err i32)
    (if (i32.eqz (local.get $length))
      (then (call $assert_fail (i32.const 8)) unreachable))
    (if (i32.gt_u (local.get $length) (global.get $NAME_MAX))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (block $failed
      (local.set $err (call $write_all (local.get $fd) (global.get $name) (local.get $length)))
      (br_if $failed (local.get $err))
      (local.set $err
        (call $write_all (local.get $fd) (global.get $newline) (global.get $newline.rosize)))
      (br_if $failed (local.get $err)))
    (local.get $err))
)
