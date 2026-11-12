;; http: a part. Its idea's contract offers the functions that read the head of an HTTP/1.1
;; request: where the head ends, its request line, its header fields, and the path with its
;; percent escapes decoded. Each works only through the addresses it is lent and reads only the
;; $HTTP_ constants below. The part's own program prints the method, the path and the Host
;; field of the request on standard input, so its tests and its proofs go through every
;; function. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md).
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_BAD_REQUEST i32 (i32.const 1))
  (global $EXIT_TOO_LONG i32 (i32.const 2))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  (global $EXIT_VERSION i32 (i32.const 5))
  ;; $fill reads to the input's end.
  (global $READ_TO_END i32 (i32.const -1))
  (global $NEWLINE i32 (i32.const 10))
  ;; The length of the name host, and the most the output's first line takes: a method, a
  ;; space and a path.
  (global $HOST_LENGTH i32 (i32.const 4))
  (global $FIRST_LINE i32 (i32.const 1041))

  ;; The part's constants, which its functions read. A program that takes a function defines
  ;; each constant the function reads as it is here.
  ;; The most bytes a head has, and the last places where a run of 4, of 2 and of the 10 bytes
  ;; of a version and its line end may start.
  (global $HTTP_HEAD i32 (i32.const 4096))
  (global $HTTP_LAST_WORD i32 (i32.const 4092))
  (global $HTTP_LAST_PAIR i32 (i32.const 4094))
  (global $HTTP_LAST_VERSION i32 (i32.const 4086))
  (global $HTTP_WORD i32 (i32.const 4))
  (global $HTTP_PAIR i32 (i32.const 2))
  (global $HTTP_VERSION_BYTES i32 (i32.const 10))
  ;; The most letters a method has, lines a head has, bytes a field's name has, and bytes a
  ;; decoded path has.
  (global $HTTP_METHOD i32 (i32.const 16))
  (global $HTTP_LINES i32 (i32.const 2048))
  (global $HTTP_NAME i32 (i32.const 32))
  (global $HTTP_PATH i32 (i32.const 1024))
  ;; What $http_at gives past the text's end, above every byte; what $http_path gives for a
  ;; path it refuses; what $http_hex gives for a byte that is no hexadecimal digit.
  (global $HTTP_NONE i32 (i32.const 256))
  (global $HTTP_NO i32 (i32.const -1))
  (global $HTTP_NO_DIGIT i32 (i32.const 16))
  ;; What $http_request_line gives: 0, or the status that answers the request.
  (global $HTTP_OK i32 (i32.const 0))
  (global $HTTP_BAD_REQUEST i32 (i32.const 400))
  (global $HTTP_NOT_SUPPORTED i32 (i32.const 505))
  ;; A blank line after a line, CR LF CR LF; a line end, CR LF; the word HTTP; and 1 and a
  ;; point: each as the bytes a load gives, the first byte lowest.
  (global $HTTP_BLANK_LINE i32 (i32.const 0x0a0d0a0d))
  (global $HTTP_LINE_END i32 (i32.const 0x0a0d))
  (global $HTTP_WORD_HTTP i32 (i32.const 0x50545448))
  (global $HTTP_ONE_POINT i32 (i32.const 0x2e31))
  ;; Characters.
  (global $HTTP_TAB i32 (i32.const 9))
  (global $HTTP_SPACE i32 (i32.const 32))
  (global $HTTP_PERCENT i32 (i32.const 37))
  (global $HTTP_SLASH i32 (i32.const 47))
  (global $HTTP_ZERO i32 (i32.const 48))
  (global $HTTP_COLON i32 (i32.const 58))
  (global $HTTP_QUESTION i32 (i32.const 63))
  (global $HTTP_BIG_A i32 (i32.const 65))
  (global $HTTP_SMALL_A i32 (i32.const 97))
  (global $HTTP_DELETE i32 (i32.const 127))
  ;; The first visible character, and how far the last one is from it; how far the last
  ;; character that prints is from the space; how far Z is from A, 9 from 0 and f from a; the
  ;; bit that makes a letter small; ten; and the bits of a hexadecimal digit.
  (global $HTTP_FIRST_VISIBLE i32 (i32.const 33))
  (global $HTTP_VISIBLE i32 (i32.const 93))
  (global $HTTP_PRINTS i32 (i32.const 94))
  (global $HTTP_LETTERS i32 (i32.const 25))
  (global $HTTP_NINE i32 (i32.const 9))
  (global $HTTP_FIVE i32 (i32.const 5))
  (global $HTTP_CASE_BIT i32 (i32.const 32))
  (global $HTTP_TEN i32 (i32.const 10))
  (global $HTTP_HEX_SHIFT i32 (i32.const 4))

  ;; Regions (rule 3, M1), from 1024 up: the messages and the name host, read-only, then the
  ;; input, what the functions write, and the output.
  (global $bad_request i32 (i32.const 1024))
  (global $bad_request.rosize i32 (i32.const 19))
  (data (i32.const 1024) "error: bad request\0a")
  (global $too_long i32 (i32.const 1056))
  (global $too_long.rosize i32 (i32.const 24))
  (data (i32.const 1056) "error: request too long\0a")
  (global $version i32 (i32.const 1088))
  (global $version.rosize i32 (i32.const 15))
  (data (i32.const 1088) "error: version\0a")
  ;; The name of the Host field, in small letters, in the HTTP_NAME bytes that $http_name_is
  ;; may read.
  (global $host i32 (i32.const 1120))
  (global $host.rosize i32 (i32.const 32))
  (data (i32.const 1120) "host\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")
  ;; Standard input: HTTP_HEAD bytes, and one more to see input that is too long.
  (global $text i32 (i32.const 2048))  ;; shared: main reads into it and lends it, emit copies from it
  (global $text.size i32 (i32.const 4097))
  ;; What $http_request_line writes: the method's length, where the target starts, the target's
  ;; length and the version's last digit, 4 bytes each.
  (global $line i32 (i32.const 6208))  ;; shared: main has it written, report reads it
  (global $line.size i32 (i32.const 16))
  ;; What $http_header writes: where a field's value starts and its length.
  (global $field i32 (i32.const 6240))
  (global $field.size i32 (i32.const 8))
  ;; The output: the method, a space, the path and a newline, then the Host field's value and
  ;; a newline.
  (global $out i32 (i32.const 8192))  ;; shared: main builds the output in it, emit copies into it
  (global $out.size i32 (i32.const 5200))

  ;; Reads a request's head from standard input and prints its method, its path and its Host
  ;; field, or says why it can't. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $head i32)
    (local $status i32)
    (local $at i32)
    (block $write_failed
      (block $read_failed
        (block $too_long
          (block $version
            (block $bad
              (local.set $got
                (call $fill (global.get $STDIN) (global.get $text) (global.get $text.size)
                  (global.get $READ_TO_END)))
              (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
              (if (i32.gt_u (local.get $got) (global.get $text.size))
                (then (call $assert_fail (i32.const 1)) unreachable))
              (br_if $too_long (i32.gt_u (local.get $got) (global.get $HTTP_HEAD)))
              (local.set $head (call $http_head_end (global.get $text) (local.get $got)))
              (br_if $bad (i32.eqz (local.get $head)))
              (if (i32.gt_u (local.get $head) (global.get $HTTP_HEAD))
                (then (call $assert_fail (i32.const 2)) unreachable))
              (local.set $status
                (call $http_request_line (global.get $text) (local.get $head)
                  (global.get $line)))
              (br_if $version (i32.eq (local.get $status) (global.get $HTTP_NOT_SUPPORTED)))
              (br_if $bad (local.get $status))
              (br_if $bad
                (i32.eqz (call $http_headers_valid (global.get $text) (local.get $head))))
              (local.set $at (call $report (local.get $head)))
              (br_if $bad (i32.eq (local.get $at) (global.get $HTTP_NO)))
              (if (i32.gt_u (local.get $at) (global.get $out.size))
                (then (call $assert_fail (i32.const 3)) unreachable))
              (br_if $write_failed
                (call $write_all (global.get $STDOUT) (global.get $out) (local.get $at)))
              (call $proc_exit (global.get $EXIT_OK))
              unreachable)
            (br_if $write_failed
              (call $write_all (global.get $STDERR) (global.get $bad_request)
                (global.get $bad_request.rosize)))
            (call $proc_exit (global.get $EXIT_BAD_REQUEST))
            unreachable)
          (br_if $write_failed
            (call $write_all (global.get $STDERR) (global.get $version)
              (global.get $version.rosize)))
          (call $proc_exit (global.get $EXIT_VERSION))
          unreachable)
        (br_if $write_failed
          (call $write_all (global.get $STDERR) (global.get $too_long)
            (global.get $too_long.rosize)))
        (call $proc_exit (global.get $EXIT_TOO_LONG))
        unreachable)
      (call $proc_exit (global.get $EXIT_READ_FAILED))
      unreachable)
    (call $proc_exit (global.get $EXIT_WRITE_FAILED))
    unreachable)

  ;; Writes the output for the head of $head bytes at the input's start: the method, a space,
  ;; the decoded path and a newline, then the Host field's value and a newline. Gives the
  ;; output's length, or HTTP_NO when the path is not valid.
  (func $report (param $head i32) (result i32)
    (local $at i32)
    (local $start i32)
    (local $length i32)
    (local $result i32)
    (if (i32.gt_u (local.get $head) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 4)) unreachable))
    (local.set $result (global.get $HTTP_NO))
    (block $bad
      ;; The method, which starts the head.
      (local.set $length (i32.load (global.get $line)))
      (if (i32.gt_u (local.get $length) (global.get $HTTP_METHOD))
        (then (call $assert_fail (i32.const 5)) unreachable))
      (local.set $at (call $emit (i32.const 0) (i32.const 0) (local.get $length)))
      (if (i32.gt_u (local.get $at) (global.get $HTTP_METHOD))
        (then (call $assert_fail (i32.const 6)) unreachable))
      (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $HTTP_SPACE))
      (local.set $at (i32.add (local.get $at) (i32.const 1)))
      ;; The path, decoded where it goes in the output.
      (local.set $start (i32.load offset=4 (global.get $line)))
      (local.set $length (i32.load offset=8 (global.get $line)))
      (if (i32.gt_u (local.get $start) (global.get $HTTP_HEAD))
        (then (call $assert_fail (i32.const 7)) unreachable))
      (if (i32.gt_u (local.get $length) (global.get $HTTP_HEAD))
        (then (call $assert_fail (i32.const 8)) unreachable))
      (local.set $length
        (call $http_path (global.get $text) (local.get $start)
          (i32.add (local.get $start) (local.get $length))
          (i32.add (global.get $out) (local.get $at))))
      (br_if $bad (i32.eq (local.get $length) (global.get $HTTP_NO)))
      (local.set $at (i32.add (local.get $at) (local.get $length)))
      (if (i32.gt_u (local.get $at) (global.get $FIRST_LINE))
        (then (call $assert_fail (i32.const 12)) unreachable))
      (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
      (local.set $at (i32.add (local.get $at) (i32.const 1)))
      ;; The Host field's value, when the head has the field.
      (if
        (call $http_header (global.get $text) (local.get $head) (global.get $host)
          (global.get $HOST_LENGTH) (global.get $field))
        (then
          (local.set $start (i32.load (global.get $field)))
          (local.set $length (i32.load offset=4 (global.get $field)))
          (local.set $at
            (call $emit (local.get $at) (local.get $start) (local.get $length)))))
      (if (i32.ge_u (local.get $at) (global.get $out.size))
        (then (call $assert_fail (i32.const 13)) unreachable))
      (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))
      (local.set $result (i32.add (local.get $at) (i32.const 1))))
    (local.get $result))

  ;; Copies $count bytes from place $from of the input to place $at of the output, one at a
  ;; time, each place checked, and gives the place after the last byte it wrote.
  (func $emit (param $at i32) (param $from i32) (param $count i32) (result i32)
    (local $k i32)
    (local $source i32)
    (local $left i32)
    (if (i32.gt_u (local.get $count) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (block $done
      (local.set $left (global.get $HTTP_HEAD))
      (loop $byte  ;; bound: 4096
        (br_if $done (i32.ge_u (local.get $k) (local.get $count)))
        (local.set $source (i32.add (local.get $from) (local.get $k)))
        (if (i32.ge_u (local.get $source) (global.get $text.size))
          (then (call $assert_fail (i32.const 10)) unreachable))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 11)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.load8_u (i32.add (global.get $text) (local.get $source))))
        (local.set $at (i32.add (local.get $at) (i32.const 1)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $at))

  ;; Gives the byte at place $i of the text at $text, or HTTP_NONE when $i is at or past
  ;; $length. $length is at most HTTP_HEAD.
  (func $http_at (param $text i32) (param $i i32) (param $length i32) (result i32)
    (local $c i32)
    (if (i32.gt_u (local.get $length) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 760)) unreachable))
    (local.set $c (global.get $HTTP_NONE))
    (if (i32.lt_u (local.get $i) (local.get $length))
      (then
        (if (i32.ge_u (local.get $i) (global.get $HTTP_HEAD))
          (then (call $assert_fail (i32.const 761)) unreachable))
        (local.set $c (i32.load8_u (i32.add (local.get $text) (local.get $i))))))
    (local.get $c))

  ;; Gives the place after the run of bytes from $first to $first plus $span that starts at
  ;; place $from of the text at $text: $from itself when its byte is no such byte. $length is
  ;; at most HTTP_HEAD, and $first plus $span is below HTTP_NONE.
  (func $http_span_end (param $text i32) (param $from i32) (param $length i32)
    (param $first i32) (param $span i32) (result i32)
    (local $i i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 762)) unreachable))
    (local.set $i (local.get $from))
    (block $done
      (local.set $left (global.get $HTTP_HEAD))
      (loop $byte  ;; bound: 4096
        ;; Below $first wraps to a large number, so one unsigned test catches both sides.
        (br_if $done
          (i32.gt_u
            (i32.sub (call $http_at (local.get $text) (local.get $i) (local.get $length))
              (local.get $first))
            (local.get $span)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (if (i32.lt_u (local.get $i) (local.get $from))
      (then (call $assert_fail (i32.const 763)) unreachable))
    (local.get $i))

  ;; Gives the place after the first blank line, CR LF CR LF, in the first $length bytes at
  ;; $text: how many bytes the head has. Gives 0 when they hold no blank line. $length is at
  ;; most HTTP_HEAD.
  (func $http_head_end (param $text i32) (param $length i32) (result i32)
    (local $i i32)
    (local $end i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 764)) unreachable))
    (block $done
      (local.set $left (global.get $HTTP_HEAD))
      (loop $byte  ;; bound: 4096
        (br_if $done
          (i32.gt_u (i32.add (local.get $i) (global.get $HTTP_WORD)) (local.get $length)))
        (if (i32.gt_u (local.get $i) (global.get $HTTP_LAST_WORD))
          (then (call $assert_fail (i32.const 765)) unreachable))
        (if
          (i32.eq (i32.load (i32.add (local.get $text) (local.get $i)))
            (global.get $HTTP_BLANK_LINE))
          (then
            (local.set $end (i32.add (local.get $i) (global.get $HTTP_WORD)))
            (br $done)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $end))

  ;; Gives the place of the first line end, CR LF, at or after place $from of the text at
  ;; $text, or $length when there is none. $length is at most HTTP_HEAD.
  (func $http_line_end (param $text i32) (param $from i32) (param $length i32) (result i32)
    (local $i i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 766)) unreachable))
    (local.set $i (local.get $from))
    (block $done
      (local.set $left (global.get $HTTP_HEAD))
      (loop $byte  ;; bound: 4096
        ;; No room for a line end: the rest has none.
        (if (i32.gt_u (i32.add (local.get $i) (global.get $HTTP_PAIR)) (local.get $length))
          (then
            (local.set $i (local.get $length))
            (br $done)))
        (if (i32.gt_u (local.get $i) (global.get $HTTP_LAST_PAIR))
          (then (call $assert_fail (i32.const 767)) unreachable))
        (br_if $done
          (i32.eq (i32.load16_u (i32.add (local.get $text) (local.get $i)))
            (global.get $HTTP_LINE_END)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $i))

  ;; Reads the request line that starts the first $length bytes at $text: a method of 1 to
  ;; HTTP_METHOD capital letters, a space, a target that starts with a slash and has only
  ;; visible characters, a space, HTTP/1.0 or HTTP/1.1, and a line end. Writes at $into, 4
  ;; bytes each, the method's length, where the target starts, the target's length and the
  ;; version's last digit, and gives HTTP_OK. Gives HTTP_NOT_SUPPORTED for another version of
  ;; HTTP and HTTP_BAD_REQUEST for any other text, and then writes nothing. $length is at most
  ;; HTTP_HEAD.
  (func $http_request_line (param $text i32) (param $length i32) (param $into i32) (result i32)  ;; returns: result
    (local $method i32)
    (local $target i32)
    (local $end i32)
    (local $i i32)
    (local $minor i32)
    (local $status i32)
    (if (i32.gt_u (local.get $length) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 768)) unreachable))
    (local.set $status (global.get $HTTP_BAD_REQUEST))
    (block $done
      (local.set $method
        (call $http_span_end (local.get $text) (i32.const 0) (local.get $length)
          (global.get $HTTP_BIG_A) (global.get $HTTP_LETTERS)))
      (br_if $done (i32.eqz (local.get $method)))
      (br_if $done (i32.gt_u (local.get $method) (global.get $HTTP_METHOD)))
      (br_if $done
        (i32.ne (call $http_at (local.get $text) (local.get $method) (local.get $length))
          (global.get $HTTP_SPACE)))
      (local.set $target (i32.add (local.get $method) (i32.const 1)))
      (br_if $done
        (i32.ne (call $http_at (local.get $text) (local.get $target) (local.get $length))
          (global.get $HTTP_SLASH)))
      (local.set $end
        (call $http_span_end (local.get $text) (local.get $target) (local.get $length)
          (global.get $HTTP_FIRST_VISIBLE) (global.get $HTTP_VISIBLE)))
      (if (i32.lt_u (local.get $end) (local.get $target))
        (then (call $assert_fail (i32.const 769)) unreachable))
      (br_if $done
        (i32.ne (call $http_at (local.get $text) (local.get $end) (local.get $length))
          (global.get $HTTP_SPACE)))
      ;; The version and the line end, 10 bytes, must be inside the text.
      (local.set $i (i32.add (local.get $end) (i32.const 1)))
      (br_if $done (i32.gt_u (local.get $i) (global.get $HTTP_LAST_VERSION)))
      (br_if $done
        (i32.gt_u (i32.add (local.get $i) (global.get $HTTP_VERSION_BYTES))
          (local.get $length)))
      (br_if $done
        (i32.ne (i32.load (i32.add (local.get $text) (local.get $i)))
          (global.get $HTTP_WORD_HTTP)))
      (br_if $done
        (i32.ne (i32.load8_u offset=4 (i32.add (local.get $text) (local.get $i)))
          (global.get $HTTP_SLASH)))
      ;; From here the text is HTTP of some version: only 1.0 and 1.1 are read.
      (local.set $status (global.get $HTTP_NOT_SUPPORTED))
      (br_if $done
        (i32.ne (i32.load16_u offset=5 (i32.add (local.get $text) (local.get $i)))
          (global.get $HTTP_ONE_POINT)))
      (local.set $minor
        (i32.sub (i32.load8_u offset=7 (i32.add (local.get $text) (local.get $i)))
          (global.get $HTTP_ZERO)))
      (br_if $done (i32.gt_u (local.get $minor) (i32.const 1)))
      (local.set $status (global.get $HTTP_BAD_REQUEST))
      (br_if $done
        (i32.ne (i32.load16_u offset=8 (i32.add (local.get $text) (local.get $i)))
          (global.get $HTTP_LINE_END)))
      (i32.store (local.get $into) (local.get $method))
      (i32.store offset=4 (local.get $into) (local.get $target))
      (i32.store offset=8 (local.get $into) (i32.sub (local.get $end) (local.get $target)))
      (i32.store offset=12 (local.get $into) (local.get $minor))
      (local.set $status (global.get $HTTP_OK)))
    (local.get $status))

  ;; Gives 1 when every header field of the head in the first $length bytes at $text is a
  ;; name of visible characters, a colon and a value with no control character but the tab,
  ;; each on a line that ends with CR LF, up to the blank line. Gives 0 when one is not, or
  ;; when the head has no blank line. $length is at most HTTP_HEAD.
  (func $http_headers_valid (param $text i32) (param $length i32) (result i32)
    (local $start i32)
    (local $end i32)
    (local $i i32)
    (local $c i32)
    (local $valid i32)
    (local $lines i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 770)) unreachable))
    ;; The fields start after the request line.
    (local.set $start
      (i32.add (call $http_line_end (local.get $text) (i32.const 0) (local.get $length))
        (global.get $HTTP_PAIR)))
    (block $bad
      (block $ended
        (local.set $lines (global.get $HTTP_LINES))
        (loop $line  ;; bound: 2048
          (local.set $end
            (call $http_line_end (local.get $text) (local.get $start) (local.get $length)))
          ;; A line with no line end.
          (br_if $bad (i32.ge_u (local.get $end) (local.get $length)))
          ;; The blank line.
          (br_if $ended (i32.eq (local.get $end) (local.get $start)))
          ;; The name: visible characters up to the colon, and at least one.
          (local.set $i (local.get $start))
          (block $named
            (local.set $left (global.get $HTTP_HEAD))
            (loop $name  ;; bound: 4096
              (local.set $c (call $http_at (local.get $text) (local.get $i) (local.get $end)))
              (br_if $named (i32.eq (local.get $c) (global.get $HTTP_COLON)))
              (br_if $bad
                (i32.gt_u (i32.sub (local.get $c) (global.get $HTTP_FIRST_VISIBLE))
                  (global.get $HTTP_VISIBLE)))
              (local.set $i (i32.add (local.get $i) (i32.const 1)))
              (local.set $left (i32.sub (local.get $left) (i32.const 1)))
              (br_if $name (local.get $left)))
            (br $bad))
          (br_if $bad (i32.eq (local.get $i) (local.get $start)))
          ;; The value: the bytes after the colon, up to the line end.
          (block $valued
            (local.set $left (global.get $HTTP_HEAD))
            (loop $value  ;; bound: 4096
              (local.set $i (i32.add (local.get $i) (i32.const 1)))
              (br_if $valued (i32.ge_u (local.get $i) (local.get $end)))
              (local.set $c (call $http_at (local.get $text) (local.get $i) (local.get $end)))
              (br_if $bad
                (i32.and (i32.lt_u (local.get $c) (global.get $HTTP_SPACE))
                  (i32.ne (local.get $c) (global.get $HTTP_TAB))))
              (br_if $bad (i32.eq (local.get $c) (global.get $HTTP_DELETE)))
              (local.set $left (i32.sub (local.get $left) (i32.const 1)))
              (br_if $value (local.get $left))))
          (local.set $start (i32.add (local.get $end) (global.get $HTTP_PAIR)))
          (local.set $lines (i32.sub (local.get $lines) (i32.const 1)))
          (br_if $line (local.get $lines)))
        ;; More lines than a head of HTTP_HEAD bytes has room for.
        (call $assert_fail (i32.const 771))
        unreachable)
      (local.set $valid (i32.const 1)))
    (local.get $valid))

  ;; Gives 1 when the field's name $name, of $name_length small letters, digits and hyphens,
  ;; stands at place $from of the text at $text with a colon after it, where a capital letter
  ;; of the text counts as its small one. Else gives 0. $length is at most HTTP_HEAD and
  ;; $name_length at most HTTP_NAME. Its caller lends $name with room for HTTP_NAME bytes.
  (func $http_name_is (param $text i32) (param $from i32) (param $length i32)
    (param $name i32) (param $name_length i32) (result i32)
    (local $k i32)
    (local $c i32)
    (local $same i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 772)) unreachable))
    (if (i32.gt_u (local.get $name_length) (global.get $HTTP_NAME))
      (then (call $assert_fail (i32.const 773)) unreachable))
    (block $differs
      (block $compared
        (local.set $left (global.get $HTTP_NAME))
        (loop $byte  ;; bound: 32
          (br_if $compared (i32.ge_u (local.get $k) (local.get $name_length)))
          (if (i32.ge_u (local.get $k) (global.get $HTTP_NAME))
            (then (call $assert_fail (i32.const 774)) unreachable))
          (local.set $c
            (call $http_at (local.get $text) (i32.add (local.get $from) (local.get $k))
              (local.get $length)))
          (if
            (i32.le_u (i32.sub (local.get $c) (global.get $HTTP_BIG_A))
              (global.get $HTTP_LETTERS))
            (then (local.set $c (i32.or (local.get $c) (global.get $HTTP_CASE_BIT)))))
          (br_if $differs
            (i32.ne (local.get $c) (i32.load8_u (i32.add (local.get $name) (local.get $k)))))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $byte (local.get $left))))
      (br_if $differs
        (i32.ne
          (call $http_at (local.get $text) (i32.add (local.get $from) (local.get $name_length))
            (local.get $length))
          (global.get $HTTP_COLON)))
      (local.set $same (i32.const 1)))
    (local.get $same))

  ;; Gives the place of the first byte that is no space and no tab from place $from of the
  ;; text at $text, or $end when every byte up to there is one. $from is at most $end, and
  ;; $end at most HTTP_HEAD.
  (func $http_blank_end (param $text i32) (param $from i32) (param $end i32) (result i32)
    (local $i i32)
    (local $c i32)
    (local $left i32)
    (if (i32.gt_u (local.get $end) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 775)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $end))
      (then (call $assert_fail (i32.const 776)) unreachable))
    (local.set $i (local.get $from))
    (block $done
      (local.set $left (global.get $HTTP_HEAD))
      (loop $byte  ;; bound: 4096
        (local.set $c (call $http_at (local.get $text) (local.get $i) (local.get $end)))
        (br_if $done
          (i32.and (i32.ne (local.get $c) (global.get $HTTP_SPACE))
            (i32.ne (local.get $c) (global.get $HTTP_TAB))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $i))

  ;; Gives the place where the spaces and tabs at the end of the bytes from place $from to
  ;; place $end of the text at $text start: $end when the last byte is neither, and $from when
  ;; every byte is one. $from is at most $end, and $end at most HTTP_HEAD.
  (func $http_blank_start (param $text i32) (param $from i32) (param $end i32) (result i32)
    (local $i i32)
    (local $c i32)
    (local $left i32)
    (if (i32.gt_u (local.get $end) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 777)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $end))
      (then (call $assert_fail (i32.const 778)) unreachable))
    (local.set $i (local.get $end))
    (block $done
      (local.set $left (global.get $HTTP_HEAD))
      (loop $byte  ;; bound: 4096
        (br_if $done (i32.le_u (local.get $i) (local.get $from)))
        (local.set $c
          (call $http_at (local.get $text) (i32.sub (local.get $i) (i32.const 1))
            (local.get $end)))
        (br_if $done
          (i32.and (i32.ne (local.get $c) (global.get $HTTP_SPACE))
            (i32.ne (local.get $c) (global.get $HTTP_TAB))))
        (local.set $i (i32.sub (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $i))

  ;; Finds the first header field named $name, of $name_length bytes, in the head in the
  ;; first $length bytes at $text, as $http_name_is compares names. Writes at $into, 4 bytes
  ;; each, where its value starts and the value's length, without the spaces and tabs around
  ;; it, and gives 1. Gives 0, and writes nothing, when the head has no such field. $length is
  ;; at most HTTP_HEAD. Its caller lends $name with room for HTTP_NAME bytes.
  (func $http_header (param $text i32) (param $length i32) (param $name i32)
    (param $name_length i32) (param $into i32) (result i32)
    (local $start i32)
    (local $end i32)
    (local $found i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 779)) unreachable))
    ;; The fields start after the request line.
    (local.set $start
      (i32.add (call $http_line_end (local.get $text) (i32.const 0) (local.get $length))
        (global.get $HTTP_PAIR)))
    (block $done
      (local.set $left (global.get $HTTP_LINES))
      (loop $line  ;; bound: 2048
        (local.set $end
          (call $http_line_end (local.get $text) (local.get $start) (local.get $length)))
        ;; The blank line, or text with no line end, ends the fields.
        (br_if $done (i32.le_u (local.get $end) (local.get $start)))
        (br_if $done (i32.ge_u (local.get $end) (local.get $length)))
        (if
          (call $http_name_is (local.get $text) (local.get $start) (local.get $length)
            (local.get $name) (local.get $name_length))
          (then
            ;; The value: after the name and its colon, to the line end.
            (local.set $start
              (i32.add (i32.add (local.get $start) (local.get $name_length)) (i32.const 1)))
            (local.set $start
              (call $http_blank_end (local.get $text) (local.get $start) (local.get $end)))
            (local.set $end
              (call $http_blank_start (local.get $text) (local.get $start) (local.get $end)))
            (if (i32.lt_u (local.get $end) (local.get $start))
              (then (call $assert_fail (i32.const 780)) unreachable))
            (i32.store (local.get $into) (local.get $start))
            (i32.store offset=4 (local.get $into) (i32.sub (local.get $end) (local.get $start)))
            (local.set $found (i32.const 1))
            (br $done)))
        (local.set $start (i32.add (local.get $end) (global.get $HTTP_PAIR)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $line (local.get $left))))
    (local.get $found))

  ;; Gives the value of the hexadecimal digit $c, 0 to 15, or HTTP_NO_DIGIT when $c is none.
  (func $http_hex (param $c i32) (result i32)
    (local $value i32)
    (local $letter i32)
    (local.set $value (global.get $HTTP_NO_DIGIT))
    (if (i32.le_u (i32.sub (local.get $c) (global.get $HTTP_ZERO)) (global.get $HTTP_NINE))
      (then (local.set $value (i32.sub (local.get $c) (global.get $HTTP_ZERO)))))
    ;; A letter from a to f, small or capital.
    (local.set $letter
      (i32.sub (i32.or (local.get $c) (global.get $HTTP_CASE_BIT)) (global.get $HTTP_SMALL_A)))
    (if (i32.le_u (local.get $letter) (global.get $HTTP_FIVE))
      (then (local.set $value (i32.add (local.get $letter) (global.get $HTTP_TEN)))))
    (local.get $value))

  ;; Decodes the path in the bytes from place $from to place $end of the text at $text, a
  ;; request's target: the bytes up to a question mark, each percent sign and its two
  ;; hexadecimal digits as the byte they name. Writes the path at $into and gives its length,
  ;; at most HTTP_PATH. Gives HTTP_NO when an escape isn't two hexadecimal digits, when a
  ;; byte of the path, decoded, is not a character that prints, or when the path is longer
  ;; than HTTP_PATH. $from is at most $end, and $end at most HTTP_HEAD. Its caller lends
  ;; $into with room for HTTP_PATH bytes.
  (func $http_path (param $text i32) (param $from i32) (param $end i32) (param $into i32)
    (result i32)
    (local $i i32)
    (local $o i32)
    (local $c i32)
    (local $high i32)
    (local $low i32)
    (local $result i32)
    (local $left i32)
    (if (i32.gt_u (local.get $end) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 781)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $end))
      (then (call $assert_fail (i32.const 782)) unreachable))
    (local.set $result (global.get $HTTP_NO))
    (local.set $i (local.get $from))
    (block $bad
      (block $ended
        (local.set $left (global.get $HTTP_HEAD))
        (loop $byte  ;; bound: 4096
          (local.set $c (call $http_at (local.get $text) (local.get $i) (local.get $end)))
          (br_if $ended (i32.eq (local.get $c) (global.get $HTTP_NONE)))
          (br_if $ended (i32.eq (local.get $c) (global.get $HTTP_QUESTION)))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (if (i32.eq (local.get $c) (global.get $HTTP_PERCENT))
            (then
              (local.set $high
                (call $http_hex
                  (call $http_at (local.get $text) (local.get $i) (local.get $end))))
              (local.set $low
                (call $http_hex
                  (call $http_at (local.get $text) (i32.add (local.get $i) (i32.const 1))
                    (local.get $end))))
              (br_if $bad
                (i32.ge_u (i32.or (local.get $high) (local.get $low))
                  (global.get $HTTP_NO_DIGIT)))
              (local.set $c
                (i32.or (i32.shl (local.get $high) (global.get $HTTP_HEX_SHIFT))
                  (local.get $low)))
              (local.set $i (i32.add (local.get $i) (global.get $HTTP_PAIR)))))
          ;; Only a character that prints, from the space to the tilde.
          (br_if $bad
            (i32.gt_u (i32.sub (local.get $c) (global.get $HTTP_SPACE))
              (global.get $HTTP_PRINTS)))
          (br_if $bad (i32.ge_u (local.get $o) (global.get $HTTP_PATH)))
          (i32.store8 (i32.add (local.get $into) (local.get $o)) (local.get $c))
          (local.set $o (i32.add (local.get $o) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $byte (local.get $left))))
      (local.set $result (local.get $o)))
    (local.get $result))
)
