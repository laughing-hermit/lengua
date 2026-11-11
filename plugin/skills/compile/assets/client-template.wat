;; follow: gets a page from a web server of example.org and prints its body, and follows one
;; redirect to another of its web servers. The compile skill's client that opens its own
;; connections, in the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md): lengua's library gives $fill and $write_all. The
;; program opens its connections itself, with lengua's tcp_open: its idea grants port 80 of any
;; subdomain of example.org, so it can go where a response sends it, and lengua refuses any
;; other computer with NOTCAPABLE. For each page it writes the computer's name and the path
;; into $place, builds the request from them, opens the connection, sends the request, reads
;; the response with $fill until the server closes the connection, and closes its own end.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (import "lengua_net_1" "tcp_open"
    (func $tcp_open (param i32 i32 i32 i32 i32) (result i32)))
  ;; 3 pages: the small regions in the first, the response in the second, its body in the third.
  (memory (export "memory") 3 3)

  ;; Descriptors: standard output and error.
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_USAGE i32 (i32.const 1))
  (global $EXIT_UNREACHABLE i32 (i32.const 3))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  (global $EXIT_NOT_ALLOWED i32 (i32.const 5))
  ;; The error number of what the idea doesn't grant, as references/host.md names it.
  (global $NOTCAPABLE i32 (i32.const 76))
  ;; What $fetch gives in place of a response's length: no response, and a computer the idea
  ;; doesn't let the program reach.
  (global $UNREACHED i32 (i32.const -1))
  (global $REFUSED i32 (i32.const -2))
  ;; The stop byte that has $fill read until the buffer is full or the input ends.
  (global $READ_TO_END i32 (i32.const -1))
  ;; The port of a web server, and a plain connection, not one over TLS.
  (global $HTTP_PORT i32 (i32.const 80))
  (global $PLAIN i32 (i32.const 0))
  ;; Sizes and limits: the program's name and two arguments; a computer's name of 1 to HOST_MAX
  ;; bytes and a path of 1 to PATH_MAX, each read from NAME_SCAN bytes at most; and a response
  ;; of RESPONSE_MAX bytes at most.
  (global $ARGC_EXPECTED i32 (i32.const 3))
  (global $HOST_MAX i32 (i32.const 253))
  (global $PATH_MAX i32 (i32.const 100))
  (global $NAME_SCAN i32 (i32.const 254))
  (global $RESPONSE_MAX i32 (i32.const 65535))
  ;; Where the computer's name and the path start in $place.
  (global $HOST_AT i32 (i32.const 0))
  (global $PATH_AT i32 (i32.const 256))
  ;; The last places in $request where the line with the computer's name, and the blank line,
  ;; may start.
  (global $HOST_LINE_LAST i32 (i32.const 367))
  (global $BLANK_LAST i32 (i32.const 380))
  ;; The blank line that ends a response's headers, `\r\n\r\n`, read as one 32-bit word, its
  ;; length, and the last place a word of the response starts.
  (global $BLANK_LINE i32 (i32.const 168626701))
  (global $BLANK_LINE_LENGTH i32 (i32.const 4))
  (global $LAST_WORD i32 (i32.const 65532))
  ;; Characters: what ends a computer's name in a redirect, and what ends its path.
  (global $SLASH i32 (i32.const 47))
  (global $CARRIAGE_RETURN i32 (i32.const 13))

  ;; Regions (rule 3, M1), from 1024 up: the messages and the request's fixed parts, read-only,
  ;; then what the host writes, the computer and the path, the request, the arguments' text, the
  ;; response and its body.
  (global $usage i32 (i32.const 1024))
  (global $usage.rosize i32 (i32.const 32))
  (data (i32.const 1024) "usage: follow <computer> <path>\0a")
  (global $cannot_reach i32 (i32.const 1056))
  (global $cannot_reach.rosize i32 (i32.const 30))
  (data (i32.const 1056) "error: can't reach the server\0a")
  (global $not_allowed i32 (i32.const 1088))
  (global $not_allowed.rosize i32 (i32.const 48))
  (data (i32.const 1088) "error: this program may not reach that computer\0a")
  (global $get i32 (i32.const 1136))
  (global $get.rosize i32 (i32.const 4))
  (data (i32.const 1136) "GET ")
  (global $host_line i32 (i32.const 1144))
  (global $host_line.rosize i32 (i32.const 17))
  (data (i32.const 1144) " HTTP/1.0\0d\0aHost: ")
  (global $blank i32 (i32.const 1164))
  (global $blank.rosize i32 (i32.const 4))
  (data (i32.const 1164) "\0d\0a\0d\0a")
  ;; The start of the header line that sends the program to another computer.
  (global $location i32 (i32.const 1168))
  (global $location.rosize i32 (i32.const 19))
  (data (i32.const 1168) "\0d\0aLocation: http://")
  ;; The argument count and the size of the arguments' text, as args_sizes_get writes them.
  (global $sizes i32 (i32.const 1192))
  (global $sizes.size i32 (i32.const 8))
  ;; Where each argument starts, as args_get writes it: the program's name, the computer, the
  ;; path.
  (global $argv i32 (i32.const 1200))
  (global $argv.size i32 (i32.const 12))
  ;; The descriptor of the connection, as tcp_open writes it.
  (global $opened i32 (i32.const 1216))
  (global $opened.size i32 (i32.const 4))
  ;; The computer's name, from HOST_AT, and the path, from PATH_AT, of the page to get.
  (global $place i32 (i32.const 1280))  ;; shared: take_arg and take_text write it; append and fetch read it
  (global $place.size i32 (i32.const 512))
  ;; The request: GET and the path, HTTP/1.0 and the Host line with the computer's name, and a
  ;; blank line.
  (global $request i32 (i32.const 1792))  ;; shared: build_request and append write it; fetch sends it
  (global $request.size i32 (i32.const 384))
  ;; The arguments' text, each ended by a NUL.
  (global $args i32 (i32.const 2176))  ;; shared: main has the host fill it; take_arg reads it
  (global $args.size i32 (i32.const 4096))
  ;; The response: RESPONSE_MAX bytes, and one more, which shows a longer one.
  (global $response i32 (i32.const 65536))  ;; shared: fetch fills it; the functions that read a response read it
  (global $response.size i32 (i32.const 65536))
  ;; The body, copied to a start of its own to print.
  (global $body i32 (i32.const 131072))  ;; shared: copy_body writes it; main prints it
  (global $body.size i32 (i32.const 65536))

  ;; Gets the page the two arguments name, follows a redirect when its response has one, and
  ;; prints the body, or says why it can't. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $err i32)
    (local $host_length i32)
    (local $path_length i32)
    (local $got i32)
    (local $len i32)
    (block $write_failed
      (block $not_allowed
        (block $no_server
          (block $usage
            (local.set $err
              (call $args_sizes_get (global.get $sizes)
                (i32.add (global.get $sizes) (i32.const 4))))
            (if (local.get $err)
              (then (call $assert_fail (i32.const 1)) unreachable))
            (br_if $usage (i32.ne (i32.load (global.get $sizes)) (global.get $ARGC_EXPECTED)))
            ;; Text longer than $args holds is longer than a name and a path.
            (br_if $usage
              (i32.gt_u (i32.load offset=4 (global.get $sizes)) (global.get $args.size)))
            (local.set $err (call $args_get (global.get $argv) (global.get $args)))
            (if (local.get $err)
              (then (call $assert_fail (i32.const 2)) unreachable))
            ;; Each argument starts where its place in argv points: an offset into $args,
            ;; which take_arg checks.
            (local.set $host_length
              (call $take_arg
                (i32.sub (i32.load offset=4 (global.get $argv)) (global.get $args))
                (global.get $HOST_AT) (global.get $HOST_MAX)))
            (br_if $usage (i32.eqz (local.get $host_length)))
            (local.set $path_length
              (call $take_arg
                (i32.sub (i32.load offset=8 (global.get $argv)) (global.get $args))
                (global.get $PATH_AT) (global.get $PATH_MAX)))
            (br_if $usage (i32.eqz (local.get $path_length)))
            (local.set $got (call $fetch (local.get $host_length) (local.get $path_length)))
            (br_if $not_allowed (i32.eq (local.get $got) (global.get $REFUSED)))
            (br_if $no_server (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $response.size))
              (then (call $assert_fail (i32.const 3)) unreachable))
            ;; A response that fills the buffer is longer than RESPONSE_MAX.
            (br_if $no_server (i32.gt_u (local.get $got) (global.get $RESPONSE_MAX)))
            ;; The page a redirect names, when the response is one.
            (local.set $got (call $redirect (local.get $got)))
            (br_if $not_allowed (i32.eq (local.get $got) (global.get $REFUSED)))
            (br_if $no_server (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $response.size))
              (then (call $assert_fail (i32.const 4)) unreachable))
            (br_if $no_server (i32.gt_u (local.get $got) (global.get $RESPONSE_MAX)))
            (local.set $len (call $copy_body (local.get $got)))
            (if (i32.gt_u (local.get $len) (global.get $body.size))
              (then (call $assert_fail (i32.const 5)) unreachable))
            (br_if $write_failed
              (call $write_all (global.get $STDOUT) (global.get $body) (local.get $len)))
            (call $proc_exit (global.get $EXIT_OK))
            unreachable)
          (br_if $write_failed
            (call $write_all (global.get $STDERR) (global.get $usage)
              (global.get $usage.rosize)))
          (call $proc_exit (global.get $EXIT_USAGE))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $cannot_reach)
            (global.get $cannot_reach.rosize)))
        (call $proc_exit (global.get $EXIT_UNREACHABLE))
        unreachable)
      (br_if $write_failed
        (call $write_all (global.get $STDERR) (global.get $not_allowed)
          (global.get $not_allowed.rosize)))
      (call $proc_exit (global.get $EXIT_NOT_ALLOWED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Copies the argument that starts at byte $at of $args, up to its NUL, to $place from $to.
  ;; Gives its length, or 0 when it isn't 1 to $max bytes.
  (func $take_arg (param $at i32) (param $to i32) (param $max i32) (result i32)
    (local $k i32)
    (local $from i32)
    (local $slot i32)
    (local $c i32)
    (local $length i32)
    (local $left i32)
    (if (i32.gt_u (local.get $max) (global.get $HOST_MAX))
      (then (call $assert_fail (i32.const 6)) unreachable))
    (block $invalid
      (block $ended
        (local.set $left (global.get $NAME_SCAN))
        (loop $byte  ;; bound: 254
          (local.set $from (i32.add (local.get $at) (local.get $k)))
          (if (i32.ge_u (local.get $from) (global.get $args.size))
            (then (call $assert_fail (i32.const 7)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $args) (local.get $from))))
          (br_if $ended (i32.eqz (local.get $c)))
          ;; One byte more than the text may have.
          (br_if $invalid (i32.ge_u (local.get $k) (local.get $max)))
          (local.set $slot (i32.add (local.get $to) (local.get $k)))
          (if (i32.ge_u (local.get $slot) (global.get $place.size))
            (then (call $assert_fail (i32.const 8)) unreachable))
          (i32.store8 (i32.add (global.get $place) (local.get $slot)) (local.get $c))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $byte (local.get $left)))
        ;; The text's NUL, or its byte past $max, comes within NAME_SCAN bytes.
        (call $assert_fail (i32.const 9))
        unreachable)
      (local.set $length (local.get $k)))
    (local.get $length))

  ;; Copies the text of the response that starts at byte $at and ends before the byte $stop,
  ;; within the response's $end bytes, to $place from $to. Gives its length, or 0 when it isn't
  ;; 1 to $max bytes or the response ends first.
  (func $take_text (param $at i32) (param $end i32) (param $stop i32) (param $to i32)
    (param $max i32) (result i32)
    (local $k i32)
    (local $from i32)
    (local $slot i32)
    (local $c i32)
    (local $length i32)
    (local $left i32)
    (if (i32.gt_u (local.get $max) (global.get $HOST_MAX))
      (then (call $assert_fail (i32.const 10)) unreachable))
    (if (i32.gt_u (local.get $end) (global.get $response.size))
      (then (call $assert_fail (i32.const 11)) unreachable))
    (block $invalid
      (block $ended
        (local.set $left (global.get $NAME_SCAN))
        (loop $byte  ;; bound: 254
          (local.set $from (i32.add (local.get $at) (local.get $k)))
          ;; The response ends before the text does.
          (br_if $invalid (i32.ge_u (local.get $from) (local.get $end)))
          (if (i32.ge_u (local.get $from) (global.get $response.size))
            (then (call $assert_fail (i32.const 12)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $response) (local.get $from))))
          (br_if $ended (i32.eq (local.get $c) (local.get $stop)))
          ;; One byte more than the text may have.
          (br_if $invalid (i32.ge_u (local.get $k) (local.get $max)))
          (local.set $slot (i32.add (local.get $to) (local.get $k)))
          (if (i32.ge_u (local.get $slot) (global.get $place.size))
            (then (call $assert_fail (i32.const 13)) unreachable))
          (i32.store8 (i32.add (global.get $place) (local.get $slot)) (local.get $c))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $byte (local.get $left)))
        ;; The text's end, or its byte past $max, comes within NAME_SCAN bytes.
        (call $assert_fail (i32.const 14))
        unreachable)
      (local.set $length (local.get $k)))
    (local.get $length))

  ;; Copies $count bytes of $place, from $from, to $request at $at, each place checked. Gives
  ;; where the next byte of the request goes.
  (func $append (param $at i32) (param $from i32) (param $count i32) (result i32)
    (local $k i32)
    (local $source i32)
    (local $slot i32)
    (local $left i32)
    (if (i32.gt_u (local.get $count) (global.get $HOST_MAX))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (block $copied
      (local.set $left (global.get $HOST_MAX))
      (loop $byte  ;; bound: 253
        (br_if $copied (i32.ge_u (local.get $k) (local.get $count)))
        (local.set $source (i32.add (local.get $from) (local.get $k)))
        (if (i32.ge_u (local.get $source) (global.get $place.size))
          (then (call $assert_fail (i32.const 16)) unreachable))
        (local.set $slot (i32.add (local.get $at) (local.get $k)))
        (if (i32.ge_u (local.get $slot) (global.get $request.size))
          (then (call $assert_fail (i32.const 17)) unreachable))
        (i32.store8 (i32.add (global.get $request) (local.get $slot))
          (i32.load8_u (i32.add (global.get $place) (local.get $source))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (i32.add (local.get $at) (local.get $k)))

  ;; Writes the request into $request: GET and the path, of $path_length bytes, then HTTP/1.0
  ;; and the Host line with the computer's name, of $host_length bytes, and a blank line. Gives
  ;; its length.
  (func $build_request (param $host_length i32) (param $path_length i32) (result i32)
    (local $at i32)
    (if (i32.gt_u (local.get $host_length) (global.get $HOST_MAX))
      (then (call $assert_fail (i32.const 18)) unreachable))
    (if (i32.gt_u (local.get $path_length) (global.get $PATH_MAX))
      (then (call $assert_fail (i32.const 19)) unreachable))
    (memory.copy (global.get $request) (global.get $get) (global.get $get.rosize))
    (local.set $at
      (call $append (global.get $get.rosize) (global.get $PATH_AT) (local.get $path_length)))
    (if (i32.gt_u (local.get $at) (global.get $HOST_LINE_LAST))
      (then (call $assert_fail (i32.const 20)) unreachable))
    (memory.copy (i32.add (global.get $request) (local.get $at)) (global.get $host_line)
      (global.get $host_line.rosize))
    (local.set $at
      (call $append (i32.add (local.get $at) (global.get $host_line.rosize))
        (global.get $HOST_AT) (local.get $host_length)))
    (if (i32.gt_u (local.get $at) (global.get $BLANK_LAST))
      (then (call $assert_fail (i32.const 21)) unreachable))
    (memory.copy (i32.add (global.get $request) (local.get $at)) (global.get $blank)
      (global.get $blank.rosize))
    (i32.add (local.get $at) (global.get $blank.rosize)))

  ;; Gets the page of the computer and the path in $place, of $host_length and $path_length
  ;; bytes: opens a connection to the computer, sends the request, reads the response into
  ;; $response until the server closes, and closes the connection. Gives the response's length,
  ;; REFUSED for a computer the idea doesn't grant, or UNREACHED.
  (func $fetch (param $host_length i32) (param $path_length i32) (result i32)
    (local $err i32)
    (local $len i32)
    (local $fd i32)
    (local $got i32)
    (if (i32.gt_u (local.get $host_length) (global.get $HOST_MAX))
      (then (call $assert_fail (i32.const 22)) unreachable))
    (local.set $got (global.get $UNREACHED))
    (block $done
      (local.set $len
        (call $build_request (local.get $host_length) (local.get $path_length)))
      (if (i32.gt_u (local.get $len) (global.get $request.size))
        (then (call $assert_fail (i32.const 23)) unreachable))
      (local.set $err
        (call $tcp_open (global.get $place) (local.get $host_length) (global.get $HTTP_PORT)
          (global.get $PLAIN) (global.get $opened)))
      (if (i32.eq (local.get $err) (global.get $NOTCAPABLE))
        (then (local.set $got (global.get $REFUSED))))
      ;; A computer lengua refuses, or one that takes no connection.
      (br_if $done (local.get $err))
      (local.set $fd (i32.load (global.get $opened)))
      (block $read
        ;; A connection that takes no write has no response.
        (br_if $read
          (call $write_all (local.get $fd) (global.get $request) (local.get $len)))
        ;; The whole response: the server closes the connection after it.
        (local.set $got
          (call $fill (local.get $fd) (global.get $response) (global.get $response.size)
            (global.get $READ_TO_END)))
        (br_if $read (i32.lt_s (local.get $got) (i32.const 0))))
      (local.set $err (call $fd_close (local.get $fd)))
      (if (local.get $err)
        (then (call $assert_fail (i32.const 24)) unreachable)))
    (local.get $got))

  ;; Gives the response to print: the one of $got bytes in $response, or, when its headers send
  ;; the program to another computer, what $fetch gives for the page they name. A response
  ;; whose headers name no computer and path is printed as it is.
  (func $redirect (param $got i32) (result i32)
    (local $at i32)
    (local $host_length i32)
    (local $path_length i32)
    (local $next i32)
    (if (i32.gt_u (local.get $got) (global.get $response.size))
      (then (call $assert_fail (i32.const 25)) unreachable))
    (local.set $next (local.get $got))
    (block $kept
      (local.set $at (call $find_location (call $body_start (local.get $got))))
      (br_if $kept (i32.eqz (local.get $at)))
      ;; The computer's name ends at the path's slash, and the path at the line's end.
      (local.set $host_length
        (call $take_text (local.get $at) (local.get $got) (global.get $SLASH)
          (global.get $HOST_AT) (global.get $HOST_MAX)))
      (br_if $kept (i32.eqz (local.get $host_length)))
      (local.set $path_length
        (call $take_text (i32.add (local.get $at) (local.get $host_length)) (local.get $got)
          (global.get $CARRIAGE_RETURN) (global.get $PATH_AT) (global.get $PATH_MAX)))
      (br_if $kept (i32.eqz (local.get $path_length)))
      (local.set $next (call $fetch (local.get $host_length) (local.get $path_length))))
    (local.get $next))

  ;; Gives where the computer's name starts in the header line that sends the program to
  ;; another computer, just after `Location: http://`, in the first $end bytes of the response,
  ;; its headers; or 0 when they have no such line.
  (func $find_location (param $end i32) (result i32)
    (local $i i32)
    (local $j i32)
    (local $from i32)
    (local $found i32)
    (local $left i32)
    (local $same i32)
    (if (i32.gt_u (local.get $end) (global.get $response.size))
      (then (call $assert_fail (i32.const 26)) unreachable))
    (block $done
      (local.set $left (global.get $response.size))
      (loop $start  ;; bound: 65536
        ;; The line's start no longer fits before $end.
        (br_if $done
          (i32.gt_u (i32.add (local.get $i) (global.get $location.rosize)) (local.get $end)))
        (block $differs
          (local.set $j (i32.const 0))
          (local.set $same (global.get $location.rosize))
          (loop $byte  ;; bound: 19
            (local.set $from (i32.add (local.get $i) (local.get $j)))
            (if (i32.ge_u (local.get $from) (global.get $response.size))
              (then (call $assert_fail (i32.const 27)) unreachable))
            (if (i32.ge_u (local.get $j) (global.get $location.rosize))
              (then (call $assert_fail (i32.const 28)) unreachable))
            (br_if $differs
              (i32.ne (i32.load8_u (i32.add (global.get $response) (local.get $from)))
                (i32.load8_u (i32.add (global.get $location) (local.get $j)))))
            (local.set $j (i32.add (local.get $j) (i32.const 1)))
            (local.set $same (i32.sub (local.get $same) (i32.const 1)))
            (br_if $byte (local.get $same)))
          ;; Every byte of the line's start is there.
          (local.set $found (i32.add (local.get $i) (global.get $location.rosize)))
          (br $done))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $start (local.get $left))))
    (local.get $found))

  ;; Gives where the body of the $got bytes of the response starts: just after its first blank
  ;; line, or at $got when it has none.
  (func $body_start (param $got i32) (result i32)
    (local $i i32)
    (local $start i32)
    (local $left i32)
    (if (i32.gt_u (local.get $got) (global.get $response.size))
      (then (call $assert_fail (i32.const 29)) unreachable))
    (local.set $start (local.get $got))
    (block $found
      (local.set $left (global.get $response.size))
      (loop $word  ;; bound: 65536
        ;; A blank line's four bytes are left from $i.
        (br_if $found
          (i32.gt_u (i32.add (local.get $i) (global.get $BLANK_LINE_LENGTH)) (local.get $got)))
        (if (i32.gt_u (local.get $i) (global.get $LAST_WORD))
          (then (call $assert_fail (i32.const 30)) unreachable))
        (if (i32.eq (i32.load (i32.add (global.get $response) (local.get $i)))
              (global.get $BLANK_LINE))
          (then
            (local.set $start (i32.add (local.get $i) (global.get $BLANK_LINE_LENGTH)))
            (br $found)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $word (local.get $left))))
    (local.get $start))

  ;; Copies the body of the $got bytes of the response, all of it after its first blank line, to
  ;; $body, one byte at a time, each place checked, and gives its length.
  (func $copy_body (param $got i32) (result i32)
    (local $start i32)
    (local $k i32)
    (local $from i32)
    (local $left i32)
    (if (i32.gt_u (local.get $got) (global.get $response.size))
      (then (call $assert_fail (i32.const 31)) unreachable))
    (local.set $start (call $body_start (local.get $got)))
    (block $copied
      (local.set $left (global.get $body.size))
      (loop $byte  ;; bound: 65536
        (local.set $from (i32.add (local.get $start) (local.get $k)))
        (br_if $copied (i32.ge_u (local.get $from) (local.get $got)))
        (if (i32.ge_u (local.get $from) (global.get $response.size))
          (then (call $assert_fail (i32.const 32)) unreachable))
        (if (i32.ge_u (local.get $k) (global.get $body.size))
          (then (call $assert_fail (i32.const 33)) unreachable))
        (i32.store8 (i32.add (global.get $body) (local.get $k))
          (i32.load8_u (i32.add (global.get $response) (local.get $from))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $k))
)
