;; A stand-in for a solver: a WebAssembly module with exceptions, written by hand for the tests of
;; a WebAssembly solver's host (`lengua-mcp --solver-wasm`, src/wasm/solver.rs), which compile it
;; with wast. No program's engine takes it: its tag and its throws are exceptions, which only the
;; solver's engine turns on. What it does depends on its first argument:
;;
;; - `catch`: throws 42 and catches it, prints `caught 42`, and exits 0;
;; - `throw`: prints `throwing 7`, then throws 7, which nothing catches;
;; - `grants`: prints what the host gives it, a line each: how many arguments it has, its name
;;   with them; how many bytes its standard input holds; the time of day's answer, and 1 when a
;;   time came; how many environment variables it has; random bytes' answer and the first byte;
;;   and the answers to opening a file, to the first folder a program may be given, and to taking
;;   a connection where a server's listener would be. Then it writes a line to standard error;
;; - `grow`: grows its memory by 64 pages, 4 MiB, and exits 0 when it grew and 1 when the host
;;   refused;
;; - `--version`: prints a version line, as z3 does;
;; - `-in`, z3's first argument as the prover starts it: reads the prover's commands to the end of
;;   its input and answers each line as z3 does, `sat` to every check, an empty list to every
;;   request for values, a reason to a request for why, and the line the prover's `echo` asks for;
;; - anything else: exits 2.
;;
;; Its memory: 0-11 a buffer's place and length for the host and the count it gives back; 16-23
;; the arguments' count and size; 24-31 the environment's; 32-39 a time; 40-43 random bytes; 44-47
;; a descriptor; 48-63 a number written out; 256-511 the arguments' places; 512-4095 their text;
;; 4096-4607 the words it prints and takes; 16384-32767 standard input as it comes; 32768-32783
;; the start of a command's line.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get" (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "environ_sizes_get"
    (func $environ_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "clock_time_get"
    (func $clock_time_get (param i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "random_get" (func $random_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_read" (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_prestat_get"
    (func $fd_prestat_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_accept"
    (func $sock_accept (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))

  (memory (export "memory") 1)

  ;; What `catch` and `throw` throw: a number.
  (tag $boom (param i32))

  ;; The words it prints, each at its place, with its length after it.
  (data (i32.const 4096) "caught ")                  ;; 7
  (data (i32.const 4112) "throwing ")                ;; 9
  (data (i32.const 4128) "arguments ")               ;; 10
  (data (i32.const 4144) "input ")                   ;; 6
  (data (i32.const 4160) "clock ")                   ;; 6
  (data (i32.const 4176) "environment ")             ;; 12
  (data (i32.const 4192) "random ")                  ;; 7
  (data (i32.const 4208) "files ")                   ;; 6
  (data (i32.const 4224) "folder ")                  ;; 7
  (data (i32.const 4240) "network ")                 ;; 8
  (data (i32.const 4256) "to standard error\n")      ;; 18
  (data (i32.const 4288) "Z3 version 4.16.0 - a stand-in in WebAssembly\n") ;; 46
  (data (i32.const 4352) "@@lengua@@\n")             ;; 11
  (data (i32.const 4368) "sat\n")                    ;; 4
  (data (i32.const 4384) "()\n")                     ;; 3
  (data (i32.const 4400) "(:reason-unknown \"a stand-in\")\n") ;; 31
  ;; The words it takes: its commands, and the starts of the prover's lines.
  (data (i32.const 4448) "catch")                    ;; 5
  (data (i32.const 4456) "throw")                    ;; 5
  (data (i32.const 4464) "grants")                   ;; 6
  (data (i32.const 4472) "grow")                     ;; 4
  (data (i32.const 4480) "--version")                ;; 9
  (data (i32.const 4496) "-in")                      ;; 3
  (data (i32.const 4504) "(echo")                    ;; 5
  (data (i32.const 4512) "(check-sat")               ;; 10
  (data (i32.const 4528) "(get-value")               ;; 10
  (data (i32.const 4544) "(get-info")                ;; 9
  (data (i32.const 4560) "in.txt")                   ;; 6
  (data (i32.const 4568) "\n")                       ;; 1
  (data (i32.const 4569) " ")                        ;; 1

  ;; Writes the `len` bytes at `at` to the descriptor `fd`.
  (func $write (param $fd i32) (param $at i32) (param $len i32)
    (i32.store (i32.const 0) (local.get $at))
    (i32.store (i32.const 4) (local.get $len))
    (drop (call $fd_write (local.get $fd) (i32.const 0) (i32.const 1) (i32.const 8))))

  ;; Prints the `len` bytes at `at`.
  (func $print (param $at i32) (param $len i32)
    (call $write (i32.const 1) (local.get $at) (local.get $len)))

  ;; Prints `value` in decimal.
  (func $number (param $value i32)
    (local $at i32)
    (local.set $at (i32.const 64))
    (loop $digits
      (local.set $at (i32.sub (local.get $at) (i32.const 1)))
      (i32.store8 (local.get $at)
        (i32.add (i32.const 48) (i32.rem_u (local.get $value) (i32.const 10))))
      (local.set $value (i32.div_u (local.get $value) (i32.const 10)))
      (br_if $digits (local.get $value)))
    (call $print (local.get $at) (i32.sub (i32.const 64) (local.get $at))))

  ;; Prints the `len` bytes at `at`, then `value`, then the end of the line.
  (func $line (param $at i32) (param $len i32) (param $value i32)
    (call $print (local.get $at) (local.get $len))
    (call $number (local.get $value))
    (call $print (i32.const 4568) (i32.const 1)))

  ;; Prints the `len` bytes at `at`, then `first`, a space, `second` and the end of the line.
  (func $pair (param $at i32) (param $len i32) (param $first i32) (param $second i32)
    (call $print (local.get $at) (local.get $len))
    (call $number (local.get $first))
    (call $print (i32.const 4569) (i32.const 1))
    (call $number (local.get $second))
    (call $print (i32.const 4568) (i32.const 1)))

  ;; Whether the first argument after its name is the `len` bytes at `at`.
  (func $first_is (param $at i32) (param $len i32) (result i32)
    (local $argument i32)
    (local $i i32)
    (if (i32.lt_u (i32.load (i32.const 16)) (i32.const 2))
      (then (return (i32.const 0))))
    (local.set $argument (i32.load (i32.const 260)))
    (block $same
      (loop $next
        (br_if $same (i32.eq (local.get $i) (local.get $len)))
        (if (i32.ne
              (i32.load8_u (i32.add (local.get $argument) (local.get $i)))
              (i32.load8_u (i32.add (local.get $at) (local.get $i))))
          (then (return (i32.const 0))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.eqz (i32.load8_u (i32.add (local.get $argument) (local.get $len)))))

  ;; Reads standard input into 16384, as much as comes in one read: how many bytes came, or 0 at
  ;; its end or when the read fails.
  (func $read (result i32)
    (i32.store (i32.const 0) (i32.const 16384))
    (i32.store (i32.const 4) (i32.const 16384))
    (if (call $fd_read (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 8))
      (then (return (i32.const 0))))
    (i32.load (i32.const 8)))

  ;; Throws `value`.
  (func $throw_it (param $value i32)
    (throw $boom (local.get $value)))

  ;; `catch`: throws 42 and catches it.
  (func $catch
    (local $caught i32)
    (local.set $caught
      (block $handler (result i32)
        (try_table (catch $boom $handler)
          (call $throw_it (i32.const 42)))
        (i32.const 0)))
    (call $line (i32.const 4096) (i32.const 7) (local.get $caught)))

  ;; `throw`: says so, then throws 7, which nothing catches.
  (func $uncaught
    (call $line (i32.const 4112) (i32.const 9) (i32.const 7))
    (call $throw_it (i32.const 7)))

  ;; `grants`: what the host gives a solver, a line for each.
  (func $grants
    (local $total i32)
    (local $read i32)
    (call $line (i32.const 4128) (i32.const 10) (i32.load (i32.const 16)))
    (block $end
      (loop $more
        (local.set $read (call $read))
        (br_if $end (i32.eqz (local.get $read)))
        (local.set $total (i32.add (local.get $total) (local.get $read)))
        (br $more)))
    (call $line (i32.const 4144) (i32.const 6) (local.get $total))
    (call $pair (i32.const 4160) (i32.const 6)
      (call $clock_time_get (i32.const 0) (i64.const 1000) (i32.const 32))
      (i64.ne (i64.load (i32.const 32)) (i64.const 0)))
    (drop (call $environ_sizes_get (i32.const 24) (i32.const 28)))
    (call $line (i32.const 4176) (i32.const 12) (i32.load (i32.const 24)))
    (i32.store8 (i32.const 40) (i32.const 255))
    (call $pair (i32.const 4192) (i32.const 7)
      (call $random_get (i32.const 40) (i32.const 4))
      (i32.load8_u (i32.const 40)))
    (call $line (i32.const 4208) (i32.const 6)
      (call $path_open (i32.const 3) (i32.const 0) (i32.const 4560) (i32.const 6)
        (i32.const 0) (i64.const 2) (i64.const 0) (i32.const 0) (i32.const 44)))
    (call $line (i32.const 4224) (i32.const 7)
      (call $fd_prestat_get (i32.const 3) (i32.const 32)))
    (call $line (i32.const 4240) (i32.const 8)
      (call $sock_accept (i32.const 3) (i32.const 0) (i32.const 44)))
    (call $write (i32.const 2) (i32.const 4256) (i32.const 18)))

  ;; `grow`: grows its memory by 64 pages, and exits 0 when it grew and 1 when it was refused.
  (func $grow
    (call $proc_exit (i32.eq (memory.grow (i32.const 64)) (i32.const -1))))

  ;; Whether the start of a line, the `have` bytes of it at 32768, starts with the `len` bytes at
  ;; `at`.
  (func $starts (param $at i32) (param $len i32) (param $have i32) (result i32)
    (local $i i32)
    (if (i32.lt_u (local.get $have) (local.get $len))
      (then (return (i32.const 0))))
    (block $same
      (loop $next
        (br_if $same (i32.eq (local.get $i) (local.get $len)))
        (if (i32.ne
              (i32.load8_u (i32.add (i32.const 32768) (local.get $i)))
              (i32.load8_u (i32.add (local.get $at) (local.get $i))))
          (then (return (i32.const 0))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.const 1))

  ;; Answers the line that starts with the `have` bytes at 32768, as z3 answers lengua's prover;
  ;; a line that asks for nothing gets nothing.
  (func $answer (param $have i32)
    (if (call $starts (i32.const 4504) (i32.const 5) (local.get $have))
      (then (call $print (i32.const 4352) (i32.const 11)) (return)))
    (if (call $starts (i32.const 4512) (i32.const 10) (local.get $have))
      (then (call $print (i32.const 4368) (i32.const 4)) (return)))
    (if (call $starts (i32.const 4528) (i32.const 10) (local.get $have))
      (then (call $print (i32.const 4384) (i32.const 3)) (return)))
    (if (call $starts (i32.const 4544) (i32.const 9) (local.get $have))
      (then (call $print (i32.const 4400) (i32.const 31)))))

  ;; `-in`: the prover's commands, read to the end of its input, each line answered as it ends;
  ;; the first 16 bytes of each line are kept, which tell its command.
  (func $solve
    (local $read i32)
    (local $i i32)
    (local $byte i32)
    (local $have i32)
    (block $end
      (loop $more
        (local.set $read (call $read))
        (br_if $end (i32.eqz (local.get $read)))
        (local.set $i (i32.const 0))
        (block $done
          (loop $bytes
            (br_if $done (i32.eq (local.get $i) (local.get $read)))
            (local.set $byte (i32.load8_u (i32.add (i32.const 16384) (local.get $i))))
            (if (i32.eq (local.get $byte) (i32.const 10))
              (then
                (call $answer (local.get $have))
                (local.set $have (i32.const 0)))
              (else
                (if (i32.lt_u (local.get $have) (i32.const 16))
                  (then
                    (i32.store8 (i32.add (i32.const 32768) (local.get $have)) (local.get $byte))
                    (local.set $have (i32.add (local.get $have) (i32.const 1)))))))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $bytes)))
        (br $more)))
    (if (local.get $have)
      (then (call $answer (local.get $have)))))

  (func (export "_start")
    (drop (call $args_sizes_get (i32.const 16) (i32.const 20)))
    (drop (call $args_get (i32.const 256) (i32.const 512)))
    (if (call $first_is (i32.const 4448) (i32.const 5))
      (then (call $catch) (return)))
    (if (call $first_is (i32.const 4456) (i32.const 5))
      (then (call $uncaught) (return)))
    (if (call $first_is (i32.const 4464) (i32.const 6))
      (then (call $grants) (return)))
    (if (call $first_is (i32.const 4472) (i32.const 4))
      (then (call $grow) (return)))
    (if (call $first_is (i32.const 4480) (i32.const 9))
      (then (call $print (i32.const 4288) (i32.const 46)) (return)))
    (if (call $first_is (i32.const 4496) (i32.const 3))
      (then (call $solve) (return)))
    (call $proc_exit (i32.const 2))))
