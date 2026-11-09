;; greet: greets the person that the NAME environment variable names, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md), in the layout of the compile skill's
;; main-template.wat: every rule passes, and lengua's library gives $assert_fail and $write_all.
;; It looks for NAME among up to 256 variables, 128 KiB of text in all, and greets a name of 1 to
;; 64 characters. environ_get writes every variable, so a larger environment gets the usage line,
;; where the native greet looked for NAME among the first 256 of any number of variables. Each
;; variable ends where the next one starts, so no byte of a name is compared to find its end.
(module
  (import "wasi_snapshot_preview1" "environ_sizes_get"
    (func $environ_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "environ_get"
    (func $environ_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 3 3)

  ;; Descriptors.
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_WRITE_FAILED i32 (i32.const 3))
  ;; Sizes and limits: up to 256 variables, and a name of up to 64 characters.
  (global $MAX_VARIABLES i32 (i32.const 256))
  (global $MAX_NAME i32 (i32.const 64))
  (global $KEY_LENGTH i32 (i32.const 5))
  (global $POINTER i32 (i32.const 4))
  (global $NUL_LENGTH i32 (i32.const 1))

  ;; Regions (rule 3, M1), from 1024 up: the messages and the key, read-only, then what the host
  ;; writes and the greeting.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 24))
  (data (i32.const 1024) "usage: NAME=<who> greet\0a")
  (global $key i32 (i32.const 1056))
  (global $key.rosize i32 (i32.const 5))
  (data (i32.const 1056) "NAME=")
  (global $hello i32 (i32.const 1072))
  (global $hello.rosize i32 (i32.const 7))
  (data (i32.const 1072) "Hello, ")
  (global $bang i32 (i32.const 1088))
  (global $bang.rosize i32 (i32.const 2))
  (data (i32.const 1088) "!\0a")
  ;; The variables' count and the size of their text, as environ_sizes_get writes them.
  (global $sizes i32 (i32.const 2048))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each variable starts, as environ_get writes it: 4 bytes for each of up to 256.
  (global $environ i32 (i32.const 2064))
  (global $environ.size i32 (i32.const 1024))
  ;; The greeting: "Hello, ", a name of up to 64 bytes, and "!" with a newline.
  (global $out i32 (i32.const 3136))
  (global $out.size i32 (i32.const 73))
  ;; The variables' text, each NAME=value ended by a NUL. A program gets only the variables its
  ;; test or its run names, so 128 KiB holds them with room to spare.
  (global $environment i32 (i32.const 4096))  ;; shared: main has the host fill it; others read it
  (global $environment.size i32 (i32.const 131072))

  ;; Finds NAME among the variables and greets the person it names, or prints the usage line.
  ;; Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $count i32)
    (local $size i32)
    (local $index i32)
    (local $start i32)
    (local $next i32)
    (local $end i32)
    (local $length i32)
    (local $left i32)
    (block $write_failed
      (block $usage
        (local.set $err
          (call $environ_sizes_get (global.get $sizes) (i32.add (global.get $sizes) (i32.const 4))))
        (if (local.get $err)
          (then (call $assert_fail (i32.const 1)) unreachable))
        (local.set $count (i32.load (global.get $sizes)))
        (local.set $size (i32.load offset=4 (global.get $sizes)))
        ;; More variables or more text than the regions hold has no name to greet.
        (br_if $usage (i32.gt_u (local.get $count) (global.get $MAX_VARIABLES)))
        (br_if $usage (i32.gt_u (local.get $size) (global.get $environment.size)))
        (local.set $err (call $environ_get (global.get $environ) (global.get $environment)))
        (if (local.get $err)
          (then (call $assert_fail (i32.const 2)) unreachable))
        ;; The variable that starts with NAME=, its start as an offset into $environment.
        (block $found
          (local.set $left (global.get $MAX_VARIABLES))
          (loop $variable  ;; bound: 256
            (br_if $usage (i32.ge_u (local.get $index) (local.get $count)))
            (local.set $start
              (i32.sub
                (i32.load
                  (i32.add (global.get $environ)
                    (i32.mul (local.get $index) (global.get $POINTER))))
                (global.get $environment)))
            ;; environ_get points each variable at its place in the text it wrote.
            (if (i32.ge_u (local.get $start) (local.get $size))
              (then (call $assert_fail (i32.const 3)) unreachable))
            (br_if $found (call $is_key (local.get $start) (local.get $size)))
            (local.set $index (i32.add (local.get $index) (i32.const 1)))
            (local.set $left (i32.sub (local.get $left) (i32.const 1)))
            (br_if $variable (local.get $left)))
          (br $usage))
        ;; The variable ends where the next one starts, or where the text does.
        (local.set $end (local.get $size))
        (local.set $next (i32.add (local.get $index) (i32.const 1)))
        (if (i32.lt_u (local.get $next) (local.get $count))
          (then
            (local.set $end
              (i32.sub
                (i32.load
                  (i32.add (global.get $environ)
                    (i32.mul (local.get $next) (global.get $POINTER))))
                (global.get $environment)))))
        ;; The name: the variable less its key and its NUL. An empty name, or one of more than
        ;; 64 characters, gets the usage line.
        (local.set $length
          (i32.sub (i32.sub (local.get $end) (local.get $start))
            (i32.add (global.get $KEY_LENGTH) (global.get $NUL_LENGTH))))
        (br_if $usage (i32.eqz (local.get $length)))
        (br_if $usage (i32.gt_u (local.get $length) (global.get $MAX_NAME)))
        (br_if $write_failed
          (call $write_greeting (i32.add (local.get $start) (global.get $KEY_LENGTH))
            (local.get $length)))
        (call $proc_exit (global.get $EXIT_OK))
        unreachable)
      (br_if $write_failed
        (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
      (call $proc_exit (global.get $EXIT_USAGE))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Whether the variable at byte $start of $environment, whose text is $size bytes, starts with
  ;; NAME=: 1 if it does, else 0.
  (func $is_key (param $start i32) (param $size i32) (result i32)
    (local $i i32)
    (local $at i32)
    (local $same i32)
    (local $left i32)
    (local.set $same (i32.const 1))
    ;; A variable too near the text's end to hold NAME= isn't it.
    (if (i32.gt_u (i32.add (local.get $start) (global.get $KEY_LENGTH)) (local.get $size))
      (then (local.set $same (i32.const 0))))
    (block $compared
      (br_if $compared (i32.eqz (local.get $same)))
      (local.set $left (global.get $KEY_LENGTH))
      (loop $byte  ;; bound: 5
        ;; Byte $i of the variable, at $at of the text, and of the key: each place checked.
        (local.set $at (i32.add (local.get $start) (local.get $i)))
        (if (i32.ge_u (local.get $at) (global.get $environment.size))
          (then (call $assert_fail (i32.const 4)) unreachable))
        (if (i32.ge_u (local.get $i) (global.get $key.rosize))
          (then (call $assert_fail (i32.const 5)) unreachable))
        (if (i32.ne
              (i32.load8_u (i32.add (global.get $environment) (local.get $at)))
              (i32.load8_u (i32.add (global.get $key) (local.get $i))))
          (then (local.set $same (i32.const 0))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $same))

  ;; Writes "Hello, ", the $length bytes of the name at byte $from of $environment, one at a time,
  ;; each place checked, and "!" with a newline to standard output: main gives where the name
  ;; starts. Gives 0, or an error number.
  (func $write_greeting (param $from i32) (param $length i32) (result i32)  ;; returns: result
    (local $k i32)
    (local $at i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $MAX_NAME))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (memory.copy (global.get $out) (global.get $hello) (global.get $hello.rosize))
    (block $copied
      (local.set $left (global.get $MAX_NAME))
      (loop $byte  ;; bound: 64
        (br_if $copied (i32.ge_u (local.get $k) (local.get $length)))
        (local.set $at (i32.add (local.get $from) (local.get $k)))
        (if (i32.ge_u (local.get $at) (global.get $environment.size))
          (then (call $assert_fail (i32.const 7)) unreachable))
        (i32.store8 (i32.add (i32.add (global.get $out) (global.get $hello.rosize)) (local.get $k))
          (i32.load8_u (i32.add (global.get $environment) (local.get $at))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (memory.copy
      (i32.add (i32.add (global.get $out) (global.get $hello.rosize)) (local.get $length))
      (global.get $bang)
      (global.get $bang.rosize))
    (call $write_all (global.get $STDOUT) (global.get $out)
      (i32.add (i32.add (global.get $hello.rosize) (local.get $length)) (global.get $bang.rosize))))
)
