;; json: a part. Its idea's contract offers $json_minify, which checks that a text is one JSON
;; value and writes it with no space between its tokens, and the functions it is made of: each
;; works only through the addresses it is lent and reads only the $JSON_ constants below. The
;; part's own program prints the JSON value on standard input in that short form, so its tests
;; and its proofs go through every function. In the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md).
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Descriptors.
  (global $STDIN i32 (i32.const 0))
  (global $STDOUT i32 (i32.const 1))
  (global $STDERR i32 (i32.const 2))
  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  (global $EXIT_NOT_JSON i32 (i32.const 1))
  (global $EXIT_TOO_LONG i32 (i32.const 2))
  (global $EXIT_READ_FAILED i32 (i32.const 3))
  (global $EXIT_WRITE_FAILED i32 (i32.const 4))
  ;; $fill reads to the input's end.
  (global $READ_TO_END i32 (i32.const -1))
  (global $NEWLINE i32 (i32.const 10))

  ;; The part's constants, which its functions read. A program that takes a function defines
  ;; each constant the function reads as it is here.
  ;; The most bytes a JSON text has, and the last place a word of four bytes may start.
  (global $JSON_TEXT i32 (i32.const 1024))
  (global $JSON_LAST_WORD i32 (i32.const 1020))
  ;; What a function gives for text that isn't JSON: no token ends at 0.
  (global $JSON_BAD i32 (i32.const 0))
  ;; What $json_at gives past the text's end: above every byte.
  (global $JSON_NONE i32 (i32.const 256))
  ;; What $json_minify waits for next: a value; a value or the array's end; a member's name; a
  ;; name or the object's end; a colon; a comma or the end of the array or object; nothing more.
  (global $JSON_WANT_VALUE i32 (i32.const 0))
  (global $JSON_WANT_VALUE_OR_END i32 (i32.const 1))
  (global $JSON_WANT_NAME i32 (i32.const 2))
  (global $JSON_WANT_NAME_OR_END i32 (i32.const 3))
  (global $JSON_WANT_COLON i32 (i32.const 4))
  (global $JSON_WANT_MORE i32 (i32.const 5))
  (global $JSON_DONE i32 (i32.const 6))
  ;; The open arrays and objects, a bit each, 1 for an object, below a bit that marks the top:
  ;; none open, and the least that holds more than 32.
  (global $JSON_NONE_OPEN i64 (i64.const 1))
  (global $JSON_TOO_DEEP i64 (i64.const 8589934592))
  (global $JSON_ONE i64 (i64.const 1))
  ;; Where a text stands, as one number: what is open, above the three bits of what it waits
  ;; for. At the start nothing is open and a value is due.
  (global $JSON_WAIT_BITS i64 (i64.const 7))
  (global $JSON_WAIT_SHIFT i64 (i64.const 3))
  (global $JSON_START i64 (i64.const 8))
  ;; What $json_structure gives in place of where the text stands: a byte that can't come
  ;; here, a byte that isn't a token by itself, and a byte between tokens.
  (global $JSON_REFUSED i64 (i64.const -1))
  (global $JSON_OTHER i64 (i64.const -2))
  (global $JSON_BLANK i64 (i64.const -3))
  ;; Characters.
  (global $JSON_TAB i32 (i32.const 9))
  (global $JSON_LINE_FEED i32 (i32.const 10))
  (global $JSON_RETURN i32 (i32.const 13))
  (global $JSON_SPACE i32 (i32.const 32))
  (global $JSON_QUOTE i32 (i32.const 34))
  (global $JSON_PLUS i32 (i32.const 43))
  (global $JSON_COMMA i32 (i32.const 44))
  (global $JSON_MINUS i32 (i32.const 45))
  (global $JSON_POINT i32 (i32.const 46))
  (global $JSON_SLASH i32 (i32.const 47))
  (global $JSON_ZERO i32 (i32.const 48))
  (global $JSON_COLON i32 (i32.const 58))
  (global $JSON_OPEN_BRACKET i32 (i32.const 91))
  (global $JSON_BACKSLASH i32 (i32.const 92))
  (global $JSON_CLOSE_BRACKET i32 (i32.const 93))
  (global $JSON_SMALL_A i32 (i32.const 97))
  (global $JSON_SMALL_B i32 (i32.const 98))
  (global $JSON_SMALL_E i32 (i32.const 101))
  (global $JSON_SMALL_F i32 (i32.const 102))
  (global $JSON_SMALL_N i32 (i32.const 110))
  (global $JSON_SMALL_R i32 (i32.const 114))
  (global $JSON_SMALL_T i32 (i32.const 116))
  (global $JSON_SMALL_U i32 (i32.const 117))
  (global $JSON_OPEN_BRACE i32 (i32.const 123))
  (global $JSON_CLOSE_BRACE i32 (i32.const 125))
  ;; How far the digit 9 is from 0, and the letter f from a; the bit that makes a letter small;
  ;; the hexadecimal digits after \u.
  (global $JSON_NINE i32 (i32.const 9))
  (global $JSON_FIVE i32 (i32.const 5))
  (global $JSON_CASE_BIT i32 (i32.const 32))
  (global $JSON_HEX_DIGITS i32 (i32.const 4))
  ;; The words true and null, and the first four bytes of false, each as the four bytes a load
  ;; gives, the first byte lowest.
  (global $JSON_TRUE i32 (i32.const 0x65757274))
  (global $JSON_NULL i32 (i32.const 0x6c6c756e))
  (global $JSON_FALS i32 (i32.const 0x736c6166))
  (global $JSON_WORD i32 (i32.const 4))

  ;; Regions (rule 3, M1), from 1024 up: the messages, read-only, then the input and the output.
  (global $not_json i32 (i32.const 1024))
  (global $not_json.rosize i32 (i32.const 16))
  (data (i32.const 1024) "error: not JSON\0a")
  (global $too_long i32 (i32.const 1056))
  (global $too_long.rosize i32 (i32.const 22))
  (data (i32.const 1056) "error: input too long\0a")
  ;; Standard input, which main lends $json_minify to read: JSON_TEXT bytes, and one more to see
  ;; input that is too long.
  (global $text i32 (i32.const 2048))
  (global $text.size i32 (i32.const 1025))
  ;; The short form, which main lends $json_minify to write: JSON_TEXT bytes, then a newline.
  (global $out i32 (i32.const 4096))
  (global $out.size i32 (i32.const 1025))

  ;; Reads standard input, and prints the JSON value it holds in its short form, or says why it
  ;; can't. Never returns: every path ends in proc_exit.
  (func $main (export "_start")
    (local $got i32)
    (local $length i32)
    (block $write_failed
      (block $read_failed
        (block $too_long
          (block $not_json
            (local.set $got
              (call $fill (global.get $STDIN) (global.get $text) (global.get $text.size)
                (global.get $READ_TO_END)))
            (br_if $read_failed (i32.lt_s (local.get $got) (i32.const 0)))
            (if (i32.gt_u (local.get $got) (global.get $text.size))
              (then (call $assert_fail (i32.const 1)) unreachable))
            (br_if $too_long (i32.gt_u (local.get $got) (global.get $JSON_TEXT)))
            (local.set $length
              (call $json_minify (global.get $text) (local.get $got) (global.get $out)))
            (br_if $not_json (i32.eq (local.get $length) (global.get $JSON_BAD)))
            (if (i32.gt_u (local.get $length) (global.get $JSON_TEXT))
              (then (call $assert_fail (i32.const 2)) unreachable))
            (i32.store8 (i32.add (global.get $out) (local.get $length)) (global.get $NEWLINE))
            (br_if $write_failed
              (call $write_all (global.get $STDOUT) (global.get $out)
                (i32.add (local.get $length) (i32.const 1))))
            (call $proc_exit (global.get $EXIT_OK))
            unreachable)
          (br_if $write_failed
            (call $write_all (global.get $STDERR) (global.get $not_json)
              (global.get $not_json.rosize)))
          (call $proc_exit (global.get $EXIT_NOT_JSON))
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

  ;; Checks that the first $length bytes at $text are one JSON value, with only spaces, tabs
  ;; and line ends around its tokens, and writes the value at $into with none of those. Gives
  ;; how many bytes it wrote, at least 1 and never more than $length, or JSON_BAD when the text
  ;; isn't one JSON value or has more than 32 arrays and objects open at once. $length is at
  ;; most JSON_TEXT. Its caller lends $text and $into with room for JSON_TEXT bytes each.
  (func $json_minify (param $text i32) (param $length i32) (param $into i32) (result i32)
    (local $i i32)
    (local $o i32)
    (local $c i32)
    (local $stands i64)
    (local $next i64)
    (local $wait i32)
    (local $end i32)
    (local $result i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $JSON_TEXT))
      (then (call $assert_fail (i32.const 861)) unreachable))
    (local.set $stands (global.get $JSON_START))
    (block $bad
      (block $ended
        (local.set $left (global.get $JSON_TEXT))
        (loop $token  ;; bound: 1024
          (br_if $ended (i32.ge_u (local.get $i) (local.get $length)))
          (local.set $c (call $json_at (local.get $text) (local.get $i) (local.get $length)))
          (local.set $end (i32.add (local.get $i) (i32.const 1)))
          (block $next_token
            (block $copy
              (local.set $next (call $json_structure (local.get $c) (local.get $stands)))
              (br_if $bad (i64.eq (local.get $next) (global.get $JSON_REFUSED)))
              ;; A space, a tab or a line end between tokens is left out.
              (if (i64.eq (local.get $next) (global.get $JSON_BLANK))
                (then
                  (local.set $i (local.get $end))
                  (br $next_token)))
              ;; A token of one byte.
              (if (i64.ne (local.get $next) (global.get $JSON_OTHER))
                (then
                  (local.set $stands (local.get $next))
                  (br $copy)))
              ;; A token of several bytes: a string, a number or a word where a value is due,
              ;; and a string where a name is.
              (local.set $wait
                (i32.wrap_i64 (i64.and (local.get $stands) (global.get $JSON_WAIT_BITS))))
              (if (i32.le_u (local.get $wait) (global.get $JSON_WANT_VALUE_OR_END))
                (then
                  (local.set $end
                    (call $json_scalar_end (local.get $text) (local.get $i) (local.get $length)))
                  ;; The value is whole when nothing is open.
                  (local.set $wait (global.get $JSON_WANT_MORE))
                  (if
                    (i64.eq (i64.shr_u (local.get $stands) (global.get $JSON_WAIT_SHIFT))
                      (global.get $JSON_NONE_OPEN))
                    (then (local.set $wait (global.get $JSON_DONE)))))
                (else
                  (br_if $bad (i32.gt_u (local.get $wait) (global.get $JSON_WANT_NAME_OR_END)))
                  (local.set $end
                    (call $json_string_end (local.get $text) (local.get $i) (local.get $length)))
                  (local.set $wait (global.get $JSON_WANT_COLON))))
              (br_if $bad (i32.eq (local.get $end) (global.get $JSON_BAD)))
              (local.set $stands
                (i64.or
                  (i64.shl (i64.shr_u (local.get $stands) (global.get $JSON_WAIT_SHIFT))
                    (global.get $JSON_WAIT_SHIFT))
                  (i64.extend_i32_u (local.get $wait)))))
            ;; The token, from $i to $end, goes out as it is.
            (local.set $o
              (call $json_copy (local.get $text) (local.get $i) (local.get $end)
                (local.get $into) (local.get $o)))
            (local.set $i (local.get $end)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $token (local.get $left))))
      ;; Each round took at least one byte, so the text has ended; its value must have too.
      (br_if $bad (i32.lt_u (local.get $i) (local.get $length)))
      (br_if $bad
        (i32.ne (i32.wrap_i64 (i64.and (local.get $stands) (global.get $JSON_WAIT_BITS)))
          (global.get $JSON_DONE)))
      (if (i32.gt_u (local.get $o) (local.get $length))
        (then (call $assert_fail (i32.const 862)) unreachable))
      (local.set $result (local.get $o)))
    (local.get $result))

  ;; Says what the byte $c does where the text stands, $stands. For a brace, a bracket, a
  ;; comma or a colon that may come there, it gives where the text stands after it. It gives
  ;; JSON_REFUSED for one that may not, and for an array or an object inside 32 others;
  ;; JSON_BLANK for a space, a tab or a line end; and JSON_OTHER for any other byte.
  (func $json_structure (param $c i32) (param $stands i64) (result i64)
    (local $wait i32)
    (local $open i64)
    (local $closer i32)
    (local $fresh i32)
    (local $result i64)
    (local.set $wait (i32.wrap_i64 (i64.and (local.get $stands) (global.get $JSON_WAIT_BITS))))
    (local.set $open (i64.shr_u (local.get $stands) (global.get $JSON_WAIT_SHIFT)))
    (local.set $result (global.get $JSON_REFUSED))
    (block $done
      (block $made
        (if
          (i32.or
            (i32.or (i32.eq (local.get $c) (global.get $JSON_SPACE))
              (i32.eq (local.get $c) (global.get $JSON_TAB)))
            (i32.or (i32.eq (local.get $c) (global.get $JSON_LINE_FEED))
              (i32.eq (local.get $c) (global.get $JSON_RETURN))))
          (then
            (local.set $result (global.get $JSON_BLANK))
            (br $done)))
        (if (i32.eq (local.get $c) (global.get $JSON_COLON))
          (then
            (br_if $done (i32.ne (local.get $wait) (global.get $JSON_WANT_COLON)))
            (local.set $wait (global.get $JSON_WANT_VALUE))
            (br $made)))
        (if (i32.eq (local.get $c) (global.get $JSON_COMMA))
          (then
            (br_if $done (i32.ne (local.get $wait) (global.get $JSON_WANT_MORE)))
            ;; After a comma, an object takes a name and an array a value.
            (local.set $wait (global.get $JSON_WANT_VALUE))
            (if (i32.wrap_i64 (i64.and (local.get $open) (global.get $JSON_ONE)))
              (then (local.set $wait (global.get $JSON_WANT_NAME))))
            (br $made)))
        (if
          (i32.or (i32.eq (local.get $c) (global.get $JSON_OPEN_BRACE))
            (i32.eq (local.get $c) (global.get $JSON_OPEN_BRACKET)))
          (then
            ;; One opens only where a value is due: one more bit, 1 for an object.
            (br_if $done (i32.gt_u (local.get $wait) (global.get $JSON_WANT_VALUE_OR_END)))
            (local.set $open (i64.shl (local.get $open) (global.get $JSON_ONE)))
            (br_if $done (i64.ge_u (local.get $open) (global.get $JSON_TOO_DEEP)))
            (local.set $wait (global.get $JSON_WANT_VALUE_OR_END))
            (if (i32.eq (local.get $c) (global.get $JSON_OPEN_BRACE))
              (then
                (local.set $open (i64.or (local.get $open) (global.get $JSON_ONE)))
                (local.set $wait (global.get $JSON_WANT_NAME_OR_END))))
            (br $made)))
        (local.set $result (global.get $JSON_OTHER))
        (br_if $done
          (i32.and (i32.ne (local.get $c) (global.get $JSON_CLOSE_BRACE))
            (i32.ne (local.get $c) (global.get $JSON_CLOSE_BRACKET))))
        ;; $c must end the array or the object that was opened last, right after it opened or
        ;; after one of its values.
        (local.set $result (global.get $JSON_REFUSED))
        (br_if $done (i64.eq (local.get $open) (global.get $JSON_NONE_OPEN)))
        (local.set $closer (global.get $JSON_CLOSE_BRACKET))
        (local.set $fresh (global.get $JSON_WANT_VALUE_OR_END))
        (if (i32.wrap_i64 (i64.and (local.get $open) (global.get $JSON_ONE)))
          (then
            (local.set $closer (global.get $JSON_CLOSE_BRACE))
            (local.set $fresh (global.get $JSON_WANT_NAME_OR_END))))
        (br_if $done (i32.ne (local.get $c) (local.get $closer)))
        (br_if $done
          (i32.and (i32.ne (local.get $wait) (local.get $fresh))
            (i32.ne (local.get $wait) (global.get $JSON_WANT_MORE))))
        (local.set $open (i64.shr_u (local.get $open) (global.get $JSON_ONE)))
        (local.set $wait (global.get $JSON_WANT_MORE))
        (if (i64.eq (local.get $open) (global.get $JSON_NONE_OPEN))
          (then (local.set $wait (global.get $JSON_DONE)))))
      (local.set $result
        (i64.or (i64.shl (local.get $open) (global.get $JSON_WAIT_SHIFT))
          (i64.extend_i32_u (local.get $wait)))))
    (local.get $result))

  ;; Gives the byte at place $i of the text at $text, or JSON_NONE when $i is at or past
  ;; $length. $length is at most JSON_TEXT.
  (func $json_at (param $text i32) (param $i i32) (param $length i32) (result i32)
    (local $c i32)
    (if (i32.gt_u (local.get $length) (global.get $JSON_TEXT))
      (then (call $assert_fail (i32.const 863)) unreachable))
    (local.set $c (global.get $JSON_NONE))
    (if (i32.lt_u (local.get $i) (local.get $length))
      (then
        (if (i32.ge_u (local.get $i) (global.get $JSON_TEXT))
          (then (call $assert_fail (i32.const 864)) unreachable))
        (local.set $c (i32.load8_u (i32.add (local.get $text) (local.get $i))))))
    (local.get $c))

  ;; Copies the bytes from place $from to place $end of the text at $text to place $o of
  ;; $into, and gives the place after the last byte it wrote there. $from is at most $end, and
  ;; $end and what $o comes to are at most JSON_TEXT.
  (func $json_copy (param $text i32) (param $from i32) (param $end i32) (param $into i32)
    (param $o i32) (result i32)
    (local $i i32)
    (local $left i32)
    (if (i32.gt_u (local.get $end) (global.get $JSON_TEXT))
      (then (call $assert_fail (i32.const 865)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $end))
      (then (call $assert_fail (i32.const 866)) unreachable))
    (local.set $i (local.get $from))
    (block $done
      (local.set $left (global.get $JSON_TEXT))
      (loop $byte  ;; bound: 1024
        (br_if $done (i32.ge_u (local.get $i) (local.get $end)))
        (if (i32.ge_u (local.get $i) (global.get $JSON_TEXT))
          (then (call $assert_fail (i32.const 867)) unreachable))
        (if (i32.ge_u (local.get $o) (global.get $JSON_TEXT))
          (then (call $assert_fail (i32.const 868)) unreachable))
        (i32.store8 (i32.add (local.get $into) (local.get $o))
          (i32.load8_u (i32.add (local.get $text) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $o (i32.add (local.get $o) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $o))

  ;; Gives the place after the string, the number or the word that starts at place $from of
  ;; the text at $text, or JSON_BAD when none starts there. $length is at most JSON_TEXT.
  (func $json_scalar_end (param $text i32) (param $from i32) (param $length i32) (result i32)
    (local $c i32)
    (local $end i32)
    (if (i32.gt_u (local.get $length) (global.get $JSON_TEXT))
      (then (call $assert_fail (i32.const 869)) unreachable))
    (local.set $c (call $json_at (local.get $text) (local.get $from) (local.get $length)))
    (block $found
      (if (i32.eq (local.get $c) (global.get $JSON_QUOTE))
        (then
          (local.set $end
            (call $json_string_end (local.get $text) (local.get $from) (local.get $length)))
          (br $found)))
      ;; Below '0' wraps to a large number, so one unsigned test finds a digit.
      (if
        (i32.or (i32.eq (local.get $c) (global.get $JSON_MINUS))
          (i32.le_u (i32.sub (local.get $c) (global.get $JSON_ZERO)) (global.get $JSON_NINE)))
        (then
          (local.set $end
            (call $json_number_end (local.get $text) (local.get $from) (local.get $length)))
          (br $found)))
      (local.set $end
        (call $json_word_end (local.get $text) (local.get $from) (local.get $length))))
    (if (i32.gt_u (local.get $end) (local.get $length))
      (then (call $assert_fail (i32.const 878)) unreachable))
    (local.get $end))

  ;; Gives the place after the string whose first quote is at place $from of the text at
  ;; $text, or JSON_BAD when no string is there: no quote, a control character, an escape that
  ;; JSON doesn't have, or no quote at the end. $length is at most JSON_TEXT.
  (func $json_string_end (param $text i32) (param $from i32) (param $length i32) (result i32)
    (local $i i32)
    (local $c i32)
    (local $end i32)
    (local $digits i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $JSON_TEXT))
      (then (call $assert_fail (i32.const 870)) unreachable))
    (block $done
      (br_if $done
        (i32.ne (call $json_at (local.get $text) (local.get $from) (local.get $length))
          (global.get $JSON_QUOTE)))
      (local.set $i (i32.add (local.get $from) (i32.const 1)))
      (local.set $left (global.get $JSON_TEXT))
      (loop $byte  ;; bound: 1024
        (local.set $c (call $json_at (local.get $text) (local.get $i) (local.get $length)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (if (i32.eq (local.get $c) (global.get $JSON_QUOTE))
          (then
            (local.set $end (local.get $i))
            (br $done)))
        ;; A control character, or the text's end.
        (br_if $done (i32.lt_u (local.get $c) (global.get $JSON_SPACE)))
        (br_if $done (i32.eq (local.get $c) (global.get $JSON_NONE)))
        (if (i32.eq (local.get $c) (global.get $JSON_BACKSLASH))
          (then
            (local.set $c (call $json_at (local.get $text) (local.get $i) (local.get $length)))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (if (i32.eq (local.get $c) (global.get $JSON_SMALL_U))
              (then
                (local.set $digits (global.get $JSON_HEX_DIGITS))
                (loop $digit  ;; bound: 4
                  (local.set $c
                    (call $json_at (local.get $text) (local.get $i) (local.get $length)))
                  ;; A digit, or a letter from a to f, small or capital.
                  (br_if $done
                    (i32.eqz
                      (i32.or
                        (i32.le_u (i32.sub (local.get $c) (global.get $JSON_ZERO))
                          (global.get $JSON_NINE))
                        (i32.le_u
                          (i32.sub (i32.or (local.get $c) (global.get $JSON_CASE_BIT))
                            (global.get $JSON_SMALL_A))
                          (global.get $JSON_FIVE)))))
                  (local.set $i (i32.add (local.get $i) (i32.const 1)))
                  (local.set $digits (i32.sub (local.get $digits) (i32.const 1)))
                  (br_if $digit (local.get $digits))))
              (else
                (block $escape
                  (br_if $escape (i32.eq (local.get $c) (global.get $JSON_QUOTE)))
                  (br_if $escape (i32.eq (local.get $c) (global.get $JSON_BACKSLASH)))
                  (br_if $escape (i32.eq (local.get $c) (global.get $JSON_SLASH)))
                  (br_if $escape (i32.eq (local.get $c) (global.get $JSON_SMALL_B)))
                  (br_if $escape (i32.eq (local.get $c) (global.get $JSON_SMALL_F)))
                  (br_if $escape (i32.eq (local.get $c) (global.get $JSON_SMALL_N)))
                  (br_if $escape (i32.eq (local.get $c) (global.get $JSON_SMALL_R)))
                  (br_if $escape (i32.eq (local.get $c) (global.get $JSON_SMALL_T)))
                  (br $done))))))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (if (i32.gt_u (local.get $end) (local.get $length))
      (then (call $assert_fail (i32.const 871)) unreachable))
    (local.get $end))

  ;; Gives the place after the run of digits that starts at place $from of the text at $text:
  ;; $from itself when no digit is there. $length is at most JSON_TEXT.
  (func $json_digits_end (param $text i32) (param $from i32) (param $length i32) (result i32)
    (local $i i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $JSON_TEXT))
      (then (call $assert_fail (i32.const 872)) unreachable))
    (local.set $i (local.get $from))
    (block $done
      (local.set $left (global.get $JSON_TEXT))
      (loop $digit  ;; bound: 1024
        (br_if $done
          (i32.gt_u
            (i32.sub (call $json_at (local.get $text) (local.get $i) (local.get $length))
              (global.get $JSON_ZERO))
            (global.get $JSON_NINE)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left))))
    (if (i32.lt_u (local.get $i) (local.get $from))
      (then (call $assert_fail (i32.const 873)) unreachable))
    (local.get $i))

  ;; Gives the place after the number that starts at place $from of the text at $text, or
  ;; JSON_BAD when no number is there: an optional minus sign; 0, or digits that don't start
  ;; with 0; an optional point and digits; an optional e or E, sign and digits. $length is at
  ;; most JSON_TEXT.
  (func $json_number_end (param $text i32) (param $from i32) (param $length i32) (result i32)
    (local $i i32)
    (local $c i32)
    (local $after i32)
    (local $end i32)
    (if (i32.gt_u (local.get $length) (global.get $JSON_TEXT))
      (then (call $assert_fail (i32.const 874)) unreachable))
    (local.set $i (local.get $from))
    (block $bad
      (if
        (i32.eq (call $json_at (local.get $text) (local.get $i) (local.get $length))
          (global.get $JSON_MINUS))
        (then (local.set $i (i32.add (local.get $i) (i32.const 1)))))
      ;; The whole part: 0 alone, or a run of digits.
      (local.set $after
        (call $json_digits_end (local.get $text) (local.get $i) (local.get $length)))
      (br_if $bad (i32.eq (local.get $after) (local.get $i)))
      (if
        (i32.eq (call $json_at (local.get $text) (local.get $i) (local.get $length))
          (global.get $JSON_ZERO))
        (then (br_if $bad (i32.ne (local.get $after) (i32.add (local.get $i) (i32.const 1))))))
      (local.set $i (local.get $after))
      ;; A point, and then at least one digit.
      (if
        (i32.eq (call $json_at (local.get $text) (local.get $i) (local.get $length))
          (global.get $JSON_POINT))
        (then
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (local.set $after
            (call $json_digits_end (local.get $text) (local.get $i) (local.get $length)))
          (br_if $bad (i32.eq (local.get $after) (local.get $i)))
          (local.set $i (local.get $after))))
      ;; An exponent: e or E, an optional sign, and then at least one digit.
      (if
        (i32.eq
          (i32.or (call $json_at (local.get $text) (local.get $i) (local.get $length))
            (global.get $JSON_CASE_BIT))
          (global.get $JSON_SMALL_E))
        (then
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (local.set $c (call $json_at (local.get $text) (local.get $i) (local.get $length)))
          (if
            (i32.or (i32.eq (local.get $c) (global.get $JSON_PLUS))
              (i32.eq (local.get $c) (global.get $JSON_MINUS)))
            (then (local.set $i (i32.add (local.get $i) (i32.const 1)))))
          (local.set $after
            (call $json_digits_end (local.get $text) (local.get $i) (local.get $length)))
          (br_if $bad (i32.eq (local.get $after) (local.get $i)))
          (local.set $i (local.get $after))))
      (local.set $end (local.get $i)))
    (if (i32.gt_u (local.get $end) (local.get $length))
      (then (call $assert_fail (i32.const 875)) unreachable))
    (local.get $end))

  ;; Gives the place after the word true, false or null that starts at place $from of the text
  ;; at $text, or JSON_BAD when none of the three is there. $length is at most JSON_TEXT.
  (func $json_word_end (param $text i32) (param $from i32) (param $length i32) (result i32)
    (local $word i32)
    (local $end i32)
    (if (i32.gt_u (local.get $length) (global.get $JSON_TEXT))
      (then (call $assert_fail (i32.const 876)) unreachable))
    (block $bad
      ;; The shortest word has four bytes.
      (br_if $bad (i32.gt_u (local.get $from) (global.get $JSON_LAST_WORD)))
      (br_if $bad
        (i32.gt_u (i32.add (local.get $from) (global.get $JSON_WORD)) (local.get $length)))
      (local.set $word (i32.load (i32.add (local.get $text) (local.get $from))))
      (if
        (i32.or (i32.eq (local.get $word) (global.get $JSON_TRUE))
          (i32.eq (local.get $word) (global.get $JSON_NULL)))
        (then
          (local.set $end (i32.add (local.get $from) (global.get $JSON_WORD)))
          (br $bad)))
      (br_if $bad (i32.ne (local.get $word) (global.get $JSON_FALS)))
      (br_if $bad
        (i32.ne
          (call $json_at (local.get $text) (i32.add (local.get $from) (global.get $JSON_WORD))
            (local.get $length))
          (global.get $JSON_SMALL_E)))
      (local.set $end
        (i32.add (i32.add (local.get $from) (global.get $JSON_WORD)) (i32.const 1))))
    (if (i32.gt_u (local.get $end) (local.get $length))
      (then (call $assert_fail (i32.const 877)) unreachable))
    (local.get $end))
)
