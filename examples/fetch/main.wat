;; fetch: gets a page from the web server on port 8080 of this computer and prints its body. The
;; WebAssembly port of the native fetch, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $fill and $write_all. lengua
;; makes the connection to the server before the program starts and hands it over as descriptor
;; 5, the first connection's. The program sends its request there, reads the response with $fill
;; until the server closes the connection, and copies the body to a start of its own to print it.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  ;; 3 pages: the small regions in the first, the response in the second, its body in the third.
  (memory (export "memory") 3 3)

  ;; Descriptors: standard output and error, and the connection lengua made to the server.
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  (global $CONNECTION i32 (i32.const 5))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_UNREACHABLE i32 (i32.const 3))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; Sizes and limits: a path of 1 to PATH_MAX bytes, read from PATH_SCAN bytes at most, and a
  ;; response of RESPONSE_MAX bytes at most.
  (global $ARGC_EXPECTED i32 (i32.const 2))
  (global $PATH_MAX i32 (i32.const 100))
  (global $PATH_SCAN i32 (i32.const 101))
  (global $RESPONSE_MAX i32 (i32.const 65535))
  ;; The blank line that ends a response's headers, `\r\n\r\n`, read as one 32-bit word, its
  ;; length, and the last place a word of the response starts.
  (global $BLANK_LINE i32 (i32.const 168626701))
  (global $BLANK_LINE_LENGTH i32 (i32.const 4))
  (global $LAST_WORD i32 (i32.const 65532))

  ;; Regions (rule 3, M1), from 1024 up: the messages and the request's two ends, read-only, then
  ;; what the host writes, the request, the arguments' text, the response and its body.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 20))
  (data (i32.const 1024) "usage: fetch <path>\0a")
  (global $cannot_reach i32 (i32.const 1056))
  (global $cannot_reach.rosize i32 (i32.const 30))
  (data (i32.const 1056) "error: can't reach the server\0a")
  (global $get i32 (i32.const 1088))
  (global $get.rosize i32 (i32.const 4))
  (data (i32.const 1088) "GET ")
  (global $version i32 (i32.const 1096))
  (global $version.rosize i32 (i32.const 13))
  (data (i32.const 1096) " HTTP/1.0\0d\0a\0d\0a")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 1112))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, then the path.
  (global $argv i32 (i32.const 1120))
  (global $argv.size i32 (i32.const 8))
  ;; The request: GET and a space, the path, and HTTP/1.0 with a blank line.
  (global $request i32 (i32.const 1136))  ;; shared: build_request writes it; main sends it
  (global $request.size i32 (i32.const 117))
  ;; The arguments' text, each ended by a NUL.
  (global $args i32 (i32.const 1280))  ;; shared: main has the host fill it; build_request reads it
  (global $args.size i32 (i32.const 4096))
  ;; The response: RESPONSE_MAX bytes, and one more, which shows a longer one.
  (global $response i32 (i32.const 65536))  ;; shared: main fills it; the body's functions read it
  (global $response.size i32 (i32.const 65536))
  ;; The body, copied to a start of its own to print.
  (global $body i32 (i32.const 131072))  ;; shared: copy_body writes it; main prints it
  (global $body.size i32 (i32.const 65536))

  ;; Sends the request for the path in the argument, reads the response and prints its body, or
  ;; says why it can't. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $len i32)
    (local $got i32)
    (block $write_failed
      (block $no_server
        (block $usage
          (local.set $err
            (call $args_sizes_get (global.get $sizes)
              (i32.add (global.get $sizes) (i32.const 4))))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 1)) unreachable))
          (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
          ;; Text longer than $args holds is longer than a path.
          (br_if $usage
            (i32.gt_u (i32.load offset=4 (global.get $sizes)) (global.get $args.size)))
          (local.set $err (call $args_get (global.get $argv) (global.get $args)))
          (if (local.get $err)
            (then (call $assert_fail (i32.const 2)) unreachable))
          ;; The path starts where argv[1] points: an offset into $args, which build_request
          ;; checks.
          (local.set $len
            (call $build_request
              (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args))))
          (br_if $usage (i32.eqz (local.get $len)))
          (if (i32.gt_u (local.get $len) (global.get $request.size))
            (then (call $assert_fail (i32.const 3)) unreachable))
          ;; A connection that lengua couldn't make takes no write.
          (br_if $no_server
            (call $write_all (global.get $CONNECTION) (global.get $request) (local.get $len)))
          ;; The whole response: the server closes the connection after it.
          (local.set $got
            (call $fill (global.get $CONNECTION) (global.get $response)
              (global.get $response.size) (global.get $READ_TO_END)))
          (br_if $no_server (i32.lt_s (local.get $got) (i32.const 0)))
          (if (i32.gt_u (local.get $got) (global.get $response.size))
            (then (call $assert_fail (i32.const 4)) unreachable))
          ;; A response that fills the buffer is longer than RESPONSE_MAX.
          (br_if $no_server (i32.gt_u (local.get $got) (global.get $RESPONSE_MAX)))
          (local.set $len (call $copy_body (local.get $got)))
          (if (i32.gt_u (local.get $len) (global.get $body.size))
            (then (call $assert_fail (i32.const 5)) unreachable))
          (br_if $write_failed
            (call $write_all (global.get $STDOUT) (global.get $body) (local.get $len)))
          (call $proc_exit (global.get $EXIT_OK))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $usage) (global.get $usage.rosize)))
        (call $proc_exit (global.get $EXIT_USAGE))
        unreachable)
      (br_if $write_failed
        (call $write_all (global.get $STDERR) (global.get $cannot_reach)
          (global.get $cannot_reach.rosize)))
      (call $proc_exit (global.get $EXIT_UNREACHABLE))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Writes the request into $request: GET and a space, the path that starts at byte $at of
  ;; $args, and HTTP/1.0 with a blank line. Gives its length, or 0 when the path isn't 1 to
  ;; PATH_MAX bytes.
  (func $build_request (param $at i32) (result i32)
    (local $k i32)
    (local $from i32)
    (local $c i32)
    (local $length i32)
    (local $left i32)
    (memory.copy (global.get $request) (global.get $get) (global.get $get.rosize))
    (block $invalid
      (block $ended
        (local.set $left (global.get $PATH_SCAN))
        (loop $byte  ;; bound: 101
          (local.set $from (i32.add (local.get $at) (local.get $k)))
          (if (i32.ge_u (local.get $from) (global.get $args.size))
            (then (call $assert_fail (i32.const 6)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $from))))
          (br_if $ended (i32.eqz (local.get $c)))
          ;; One byte more than a path has.
          (br_if $invalid (i32.ge_u (local.get $k) (global.get $PATH_MAX)))
          (i32.store8
            (i32.add (global.get $request) (i32.add (global.get $get.rosize) (local.get $k)))
            (local.get $c))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $byte (local.get $left)))
        ;; The path's NUL, or its byte past PATH_MAX, comes within PATH_SCAN bytes.
        (call $assert_fail (i32.const 7))
        unreachable)
      (br_if $invalid (i32.eqz (local.get $k)))
      (if (i32.gt_u (local.get $k) (global.get $PATH_MAX))
        (then (call $assert_fail (i32.const 8)) unreachable))
      (memory.copy
        (i32.add (global.get $request) (i32.add (global.get $get.rosize) (local.get $k)))
        (global.get $version) (global.get $version.rosize))
      (local.set $length
        (i32.add (i32.add (global.get $get.rosize) (local.get $k)) (global.get $version.rosize))))
    (local.get $length))

  ;; Gives where the body of the $got bytes of the response starts: just after its first blank
  ;; line, or at $got when it has none.
  (func $body_start (param $got i32) (result i32)
    (local $i i32)
    (local $start i32)
    (local $left i32)
    (if (i32.gt_u (local.get $got) (global.get $response.size))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (local.set $start (local.get $got))
    (block $found
      (local.set $left (global.get $response.size))
      (loop $place  ;; bound: 65536
        ;; A blank line's four bytes are left from $i.
        (br_if $found
          (i32.gt_u (i32.add (local.get $i) (global.get $BLANK_LINE_LENGTH)) (local.get $got)))
        (if (i32.gt_u (local.get $i) (global.get $LAST_WORD))
          (then (call $assert_fail (i32.const 10)) unreachable))
        (if (i32.eq (i32.load (i32.add (global.get $response) (local.get $i)))
              (global.get $BLANK_LINE))
          (then
            (local.set $start (i32.add (local.get $i) (global.get $BLANK_LINE_LENGTH)))
            (br $found)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $place (local.get $left))))
    (local.get $start))

  ;; Copies the body of the $got bytes of the response, all of it after its first blank line, to
  ;; $body, one byte at a time, each place checked, and gives its length.
  (func $copy_body (param $got i32) (result i32)
    (local $start i32)
    (local $k i32)
    (local $from i32)
    (local $left i32)
    (if (i32.gt_u (local.get $got) (global.get $response.size))
      (then (call $assert_fail (i32.const 11)) unreachable))
    (local.set $start (call $body_start (local.get $got)))
    (block $copied
      (local.set $left (global.get $body.size))
      (loop $byte  ;; bound: 65536
        (local.set $from (i32.add (local.get $start) (local.get $k)))
        (br_if $copied (i32.ge_u (local.get $from) (local.get $got)))
        (if (i32.ge_u (local.get $from) (global.get $response.size))
          (then (call $assert_fail (i32.const 12)) unreachable))
        (if (i32.ge_u (local.get $k) (global.get $body.size))
          (then (call $assert_fail (i32.const 13)) unreachable))
        (i32.store8 (i32.add (global.get $body) (local.get $k))
          (i32.load8_u (i32.add (global.get $response) (local.get $from))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $k))
)
