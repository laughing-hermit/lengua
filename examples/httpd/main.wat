;; httpd: a web server for the folder it runs in, for up to 8 clients at once. It serves the
;; files of the folder public with their types, their ETags and parts of them, lists a folder,
;; and keeps a store of JSON values in the file store.json. In the WebAssembly dialect 1
;; (references/power-of-10-webassembly.md), in the shape of the compile skill's event-loop
;; server: one loop that never ends waits with poll_oneoff on every client's connection and,
;; while a slot is free, on the listener, descriptor 4. A request is answered once it is whole,
;; in one go, and its connection closed.
;;
;; It is built on four parts, whose functions stand at the end, each under its `;; from:` line:
;; http reads the head of a request, json checks a value and makes it short, decimal reads and
;; writes numbers, and crc32 computes the ETag.
(module
  (import "wasi_snapshot_preview1" "poll_oneoff"
    (func $poll_oneoff (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_accept"
    (func $sock_accept (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_recv"
    (func $sock_recv (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_filestat_get"
    (func $fd_filestat_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_readdir"
    (func $fd_readdir (param i32 i32 i32 i64 i32) (result i32)))
  (memory (export "memory") 23 23)

  ;; Descriptors: standard output, the folder the server runs in, and the listener, which
  ;; lengua hands a server as 4. Each connection takes the lowest number free after it.
  (global $STDOUT i32 (i32.const 1))
  (global $ROOT i32 (i32.const 3))
  (global $LISTENER i32 (i32.const 4))
  ;; poll_oneoff's records: a subscription of 48 bytes, its own number first, its kind at 8 and
  ;; its descriptor at 16, and an event of 32 bytes, its subscription's number first. A wait to
  ;; read is kind 1.
  (global $SUBSCRIPTION i32 (i32.const 48))
  (global $EVENT i32 (i32.const 32))
  (global $FD_READ i32 (i32.const 1))
  ;; The socket calls' flags and path_open's: none. sock_recv's buffers: one.
  (global $NO_FLAGS i32 (i32.const 0))
  (global $ONE_BUFFER i32 (i32.const 1))
  ;; path_open: a folder; a file made when it isn't there and emptied; the rights to read and
  ;; to write; none. What fd_filestat_get says a file and a folder are. fd_readdir from the
  ;; first name. $fill to the file's end.
  (global $OPEN_FOLDER i32 (i32.const 2))
  (global $OPEN_NEW i32 (i32.const 9))
  (global $RIGHT_READ i64 (i64.const 2))
  (global $RIGHT_WRITE i64 (i64.const 64))
  (global $NO_RIGHTS i64 (i64.const 0))
  (global $IS_FOLDER i32 (i32.const 3))
  (global $IS_FILE i32 (i32.const 4))
  (global $FROM_FIRST i64 (i64.const 0))
  (global $READ_TO_END i32 (i32.const -1))
  ;; Error numbers: no such file, and what store_save gives when it cannot write.
  (global $NOENT i32 (i32.const 44))
  (global $IO i32 (i32.const 29))

  ;; Sizes and limits. Up to CLIENTS connections at once, each with a slot of SLOT bytes for
  ;; its request: a head of HTTP_HEAD bytes and a body of BODY_MAX. A wait is on each client's
  ;; connection, and on the listener only while a slot is free, so on at most CLIENTS.
  (global $CLIENTS i32 (i32.const 8))
  (global $LISTENING i32 (i32.const 8))
  (global $FREE i32 (i32.const 0))
  (global $WORD i32 (i32.const 4))
  (global $PAIR i32 (i32.const 2))
  (global $SLOT i32 (i32.const 8192))
  (global $BODY_MAX i32 (i32.const 4096))
  (global $BODY_MAX_64 i64 (i64.const 4096))
  (global $BODY_TOO_LARGE i32 (i32.const 4097))
  ;; The longest literal text and its NUL.
  (global $TEXT_MAX i32 (i32.const 64))
  ;; The last place in $out where a number of DECIMAL_TEXT bytes may start.
  (global $OUT_NUMBER_LAST i32 (i32.const 8172))
  ;; A file: at most FILE_MAX bytes, read with room for one more to see a longer one; the
  ;; pieces of CRC32_TEXT bytes it has at most, and one.
  (global $FILE_MAX i32 (i32.const 1048576))
  (global $FILE_MAX_64 i64 (i64.const 1048576))
  (global $FILE_ROOM i32 (i32.const 1048577))
  (global $PIECES i32 (i32.const 257))
  ;; A folder's list: at most ENTRIES names of at most NAME_MAX bytes; an entry of fd_readdir
  ;; has 24 bytes before its name, the name's length at 16 and its kind at 20; and the last
  ;; place in $entries where an entry with the longest name may start.
  (global $ENTRIES i32 (i32.const 256))
  (global $ENTRIES_AND_ONE i32 (i32.const 257))
  (global $ENTRY_HEAD i32 (i32.const 24))
  (global $NAME_MAX i32 (i32.const 255))
  (global $NAME_ROOM i32 (i32.const 256))
  (global $ENTRY_LAST i32 (i32.const 73440))
  ;; The last place in $listing where a slash and a newline may start.
  (global $LISTING_LAST i32 (i32.const 69630))
  ;; The store: at most STORE_MAX bytes, read with room for one more; the least a store has,
  ;; a brace on a line and a brace on a line; a key of at most KEY_MAX bytes, and one more round
  ;; to see where a key ends.
  (global $STORE_MAX i32 (i32.const 65536))
  (global $STORE_ROOM i32 (i32.const 65537))
  (global $STORE_MIN i32 (i32.const 4))
  (global $KEY_MAX i32 (i32.const 64))
  (global $KEY_ROOM i32 (i32.const 65))
  ;; The lengths of /files, of public, of /kv/ and of store.json.
  (global $FILES_LENGTH i32 (i32.const 6))
  (global $FILES_SLASH_LENGTH i32 (i32.const 7))
  (global $KEY_PREFIX i32 (i32.const 4))
  (global $STORE_FILE_LENGTH i32 (i32.const 10))
  ;; An ETag in a request: a quote, 1 to 10 digits and a quote. A Range field: bytes= and at
  ;; least a digit and a hyphen.
  (global $TAG_MIN i32 (i32.const 3))
  (global $TAG_MAX i32 (i32.const 12))
  (global $RANGE_MIN i32 (i32.const 8))
  (global $RANGE_PREFIX i32 (i32.const 6))
  ;; The longest end of a file's name that names a type, and its point.
  (global $TYPE_ROOM i32 (i32.const 9))
  (global $BYTE_BITS i64 (i64.const 8))

  ;; What examine gives besides a status: the request is whole, or more of it must come.
  (global $WHOLE i32 (i32.const 0))
  (global $WAIT i32 (i32.const 1))
  ;; The statuses.
  (global $S_OK i32 (i32.const 200))
  (global $S_NO_CONTENT i32 (i32.const 204))
  (global $S_PARTIAL i32 (i32.const 206))
  (global $S_NOT_MODIFIED i32 (i32.const 304))
  (global $S_BAD_REQUEST i32 (i32.const 400))
  (global $S_NOT_FOUND i32 (i32.const 404))
  (global $S_METHOD i32 (i32.const 405))
  (global $S_LENGTH i32 (i32.const 411))
  (global $S_TOO_LARGE i32 (i32.const 413))
  (global $S_TARGET i32 (i32.const 414))
  (global $S_RANGE i32 (i32.const 416))
  (global $S_FIELDS i32 (i32.const 431))
  (global $S_ERROR i32 (i32.const 500))
  (global $S_NOT_IMPLEMENTED i32 (i32.const 501))
  (global $S_FULL i32 (i32.const 507))
  ;; The methods the routes take, and any other.
  (global $OTHER_METHOD i32 (i32.const 0))
  (global $GET i32 (i32.const 1))
  (global $PUT i32 (i32.const 2))
  (global $DELETE i32 (i32.const 3))
  (global $DELETE_LENGTH i32 (i32.const 6))
  ;; HEAD: a GET whose answer goes out with no body.
  (global $HEAD i32 (i32.const 4))
  (global $HEAD_LENGTH i32 (i32.const 4))
  ;; What content_length gives when the request has no such field, and for one that is no
  ;; number. What digits gives for no digits, and for text that is no number.
  (global $LENGTH_NONE i32 (i32.const -1))
  (global $LENGTH_BAD i32 (i32.const -2))
  (global $DIGITS_NONE i64 (i64.const -1))
  (global $DIGITS_BAD i64 (i64.const -2))
  ;; What file_read gives for what is no file, and for a file it cannot hold; list_read for a
  ;; folder it cannot list; store_load for a store that is not in its form.
  (global $NOT_A_FILE i32 (i32.const -1))
  (global $FILE_BROKEN i32 (i32.const -2))
  (global $LIST_BROKEN i32 (i32.const -1))
  (global $STORE_BROKEN i32 (i32.const -1))
  ;; What range_of gives: the whole file, a part of it, or no part.
  (global $RANGE_WHOLE i32 (i32.const 0))
  (global $RANGE_PART i32 (i32.const 1))
  (global $RANGE_NONE i32 (i32.const 2))
  ;; How a member's key stands to the key that is looked for.
  (global $ORDER_LESS i32 (i32.const 0))
  (global $ORDER_SAME i32 (i32.const 1))
  (global $ORDER_GREATER i32 (i32.const 2))
  ;; Words, each as the bytes a load gives, the first byte lowest: the methods with what
  ;; follows them; bytes= of a Range field; the store's first line, its last line, and an
  ;; empty store; and public, the folder of the files.
  (global $WORD_GET i32 (i32.const 0x20544547))
  (global $WORD_PUT i32 (i32.const 0x20545550))
  (global $WORD_DELE i32 (i32.const 0x454c4544))
  (global $WORD_TE i32 (i32.const 0x4554))
  (global $WORD_HEAD i32 (i32.const 0x44414548))
  (global $WORD_BYTE i32 (i32.const 0x65747962))
  (global $WORD_S_EQUALS i32 (i32.const 0x3d73))
  (global $WORD_STORE_START i32 (i32.const 0x0a7b))
  (global $WORD_STORE_END i32 (i32.const 0x0a7d))
  (global $WORD_EMPTY_STORE i32 (i32.const 0x0a7d0a7b))
  (global $WORD_PUBLIC i64 (i64.const 0x000063696c627570))
  ;; The ends of names, as $type_of reads them: the bytes after the last point, small, the
  ;; first lowest.
  (global $END_TXT i64 (i64.const 0x747874))
  (global $END_HTML i64 (i64.const 0x6c6d7468))
  (global $END_HTM i64 (i64.const 0x6d7468))
  (global $END_CSS i64 (i64.const 0x737363))
  (global $END_CSV i64 (i64.const 0x767363))
  (global $END_JS i64 (i64.const 0x736a))
  (global $END_JSON i64 (i64.const 0x6e6f736a))
  (global $END_WASM i64 (i64.const 0x6d736177))
  (global $END_SVG i64 (i64.const 0x677673))
  (global $END_PNG i64 (i64.const 0x676e70))
  (global $END_JPG i64 (i64.const 0x67706a))
  (global $END_JPEG i64 (i64.const 0x6765706a))
  (global $END_GIF i64 (i64.const 0x666967))
  ;; Characters.
  (global $NEWLINE i32 (i32.const 10))
  (global $SPACE i32 (i32.const 32))
  (global $QUOTE i32 (i32.const 34))
  (global $STAR i32 (i32.const 42))
  (global $COMMA i32 (i32.const 44))
  (global $HYPHEN i32 (i32.const 45))
  (global $POINT i32 (i32.const 46))
  (global $SLASH i32 (i32.const 47))
  (global $ZERO i32 (i32.const 48))
  (global $NINE i32 (i32.const 9))
  (global $BIG_A i32 (i32.const 65))
  (global $SMALL_A i32 (i32.const 97))
  (global $LETTERS i32 (i32.const 25))
  (global $CASE_BIT i32 (i32.const 32))
  (global $UNDERSCORE i32 (i32.const 95))

  ;; Where each literal text starts in $texts. Each ends with a NUL.
  (global $T_HTTP i32 (i32.const 0))
  (global $T_LINE_END i32 (i32.const 10))
  (global $T_TYPE i32 (i32.const 13))
  (global $T_LENGTH i32 (i32.const 30))
  (global $T_TAG i32 (i32.const 49))
  (global $T_TAG_END i32 (i32.const 57))
  (global $T_RANGE i32 (i32.const 61))
  (global $T_ANY i32 (i32.const 83))
  (global $T_CLOSE i32 (i32.const 86))
  (global $T_ALLOW_GET i32 (i32.const 108))
  (global $T_ALLOW_ALL i32 (i32.const 127))
  (global $T_NEWLINE i32 (i32.const 159))
  (global $T_SPACE i32 (i32.const 161))
  (global $T_HYPHEN i32 (i32.const 163))
  (global $T_SLASH i32 (i32.const 165))
  (global $T_NO_LINE i32 (i32.const 167))
  (global $T_OK i32 (i32.const 171))
  (global $T_COMMA i32 (i32.const 175))
  (global $T_COMMA_NEWLINE i32 (i32.const 177))
  (global $T_STORE_END i32 (i32.const 180))
  (global $T_NAME_END i32 (i32.const 184))
  (global $T_STORE_FILE i32 (i32.const 188))
  (global $P_HEALTH i32 (i32.const 199))
  (global $P_FILES i32 (i32.const 207))
  (global $P_STORE i32 (i32.const 215))
  (global $P_KEY i32 (i32.const 219))
  (global $R_200 i32 (i32.const 224))
  (global $R_204 i32 (i32.const 231))
  (global $R_206 i32 (i32.const 246))
  (global $R_304 i32 (i32.const 266))
  (global $R_400 i32 (i32.const 283))
  (global $R_404 i32 (i32.const 299))
  (global $R_405 i32 (i32.const 313))
  (global $R_411 i32 (i32.const 336))
  (global $R_413 i32 (i32.const 356))
  (global $R_414 i32 (i32.const 378))
  (global $R_416 i32 (i32.const 395))
  (global $R_431 i32 (i32.const 421))
  (global $R_500 i32 (i32.const 457))
  (global $R_501 i32 (i32.const 483))
  (global $R_505 i32 (i32.const 503))
  (global $R_507 i32 (i32.const 534))
  (global $M_PLAIN i32 (i32.const 559))
  (global $M_HTML i32 (i32.const 570))
  (global $M_CSS i32 (i32.const 580))
  (global $M_CSV i32 (i32.const 589))
  (global $M_SCRIPT i32 (i32.const 598))
  (global $M_JSON i32 (i32.const 614))
  (global $M_WASM i32 (i32.const 631))
  (global $M_BYTES i32 (i32.const 648))
  (global $M_SVG i32 (i32.const 673))
  (global $M_PNG i32 (i32.const 687))
  (global $M_JPEG i32 (i32.const 697))
  (global $M_GIF i32 (i32.const 708))
  ;; No type and no other field, for head_start and fail.
  (global $NO_TEXT i32 (i32.const 0))
  ;; Where each field's name starts in $names, and its length.
  (global $N_LENGTH i32 (i32.const 0))
  (global $N_LENGTH_LENGTH i32 (i32.const 14))
  (global $N_ENCODING i32 (i32.const 32))
  (global $N_ENCODING_LENGTH i32 (i32.const 17))
  (global $N_MATCH i32 (i32.const 64))
  (global $N_MATCH_LENGTH i32 (i32.const 13))
  (global $N_RANGE i32 (i32.const 96))
  (global $N_RANGE_LENGTH i32 (i32.const 5))

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

  ;; The part's constants, which its two functions read. A program that takes a function
  ;; defines each constant the function reads as it is here.
  ;; What $decimal_parse gives.
  (global $DECIMAL_OK i32 (i32.const 0))
  (global $DECIMAL_INVALID i32 (i32.const 28))
  ;; The most bytes a number's text has, a minus sign and 19 digits, and the most digits.
  (global $DECIMAL_TEXT i32 (i32.const 20))
  (global $DECIMAL_DIGITS i32 (i32.const 19))
  ;; The largest magnitude, and the largest that still takes one more digit.
  (global $DECIMAL_MAX i64 (i64.const 9223372036854775807))
  (global $DECIMAL_MAX_TENTH i64 (i64.const 922337203685477580))
  (global $DECIMAL_TEN i64 (i64.const 10))
  ;; Characters: the minus sign, the digit 0, and how far the digit 9 is from it.
  (global $DECIMAL_MINUS i32 (i32.const 45))
  (global $DECIMAL_ZERO i32 (i32.const 48))
  (global $DECIMAL_NINE i32 (i32.const 9))

  ;; The part's constants, which its function reads. A program that takes the function defines
  ;; each of them as it is here.
  ;; The most bytes one call takes.
  (global $CRC32_TEXT i32 (i32.const 4096))
  ;; The CRC-32's polynomial, its bits reflected, and the value the CRC is xored with at its
  ;; start and at its end: all ones.
  (global $CRC32_POLY i32 (i32.const 0xedb88320))
  (global $CRC32_ALL_ONES i32 (i32.const -1))
  ;; The bits of a byte.
  (global $CRC32_BITS i32 (i32.const 8))

  ;; Regions (rule 3, M1), from 1024 up. The literal texts, read-only, one after another.
  (global $texts i32 (i32.const 1024))
  (global $texts.rosize i32 (i32.const 718))
  (data (i32.const 1024) "HTTP/1.1 \00")
  (data (i32.const 1034) "\0d\0a\00")
  (data (i32.const 1037) "\0d\0aContent-Type: \00")
  (data (i32.const 1054) "\0d\0aContent-Length: \00")
  (data (i32.const 1073) "ETag: \22\00")
  (data (i32.const 1081) "\22\0d\0a\00")
  (data (i32.const 1085) "Content-Range: bytes \00")
  (data (i32.const 1107) "*/\00")
  (data (i32.const 1110) "Connection: close\0d\0a\0d\0a\00")
  (data (i32.const 1132) "Allow: GET, HEAD\0d\0a\00")
  (data (i32.const 1151) "Allow: GET, HEAD, PUT, DELETE\0d\0a\00")
  (data (i32.const 1183) "\0a\00")
  (data (i32.const 1185) " \00")
  (data (i32.const 1187) "-\00")
  (data (i32.const 1189) "/\00")
  (data (i32.const 1191) "- -\00")
  (data (i32.const 1195) "ok\0a\00")
  (data (i32.const 1199) ",\00")
  (data (i32.const 1201) ",\0a\00")
  (data (i32.const 1204) "\0a}\0a\00")
  (data (i32.const 1208) "\22: \00")
  (data (i32.const 1212) "store.json\00")
  (data (i32.const 1223) "/health\00")
  (data (i32.const 1231) "/files/\00")
  (data (i32.const 1239) "/kv\00")
  (data (i32.const 1243) "/kv/\00")
  (data (i32.const 1248) "200 OK\00")
  (data (i32.const 1255) "204 No Content\00")
  (data (i32.const 1270) "206 Partial Content\00")
  (data (i32.const 1290) "304 Not Modified\00")
  (data (i32.const 1307) "400 Bad Request\00")
  (data (i32.const 1323) "404 Not Found\00")
  (data (i32.const 1337) "405 Method Not Allowed\00")
  (data (i32.const 1360) "411 Length Required\00")
  (data (i32.const 1380) "413 Content Too Large\00")
  (data (i32.const 1402) "414 URI Too Long\00")
  (data (i32.const 1419) "416 Range Not Satisfiable\00")
  (data (i32.const 1445) "431 Request Header Fields Too Large\00")
  (data (i32.const 1481) "500 Internal Server Error\00")
  (data (i32.const 1507) "501 Not Implemented\00")
  (data (i32.const 1527) "505 HTTP Version Not Supported\00")
  (data (i32.const 1558) "507 Insufficient Storage\00")
  (data (i32.const 1583) "text/plain\00")
  (data (i32.const 1594) "text/html\00")
  (data (i32.const 1604) "text/css\00")
  (data (i32.const 1613) "text/csv\00")
  (data (i32.const 1622) "text/javascript\00")
  (data (i32.const 1638) "application/json\00")
  (data (i32.const 1655) "application/wasm\00")
  (data (i32.const 1672) "application/octet-stream\00")
  (data (i32.const 1697) "image/svg+xml\00")
  (data (i32.const 1711) "image/png\00")
  (data (i32.const 1721) "image/jpeg\00")
  (data (i32.const 1732) "image/gif\00")
  ;; The names of the header fields the server reads, in small letters, each in the HTTP_NAME
  ;; bytes that $http_name_is may read.
  (global $names i32 (i32.const 2048))
  (global $names.rosize i32 (i32.const 128))
  (data (i32.const 2048) "content-length\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")
  (data (i32.const 2080) "transfer-encoding\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")
  (data (i32.const 2112) "if-none-match\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")
  (data (i32.const 2144) "range\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")

  ;; What poll_oneoff waits on, and what it says happened.
  (global $subscriptions i32 (i32.const 2304))  ;; shared: subscribe writes it, main waits on it
  (global $subscriptions.size i32 (i32.const 384))
  (global $events i32 (i32.const 2752))
  (global $events.size i32 (i32.const 256))
  ;; How many events poll_oneoff wrote, and the descriptor sock_accept gave.
  (global $event_count i32 (i32.const 3040))
  (global $event_count.size i32 (i32.const 4))
  (global $accepted i32 (i32.const 3048))
  (global $accepted.size i32 (i32.const 4))
  ;; One buffer's address and length, for sock_recv, and what it writes.
  (global $iovec i32 (i32.const 3056))
  (global $iovec.size i32 (i32.const 8))
  (global $received i32 (i32.const 3064))
  (global $received.size i32 (i32.const 4))
  (global $received_flags i32 (i32.const 3072))
  (global $received_flags.size i32 (i32.const 2))
  ;; The descriptor path_open gave, and how many bytes fd_readdir wrote.
  (global $opened i32 (i32.const 3080))  ;; shared: each function that opens a file reads its descriptor here
  (global $opened.size i32 (i32.const 4))
  (global $listed i32 (i32.const 3088))
  (global $listed.size i32 (i32.const 4))
  ;; Each slot's descriptor, FREE where no client is, and how many bytes of its request have
  ;; come, 4 bytes a slot.
  (global $fds i32 (i32.const 3104))  ;; shared: take fills a slot, release frees it, all read it
  (global $fds.size i32 (i32.const 32))
  (global $lengths i32 (i32.const 3136))  ;; shared: receive counts, release starts a slot over
  (global $lengths.size i32 (i32.const 32))
  ;; What $http_request_line writes: the method's length, where the target starts, the target's
  ;; length and the version's last digit.
  (global $line i32 (i32.const 3168))  ;; shared: examine has it written, route and log read it
  (global $line.size i32 (i32.const 16))
  ;; What $http_header writes: where a field's value starts and its length.
  (global $field i32 (i32.const 3184))  ;; shared: each function that looks for a field reads its place here
  (global $field.size i32 (i32.const 8))
  ;; The number $decimal_parse writes.
  (global $number i32 (i32.const 3200))
  (global $number.size i32 (i32.const 8))
  ;; What examine found: the head's length, the body's length, whether the request has a
  ;; Content-Length field, and whether its request line could be read, 4 bytes each.
  (global $sizes i32 (i32.const 3216))  ;; shared: examine writes them, the routes and log read them
  (global $sizes.size i32 (i32.const 16))
  ;; The answer's status and its body's length, for the log; whether the answer goes out with
  ;; no body, as a HEAD asks; and where the answer's head ends in $out. 4 bytes each.
  (global $logged i32 (i32.const 3232))  ;; shared: head_start, head_end and route write them, send and log read them
  (global $logged.size i32 (i32.const 16))
  ;; A part of a file: range_read's first and last byte, 8 bytes each, then range_of's first
  ;; byte and count, 4 bytes each.
  (global $span i32 (i32.const 3248))  ;; shared: range_read and range_of write it, send_file reads it
  (global $span.size i32 (i32.const 16))
  ;; What fd_filestat_get writes: the kind at 16 and the size at 32.
  (global $stat i32 (i32.const 3264))
  (global $stat.size i32 (i32.const 64))
  ;; The key a request names.
  (global $key i32 (i32.const 3328))  ;; shared: key_take writes it, key_order and fresh_member read it
  (global $key.size i32 (i32.const 64))
  ;; Where each name of a list starts in $entries, 4 bytes each.
  (global $order i32 (i32.const 3392))  ;; shared: list_read fills it, list_sort orders it, list_text reads it
  (global $order.size i32 (i32.const 1024))
  ;; The path of a request, decoded, which $http_path writes.
  (global $path i32 (i32.const 4608))  ;; shared: route has it written, the routes read it
  (global $path.size i32 (i32.const 1024))
  ;; The name of the file or folder to open: public and what the path has after /files.
  (global $name i32 (i32.const 5632))  ;; shared: file_name writes it, serve_file and serve_list open it
  (global $name.size i32 (i32.const 1040))
  ;; A value of the store in its short form, and a newline.
  (global $value i32 (i32.const 6720))  ;; shared: key_put has it written, key_get fills it, fresh_member reads it
  (global $value.size i32 (i32.const 1025))
  ;; The head of an answer, a short body after it, and then the line of the log.
  (global $out i32 (i32.const 8192))  ;; shared: the put functions write it, the routes send it
  (global $out.size i32 (i32.const 8192))
  ;; What one sock_recv brought.
  (global $chunk i32 (i32.const 16384))  ;; shared: receive reads into it, append copies it out
  (global $chunk.size i32 (i32.const 8192))
  ;; The request that is answered now, copied from its slot.
  (global $request i32 (i32.const 24576))  ;; shared: load fills it, examine and the routes read it
  (global $request.size i32 (i32.const 8192))
  ;; Each slot's request so far, SLOT bytes a slot.
  (global $slots i32 (i32.const 32768))  ;; shared: append writes a slot, load reads it
  (global $slots.size i32 (i32.const 65536))
  ;; What fd_readdir wrote: the entries of a folder.
  (global $entries i32 (i32.const 98304))  ;; shared: list_read fills it, entry_before and list_text read it
  (global $entries.size i32 (i32.const 73728))
  ;; The list of a folder, as it is sent: ENTRIES lines of a name, a slash and a newline.
  (global $listing i32 (i32.const 172032))  ;; shared: list_text writes it, serve_list sends it
  (global $listing.size i32 (i32.const 69632))
  ;; The store as its file holds it: STORE_MAX bytes and one more.
  (global $store i32 (i32.const 241664))  ;; shared: store_load fills it, the store's functions read it
  (global $store.size i32 (i32.const 69632))
  ;; The store as a change makes it: the old store and one member more.
  (global $fresh i32 (i32.const 311296))  ;; shared: the fresh functions write it, store_save writes it out
  (global $fresh.size i32 (i32.const 135168))
  ;; A file: FILE_MAX bytes and one more, and room for the last piece $crc32_update may read.
  (global $content i32 (i32.const 446464))  ;; shared: file_read fills it, checksum and send_file read it
  (global $content.size i32 (i32.const 1052672))

  ;; Waits, for as long as it runs, on every client's connection and, while a slot is free, on
  ;; the listener; then takes each new connection, and reads from each connection that has
  ;; something to read or whose client has closed its side.
  (func $main (export "_start")
    (local $count i32)
    (local $err i32)
    (local $happened i32)
    (local $e i32)
    (local $slot i32)
    (local $left i32)
    (loop $serve  ;; bound: forever
      (local.set $count (call $subscribe))
      (if (i32.gt_u (local.get $count) (global.get $CLIENTS))
        (then (call $assert_fail (i32.const 1)) unreachable))
      (block $served
        (local.set $err
          (call $poll_oneoff (global.get $subscriptions) (global.get $events) (local.get $count)
            (global.get $event_count)))
        ;; A wait that fails is made again.
        (br_if $served (local.get $err))
        (local.set $happened (i32.load (global.get $event_count)))
        ;; poll_oneoff writes at most one event a subscription.
        (if (i32.gt_u (local.get $happened) (local.get $count))
          (then (call $assert_fail (i32.const 2)) unreachable))
        (local.set $e (i32.const 0))
        (local.set $left (global.get $CLIENTS))
        (loop $event  ;; bound: 8
          (br_if $served (i32.ge_u (local.get $e) (local.get $happened)))
          (if (i32.ge_u (local.get $e) (global.get $CLIENTS))
            (then (call $assert_fail (i32.const 3)) unreachable))
          ;; An event starts with its subscription's own number: a slot, or LISTENING.
          (local.set $slot
            (i32.wrap_i64
              (i64.load
                (i32.add (global.get $events) (i32.mul (local.get $e) (global.get $EVENT))))))
          (if (i32.eq (local.get $slot) (global.get $LISTENING))
            (then (call $take))
            (else (call $receive (local.get $slot))))
          (local.set $e (i32.add (local.get $e) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $event (local.get $left))))
      (br $serve)))

  ;; Writes a wait to read on each client's connection, then on the listener while a slot is
  ;; free, each with its own number, the slot's or LISTENING, and gives how many it wrote. With
  ;; every slot taken, further clients wait their turn in the listener's queue.
  (func $subscribe (result i32)
    (local $slot i32)
    (local $count i32)
    (local $fd i32)
    (local $at i32)
    (local $left i32)
    (local.set $left (global.get $CLIENTS))
    (loop $slots  ;; bound: 8
      (if (i32.ge_u (local.get $slot) (global.get $CLIENTS))
        (then (call $assert_fail (i32.const 4)) unreachable))
      (local.set $fd
        (i32.load (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD)))))
      (if (i32.ne (local.get $fd) (global.get $FREE))
        (then
          (if (i32.ge_u (local.get $count) (global.get $CLIENTS))
            (then (call $assert_fail (i32.const 5)) unreachable))
          (local.set $at
            (i32.add (global.get $subscriptions)
              (i32.mul (local.get $count) (global.get $SUBSCRIPTION))))
          (i64.store (local.get $at) (i64.extend_i32_u (local.get $slot)))
          (i32.store8 offset=8 (local.get $at) (global.get $FD_READ))
          (i32.store offset=16 (local.get $at) (local.get $fd))
          (local.set $count (i32.add (local.get $count) (i32.const 1)))))
      (local.set $slot (i32.add (local.get $slot) (i32.const 1)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $slots (local.get $left)))
    (if (i32.lt_u (local.get $count) (global.get $CLIENTS))
      (then
        (local.set $at
          (i32.add (global.get $subscriptions)
            (i32.mul (local.get $count) (global.get $SUBSCRIPTION))))
        (i64.store (local.get $at) (i64.extend_i32_u (global.get $LISTENING)))
        (i32.store8 offset=8 (local.get $at) (global.get $FD_READ))
        (i32.store offset=16 (local.get $at) (global.get $LISTENER))
        (local.set $count (i32.add (local.get $count) (i32.const 1)))))
    (local.get $count))

  ;; Takes the next connection into the first free slot.
  (func $take
    (local $err i32)
    (local $fd i32)
    (local $slot i32)
    (local $left i32)
    (block $taken
      (local.set $err
        (call $sock_accept (global.get $LISTENER) (global.get $NO_FLAGS) (global.get $accepted)))
      ;; A client that left before its turn leaves nothing to take.
      (br_if $taken (local.get $err))
      (local.set $fd (i32.load (global.get $accepted)))
      (if (i32.le_u (local.get $fd) (global.get $LISTENER))
        (then (call $assert_fail (i32.const 6)) unreachable))
      (block $found
        (local.set $left (global.get $CLIENTS))
        (loop $free  ;; bound: 8
          (if (i32.ge_u (local.get $slot) (global.get $CLIENTS))
            (then (call $assert_fail (i32.const 7)) unreachable))
          (br_if $found
            (i32.eq
              (i32.load (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD))))
              (global.get $FREE)))
          (local.set $slot (i32.add (local.get $slot) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $free (local.get $left)))
        ;; main waits on the listener only while a slot is free.
        (call $assert_fail (i32.const 8))
        unreachable)
      (i32.store (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD)))
        (local.get $fd))))

  ;; Receives what has come on slot $slot's connection, at most what its slot has room for,
  ;; adds it to the slot's request, and lets progress see whether the request is whole. A
  ;; connection that fails, or whose client has closed its side, has ended.
  (func $receive (param $slot i32)
    (local $err i32)
    (local $fd i32)
    (local $len i32)
    (local $room i32)
    (local $got i32)
    (local $ended i32)
    (if (i32.ge_u (local.get $slot) (global.get $CLIENTS))
      (then (call $assert_fail (i32.const 9)) unreachable))
    (local.set $fd
      (i32.load (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD)))))
    (local.set $len
      (i32.load (i32.add (global.get $lengths) (i32.mul (local.get $slot) (global.get $WORD)))))
    ;; A request is answered as soon as its slot is full, so a slot has room.
    (if (i32.ge_u (local.get $len) (global.get $SLOT))
      (then (call $assert_fail (i32.const 10)) unreachable))
    (local.set $room (i32.sub (global.get $SLOT) (local.get $len)))
    (local.set $ended (i32.const 1))
    (block $done
      ;; One buffer: the chunk, as long as the room the slot has left.
      (i32.store (global.get $iovec) (global.get $chunk))
      (i32.store offset=4 (global.get $iovec) (local.get $room))
      (local.set $err
        (call $sock_recv (local.get $fd) (global.get $iovec) (global.get $ONE_BUFFER)
          (global.get $NO_FLAGS) (global.get $received) (global.get $received_flags)))
      (br_if $done (local.get $err))
      ;; The bound of a count read from memory first, which makes it a number the rules follow.
      (local.set $got (i32.load (global.get $received)))
      (if (i32.gt_u (local.get $got) (local.get $room))
        (then (call $assert_fail (i32.const 11)) unreachable))
      (br_if $done (i32.eqz (local.get $got)))
      (call $append (i32.add (i32.mul (local.get $slot) (global.get $SLOT)) (local.get $len))
        (local.get $got))
      (i32.store (i32.add (global.get $lengths) (i32.mul (local.get $slot) (global.get $WORD)))
        (i32.add (local.get $len) (local.get $got)))
      (local.set $ended (i32.const 0)))
    (call $progress (local.get $slot) (local.get $ended)))

  ;; Copies the $count bytes at the chunk's start to byte $at of the slots, one at a time, each
  ;; place checked: receive gives the place where the slot's request so far ends.
  (func $append (param $at i32) (param $count i32)
    (local $k i32)
    (local $to i32)
    (local $left i32)
    (if (i32.gt_u (local.get $count) (global.get $SLOT))
      (then (call $assert_fail (i32.const 12)) unreachable))
    (block $done
      (local.set $left (global.get $SLOT))
      (loop $byte  ;; bound: 8192
        (br_if $done (i32.ge_u (local.get $k) (local.get $count)))
        (local.set $to (i32.add (local.get $at) (local.get $k)))
        (if (i32.ge_u (local.get $to) (global.get $slots.size))
          (then (call $assert_fail (i32.const 13)) unreachable))
        (if (i32.ge_u (local.get $k) (global.get $chunk.size))
          (then (call $assert_fail (i32.const 14)) unreachable))
        (i32.store8 (i32.add (global.get $slots) (local.get $to))
          (i32.load8_u (i32.add (global.get $chunk) (local.get $k))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left)))))

  ;; Copies the first $length bytes of the slot that starts at byte $from of the slots to the
  ;; request's start, one at a time, each place checked.
  (func $load (param $from i32) (param $length i32)
    (local $k i32)
    (local $source i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $SLOT))
      (then (call $assert_fail (i32.const 15)) unreachable))
    (block $done
      (local.set $left (global.get $SLOT))
      (loop $byte  ;; bound: 8192
        (br_if $done (i32.ge_u (local.get $k) (local.get $length)))
        (local.set $source (i32.add (local.get $from) (local.get $k)))
        (if (i32.ge_u (local.get $source) (global.get $slots.size))
          (then (call $assert_fail (i32.const 16)) unreachable))
        (if (i32.ge_u (local.get $k) (global.get $request.size))
          (then (call $assert_fail (i32.const 17)) unreachable))
        (i32.store8 (i32.add (global.get $request) (local.get $k))
          (i32.load8_u (i32.add (global.get $slots) (local.get $source))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left)))))

  ;; Sees whether slot $slot's request is whole, now that more of it has come or its
  ;; connection has ended, $ended. A whole request is answered by its route; one that can't be
  ;; served, or that ended before it was whole, with its status. Then the line of the log goes
  ;; out, the connection is closed, and the slot is free.
  (func $progress (param $slot i32) (param $ended i32)
    (local $fd i32)
    (local $length i32)
    (local $status i32)
    (if (i32.ge_u (local.get $slot) (global.get $CLIENTS))
      (then (call $assert_fail (i32.const 18)) unreachable))
    (local.set $fd
      (i32.load (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD)))))
    (local.set $length
      (i32.load (i32.add (global.get $lengths) (i32.mul (local.get $slot) (global.get $WORD)))))
    (if (i32.gt_u (local.get $length) (global.get $SLOT))
      (then (call $assert_fail (i32.const 19)) unreachable))
    (call $load (i32.mul (local.get $slot) (global.get $SLOT)) (local.get $length))
    (local.set $status (call $examine (local.get $length)))
    (block $waits
      (block $close
        (if (i32.eq (local.get $status) (global.get $WAIT))
          (then
            ;; More of the request will come.
            (br_if $waits (i32.eqz (local.get $ended)))
            ;; A client that sent nothing gets nothing.
            (br_if $close (i32.eqz (local.get $length)))
            (local.set $status (global.get $S_BAD_REQUEST))))
        (if (i32.eq (local.get $status) (global.get $WHOLE))
          (then (call $route (local.get $fd)))
          (else (call $fail (local.get $fd) (local.get $status) (global.get $NO_TEXT))))
        (call $log))
      (call $release (local.get $slot))))

  ;; Closes slot $slot's connection and frees the slot for the next client.
  (func $release (param $slot i32)
    (local $err i32)
    (if (i32.ge_u (local.get $slot) (global.get $CLIENTS))
      (then (call $assert_fail (i32.const 20)) unreachable))
    (block $closed
      (local.set $err
        (call $fd_close
          (i32.load (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD))))))
      ;; A connection that doesn't close is lengua's to drop: the slot is free all the same.
      (br_if $closed (local.get $err)))
    (i32.store (i32.add (global.get $fds) (i32.mul (local.get $slot) (global.get $WORD)))
      (global.get $FREE))
    (i32.store (i32.add (global.get $lengths) (i32.mul (local.get $slot) (global.get $WORD)))
      (i32.const 0)))

  ;; Says where the request of $length bytes at the request's start stands: WHOLE, WAIT while
  ;; more of it must come, or the status that refuses it. Writes what it found to $sizes: the
  ;; head's length, the body's, whether a Content-Length field gave it, and whether the request
  ;; line could be read.
  (func $examine (param $length i32) (result i32)
    (local $limit i32)
    (local $head i32)
    (local $body i32)
    (local $status i32)
    (if (i32.gt_u (local.get $length) (global.get $SLOT))
      (then (call $assert_fail (i32.const 21)) unreachable))
    (i64.store (global.get $sizes) (i64.const 0))
    (i64.store offset=8 (global.get $sizes) (i64.const 0))
    (i32.store offset=8 (global.get $logged) (i32.const 0))
    (local.set $limit (local.get $length))
    (if (i32.gt_u (local.get $limit) (global.get $HTTP_HEAD))
      (then (local.set $limit (global.get $HTTP_HEAD))))
    (local.set $status (global.get $WAIT))
    (block $done
      (local.set $head (call $http_head_end (global.get $request) (local.get $limit)))
      (if (i32.eqz (local.get $head))
        (then
          ;; No blank line yet: in HTTP_HEAD bytes, there will be none.
          (if (i32.ge_u (local.get $length) (global.get $HTTP_HEAD))
            (then (local.set $status (global.get $S_FIELDS))))
          (br $done)))
      (if (i32.gt_u (local.get $head) (global.get $HTTP_HEAD))
        (then (call $assert_fail (i32.const 22)) unreachable))
      (local.set $status
        (call $http_request_line (global.get $request) (local.get $head) (global.get $line)))
      (br_if $done (local.get $status))
      (i32.store offset=12 (global.get $sizes) (i32.const 1))
      (local.set $status (global.get $S_BAD_REQUEST))
      (br_if $done
        (i32.eqz (call $http_headers_valid (global.get $request) (local.get $head))))
      ;; A body in pieces, which this server doesn't read.
      (local.set $status (global.get $S_NOT_IMPLEMENTED))
      (br_if $done
        (call $http_header (global.get $request) (local.get $head)
          (i32.add (global.get $names) (global.get $N_ENCODING)) (global.get $N_ENCODING_LENGTH)
          (global.get $field)))
      (local.set $body (call $content_length (local.get $head)))
      (local.set $status (global.get $S_BAD_REQUEST))
      (br_if $done (i32.eq (local.get $body) (global.get $LENGTH_BAD)))
      (if (i32.eq (local.get $body) (global.get $LENGTH_NONE))
        (then (local.set $body (i32.const 0)))
        (else (i32.store offset=8 (global.get $sizes) (i32.const 1))))
      (local.set $status (global.get $S_TOO_LARGE))
      (br_if $done (i32.gt_u (local.get $body) (global.get $BODY_MAX)))
      (i32.store (global.get $sizes) (local.get $head))
      (i32.store offset=4 (global.get $sizes) (local.get $body))
      (local.set $status (global.get $WAIT))
      (br_if $done (i32.lt_u (local.get $length) (i32.add (local.get $head) (local.get $body))))
      (local.set $status (global.get $WHOLE)))
    (local.get $status))

  ;; Gives the number that the $length bytes at place $start of the request write in decimal
  ;; digits, DIGITS_NONE when $length is 0, and DIGITS_BAD for any other text, or a number of
  ;; more than DECIMAL_DIGITS digits.
  (func $digits (param $start i32) (param $length i32) (result i64)
    (local $err i32)
    (local $value i64)
    (local.set $value (global.get $DIGITS_NONE))
    (block $done
      (br_if $done (i32.eqz (local.get $length)))
      (local.set $value (global.get $DIGITS_BAD))
      (br_if $done (i32.gt_u (local.get $length) (global.get $DECIMAL_DIGITS)))
      (br_if $done (i32.ge_u (local.get $start) (global.get $HTTP_HEAD)))
      ;; $decimal_parse takes a minus sign, and a number here has none.
      (br_if $done
        (i32.eq (i32.load8_u (i32.add (global.get $request) (local.get $start)))
          (global.get $DECIMAL_MINUS)))
      (local.set $err
        (call $decimal_parse (i32.add (global.get $request) (local.get $start))
          (local.get $length) (global.get $number)))
      (br_if $done (local.get $err))
      (local.set $value (i64.load (global.get $number)))
      (if (i64.lt_s (local.get $value) (i64.const 0))
        (then (call $assert_fail (i32.const 23)) unreachable)))
    (local.get $value))

  ;; Gives the length that the Content-Length field of the head of $head bytes states:
  ;; LENGTH_NONE when the head has no such field, LENGTH_BAD when its value is no number, and
  ;; BODY_TOO_LARGE for every number above BODY_MAX.
  (func $content_length (param $head i32) (result i32)
    (local $n i64)
    (local $result i32)
    (if (i32.gt_u (local.get $head) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 24)) unreachable))
    (local.set $result (global.get $LENGTH_NONE))
    (block $done
      (br_if $done
        (i32.eqz
          (call $http_header (global.get $request) (local.get $head)
            (i32.add (global.get $names) (global.get $N_LENGTH)) (global.get $N_LENGTH_LENGTH)
            (global.get $field))))
      (local.set $result (global.get $LENGTH_BAD))
      (local.set $n
        (call $digits (i32.load (global.get $field)) (i32.load offset=4 (global.get $field))))
      (br_if $done (i64.lt_s (local.get $n) (i64.const 0)))
      (local.set $result (global.get $BODY_TOO_LARGE))
      (br_if $done (i64.gt_u (local.get $n) (global.get $BODY_MAX_64)))
      (local.set $result (i32.wrap_i64 (local.get $n)))
      (if (i32.gt_u (local.get $result) (global.get $BODY_MAX))
        (then (call $assert_fail (i32.const 25)) unreachable)))
    (local.get $result))

  ;; Answers the whole request at the request's start on $fd, by its method and its path.
  (func $route (param $fd i32)
    (local $method i32)
    (local $start i32)
    (local $length i32)
    (local $size i32)
    (local.set $length (i32.load (global.get $line)))
    (if (i32.gt_u (local.get $length) (global.get $HTTP_METHOD))
      (then (call $assert_fail (i32.const 26)) unreachable))
    (local.set $method (call $method_of (local.get $length)))
    ;; A HEAD is answered as a GET, and the answer goes out with no body.
    (if (i32.eq (local.get $method) (global.get $HEAD))
      (then
        (i32.store offset=8 (global.get $logged) (i32.const 1))
        (local.set $method (global.get $GET))))
    (local.set $start (i32.load offset=4 (global.get $line)))
    (local.set $length (i32.load offset=8 (global.get $line)))
    (if (i32.gt_u (local.get $start) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 27)) unreachable))
    (if (i32.gt_u (local.get $length) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 28)) unreachable))
    (block $done
      (if (i32.gt_u (local.get $length) (global.get $HTTP_PATH))
        (then
          ;; The log has no room for a target this long.
          (i32.store offset=12 (global.get $sizes) (i32.const 0))
          (call $fail (local.get $fd) (global.get $S_TARGET) (global.get $NO_TEXT))
          (br $done)))
      (local.set $size
        (call $http_path (global.get $request) (local.get $start)
          (i32.add (local.get $start) (local.get $length)) (global.get $path)))
      (if (i32.eq (local.get $size) (global.get $HTTP_NO))
        (then
          (call $fail (local.get $fd) (global.get $S_BAD_REQUEST) (global.get $NO_TEXT))
          (br $done)))
      (if (i32.gt_u (local.get $size) (global.get $HTTP_PATH))
        (then (call $assert_fail (i32.const 29)) unreachable))
      (if
        (i32.eq (call $path_prefix (local.get $size) (global.get $P_HEALTH)) (local.get $size))
        (then
          (call $serve_health (local.get $fd) (local.get $method))
          (br $done)))
      (if (call $path_prefix (local.get $size) (global.get $P_FILES))
        (then
          (call $serve_files (local.get $fd) (local.get $method) (local.get $size))
          (br $done)))
      (if (i32.eq (call $path_prefix (local.get $size) (global.get $P_STORE)) (local.get $size))
        (then
          (call $serve_store (local.get $fd) (local.get $method))
          (br $done)))
      (if (call $path_prefix (local.get $size) (global.get $P_KEY))
        (then
          (call $serve_key (local.get $fd) (local.get $method) (local.get $size))
          (br $done)))
      (call $fail (local.get $fd) (global.get $S_NOT_FOUND) (global.get $NO_TEXT))))

  ;; Says which method the request's $length first bytes are: GET, PUT, DELETE, HEAD or
  ;; OTHER_METHOD. The request line has a space after its method.
  (func $method_of (param $length i32) (result i32)
    (local $word i32)
    (local $method i32)
    (if (i32.gt_u (local.get $length) (global.get $HTTP_METHOD))
      (then (call $assert_fail (i32.const 30)) unreachable))
    (local.set $method (global.get $OTHER_METHOD))
    (local.set $word (i32.load (global.get $request)))
    (if (i32.eq (local.get $word) (global.get $WORD_GET))
      (then (local.set $method (global.get $GET))))
    (if (i32.eq (local.get $word) (global.get $WORD_PUT))
      (then (local.set $method (global.get $PUT))))
    (if
      (i32.and
        (i32.and (i32.eq (local.get $word) (global.get $WORD_DELE))
          (i32.eq (i32.load16_u offset=4 (global.get $request)) (global.get $WORD_TE)))
        (i32.eq (local.get $length) (global.get $DELETE_LENGTH)))
      (then (local.set $method (global.get $DELETE))))
    (if
      (i32.and
        (i32.and (i32.eq (local.get $word) (global.get $WORD_HEAD))
          (i32.eq (i32.load8_u offset=4 (global.get $request)) (global.get $SPACE)))
        (i32.eq (local.get $length) (global.get $HEAD_LENGTH)))
      (then (local.set $method (global.get $HEAD))))
    (local.get $method))

  ;; Gives the length of the literal text $text when the path of $size bytes starts with it,
  ;; and 0 when it doesn't. So the path is the text when it gives $size.
  (func $path_prefix (param $size i32) (param $text i32) (result i32)
    (local $k i32)
    (local $c i32)
    (local $at i32)
    (local $result i32)
    (local $left i32)
    (if (i32.gt_u (local.get $size) (global.get $HTTP_PATH))
      (then (call $assert_fail (i32.const 31)) unreachable))
    (block $differs
      (block $whole
        (local.set $left (global.get $TEXT_MAX))
        (loop $byte  ;; bound: 64
          (local.set $at (i32.add (local.get $text) (local.get $k)))
          (if (i32.ge_u (local.get $at) (global.get $texts.rosize))
            (then (call $assert_fail (i32.const 32)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $texts) (local.get $at))))
          (br_if $whole (i32.eqz (local.get $c)))
          (br_if $differs (i32.ge_u (local.get $k) (local.get $size)))
          (br_if $differs (i32.ge_u (local.get $k) (global.get $path.size)))
          (br_if $differs
            (i32.ne (local.get $c) (i32.load8_u (i32.add (global.get $path) (local.get $k)))))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $byte (local.get $left)))
        ;; No literal text is this long.
        (call $assert_fail (i32.const 33))
        unreachable)
      (local.set $result (local.get $k)))
    (local.get $result))

  ;; Copies the literal text $text to place $at of $out and gives the place after it.
  (func $put (param $at i32) (param $text i32) (result i32)
    (local $k i32)
    (local $c i32)
    (local $from i32)
    (local $left i32)
    (block $done
      (local.set $left (global.get $TEXT_MAX))
      (loop $byte  ;; bound: 64
        (local.set $from (i32.add (local.get $text) (local.get $k)))
        (if (i32.ge_u (local.get $from) (global.get $texts.rosize))
          (then (call $assert_fail (i32.const 34)) unreachable))
        (local.set $c (i32.load8_u (i32.add (global.get $texts) (local.get $from))))
        (br_if $done (i32.eqz (local.get $c)))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 35)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at)) (local.get $c))
        (local.set $at (i32.add (local.get $at) (i32.const 1)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $at))

  ;; Writes $value, which is not negative, in decimal at place $at of $out and gives the place
  ;; after it.
  (func $put_number (param $at i32) (param $value i64) (result i32)
    (local $length i32)
    (if (i32.gt_u (local.get $at) (global.get $OUT_NUMBER_LAST))
      (then (call $assert_fail (i32.const 36)) unreachable))
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then (call $assert_fail (i32.const 37)) unreachable))
    (local.set $length
      (call $decimal_format (local.get $value) (i32.add (global.get $out) (local.get $at))))
    (if (i32.gt_u (local.get $length) (global.get $DECIMAL_TEXT))
      (then (call $assert_fail (i32.const 38)) unreachable))
    (i32.add (local.get $at) (local.get $length)))

  ;; Copies $count bytes from place $from of the request to place $at of $out, one at a time,
  ;; each place checked, and gives the place after them.
  (func $put_request (param $at i32) (param $from i32) (param $count i32) (result i32)
    (local $k i32)
    (local $source i32)
    (local $left i32)
    (if (i32.gt_u (local.get $count) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 39)) unreachable))
    (block $done
      (local.set $left (global.get $HTTP_HEAD))
      (loop $byte  ;; bound: 4096
        (br_if $done (i32.ge_u (local.get $k) (local.get $count)))
        (local.set $source (i32.add (local.get $from) (local.get $k)))
        (if (i32.ge_u (local.get $source) (global.get $request.size))
          (then (call $assert_fail (i32.const 40)) unreachable))
        (if (i32.ge_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 41)) unreachable))
        (i32.store8 (i32.add (global.get $out) (local.get $at))
          (i32.load8_u (i32.add (global.get $request) (local.get $source))))
        (local.set $at (i32.add (local.get $at) (i32.const 1)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $at))

  ;; Gives the literal text of a status: its number and its reason.
  (func $reason (param $status i32) (result i32)
    (local $text i32)
    (local.set $text (global.get $R_500))
    (if (i32.eq (local.get $status) (global.get $S_OK))
      (then (local.set $text (global.get $R_200))))
    (if (i32.eq (local.get $status) (global.get $S_NO_CONTENT))
      (then (local.set $text (global.get $R_204))))
    (if (i32.eq (local.get $status) (global.get $S_PARTIAL))
      (then (local.set $text (global.get $R_206))))
    (if (i32.eq (local.get $status) (global.get $S_NOT_MODIFIED))
      (then (local.set $text (global.get $R_304))))
    (if (i32.eq (local.get $status) (global.get $S_BAD_REQUEST))
      (then (local.set $text (global.get $R_400))))
    (if (i32.eq (local.get $status) (global.get $S_NOT_FOUND))
      (then (local.set $text (global.get $R_404))))
    (if (i32.eq (local.get $status) (global.get $S_METHOD))
      (then (local.set $text (global.get $R_405))))
    (if (i32.eq (local.get $status) (global.get $S_LENGTH))
      (then (local.set $text (global.get $R_411))))
    (if (i32.eq (local.get $status) (global.get $S_TOO_LARGE))
      (then (local.set $text (global.get $R_413))))
    (if (i32.eq (local.get $status) (global.get $S_TARGET))
      (then (local.set $text (global.get $R_414))))
    (if (i32.eq (local.get $status) (global.get $S_RANGE))
      (then (local.set $text (global.get $R_416))))
    (if (i32.eq (local.get $status) (global.get $S_FIELDS))
      (then (local.set $text (global.get $R_431))))
    (if (i32.eq (local.get $status) (global.get $S_NOT_IMPLEMENTED))
      (then (local.set $text (global.get $R_501))))
    (if (i32.eq (local.get $status) (global.get $HTTP_NOT_SUPPORTED))
      (then (local.set $text (global.get $R_505))))
    (if (i32.eq (local.get $status) (global.get $S_FULL))
      (then (local.set $text (global.get $R_507))))
    (local.get $text))

  ;; Starts an answer at the start of $out: the status line, and, when $type is a type's
  ;; text, the fields that state the type and the length, $length, of the body. Notes the
  ;; status and the length for the log. Gives the place after what it wrote.
  (func $head_start (param $status i32) (param $type i32) (param $length i32) (result i32)
    (local $at i32)
    (if (i32.gt_u (local.get $length) (global.get $FILE_MAX))
      (then (call $assert_fail (i32.const 42)) unreachable))
    (i32.store (global.get $logged) (local.get $status))
    (i32.store offset=4 (global.get $logged) (local.get $length))
    (local.set $at (call $put (i32.const 0) (global.get $T_HTTP)))
    (local.set $at (call $put (local.get $at) (call $reason (local.get $status))))
    (if (i32.ne (local.get $type) (global.get $NO_TEXT))
      (then
        (local.set $at (call $put (local.get $at) (global.get $T_TYPE)))
        (local.set $at (call $put (local.get $at) (local.get $type)))
        (local.set $at (call $put (local.get $at) (global.get $T_LENGTH)))
        (local.set $at
          (call $put_number (local.get $at) (i64.extend_i32_u (local.get $length))))))
    (call $put (local.get $at) (global.get $T_LINE_END)))

  ;; Writes the end of an answer's head, the Connection field and the blank line, at place $at
  ;; of $out, notes where the head ends, and gives the place after it.
  (func $head_end (param $at i32) (result i32)
    (local.set $at (call $put (local.get $at) (global.get $T_CLOSE)))
    (if (i32.gt_u (local.get $at) (global.get $out.size))
      (then (call $assert_fail (i32.const 148)) unreachable))
    (i32.store offset=12 (global.get $logged) (local.get $at))
    (local.get $at))

  ;; Sends the first $at bytes of $out on $fd: an answer's head and the short body after it. A
  ;; HEAD gets the head alone. A client that has gone gets nothing, and the
  ;; server goes on.
  (func $send (param $fd i32) (param $at i32)
    (local $head i32)
    (if (i32.load offset=8 (global.get $logged))
      (then
        (local.set $head (i32.load offset=12 (global.get $logged)))
        (if (i32.gt_u (local.get $head) (global.get $out.size))
          (then (call $assert_fail (i32.const 149)) unreachable))
        (block $gone
          (br_if $gone
            (call $write_all (local.get $fd) (global.get $out) (local.get $head)))))
      (else
        (if (i32.gt_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 43)) unreachable))
        (block $lost
          (br_if $lost (call $write_all (local.get $fd) (global.get $out) (local.get $at)))))))

  ;; Answers on $fd with $status and a body that is the status's text and a newline, with the
  ;; field $extra, a literal text, when it is one.
  (func $fail (param $fd i32) (param $status i32) (param $extra i32)
    (local $text i32)
    (local $at i32)
    (local.set $text (call $reason (local.get $status)))
    ;; The body's length: where the text would end at the start of $out, and its newline.
    (local.set $at (i32.add (call $put (i32.const 0) (local.get $text)) (i32.const 1)))
    (if (i32.gt_u (local.get $at) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 44)) unreachable))
    (local.set $at (call $head_start (local.get $status) (global.get $M_PLAIN) (local.get $at)))
    (if (i32.ne (local.get $extra) (global.get $NO_TEXT))
      (then (local.set $at (call $put (local.get $at) (local.get $extra)))))
    (local.set $at (call $head_end (local.get $at)))
    (local.set $at (call $put (local.get $at) (local.get $text)))
    (local.set $at (call $put (local.get $at) (global.get $T_NEWLINE)))
    (call $send (local.get $fd) (local.get $at)))

  ;; Answers on $fd with status 204 and no body.
  (func $no_content (param $fd i32)
    (local $at i32)
    (local.set $at
      (call $head_start (global.get $S_NO_CONTENT) (global.get $NO_TEXT) (i32.const 0)))
    (local.set $at (call $head_end (local.get $at)))
    (if (i32.gt_u (local.get $at) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 45)) unreachable))
    (call $send (local.get $fd) (local.get $at)))

  ;; Prints the line of the log for the request that was just answered: its method and its
  ;; target, or two hyphens when its request line couldn't be read, then the status and the
  ;; length of the answer's body. A line that can't be printed is lost, and the server goes on.
  (func $log
    (local $at i32)
    (local $start i32)
    (local $length i32)
    (if (i32.load offset=12 (global.get $sizes))
      (then
        (local.set $length (i32.load (global.get $line)))
        (if (i32.gt_u (local.get $length) (global.get $HTTP_METHOD))
          (then (call $assert_fail (i32.const 46)) unreachable))
        (local.set $at (call $put_request (i32.const 0) (i32.const 0) (local.get $length)))
        (local.set $at (call $put (local.get $at) (global.get $T_SPACE)))
        (local.set $start (i32.load offset=4 (global.get $line)))
        (local.set $length (i32.load offset=8 (global.get $line)))
        (if (i32.gt_u (local.get $length) (global.get $HTTP_HEAD))
          (then (call $assert_fail (i32.const 47)) unreachable))
        (local.set $at
          (call $put_request (local.get $at) (local.get $start) (local.get $length))))
      (else (local.set $at (call $put (i32.const 0) (global.get $T_NO_LINE)))))
    (local.set $at (call $put (local.get $at) (global.get $T_SPACE)))
    (local.set $at
      (call $put_number (local.get $at) (i64.extend_i32_u (i32.load (global.get $logged)))))
    (local.set $at (call $put (local.get $at) (global.get $T_SPACE)))
    ;; The length of the body that went out: none for a HEAD.
    (local.set $length (i32.load offset=4 (global.get $logged)))
    (if (i32.load offset=8 (global.get $logged))
      (then (local.set $length (i32.const 0))))
    (local.set $at (call $put_number (local.get $at) (i64.extend_i32_u (local.get $length))))
    (local.set $at (call $put (local.get $at) (global.get $T_NEWLINE)))
    (if (i32.gt_u (local.get $at) (global.get $out.size))
      (then (call $assert_fail (i32.const 48)) unreachable))
    (block $lost
      (br_if $lost (call $write_all (global.get $STDOUT) (global.get $out) (local.get $at)))))

  ;; GET /health: says ok.
  (func $serve_health (param $fd i32) (param $method i32)
    (local $at i32)
    (block $done
      (if (i32.ne (local.get $method) (global.get $GET))
        (then
          (call $fail (local.get $fd) (global.get $S_METHOD) (global.get $T_ALLOW_GET))
          (br $done)))
      (local.set $at
        (call $head_start (global.get $S_OK) (global.get $M_PLAIN)
          (call $put (i32.const 0) (global.get $T_OK))))
      (local.set $at (call $head_end (local.get $at)))
      (local.set $at (call $put (local.get $at) (global.get $T_OK)))
      (call $send (local.get $fd) (local.get $at))))

  ;; GET /files/ and a path: a file of the folder public, or, for a path that ends with a
  ;; slash, the list of a folder there. A name that starts with a point is not served.
  (func $serve_files (param $fd i32) (param $method i32) (param $size i32)
    (local $length i32)
    (local $last i32)
    (block $done
      (if (i32.ne (local.get $method) (global.get $GET))
        (then
          (call $fail (local.get $fd) (global.get $S_METHOD) (global.get $T_ALLOW_GET))
          (br $done)))
      (if (call $hidden (local.get $size))
        (then
          (call $fail (local.get $fd) (global.get $S_NOT_FOUND) (global.get $NO_TEXT))
          (br $done)))
      (local.set $length (call $file_name (local.get $size)))
      ;; The path's last byte: route found /files/ at its start, so it has one.
      (local.set $last (i32.sub (local.get $size) (i32.const 1)))
      (if (i32.ge_u (local.get $last) (global.get $path.size))
        (then (call $assert_fail (i32.const 49)) unreachable))
      (if
        (i32.eq (i32.load8_u (i32.add (global.get $path) (local.get $last)))
          (global.get $SLASH))
        (then
          (call $serve_list (local.get $fd) (local.get $length))
          (br $done)))
      (call $serve_file (local.get $fd) (local.get $length) (local.get $size))))

  ;; Gives 1 when a name in the path of $size bytes starts with a point: a byte after a slash.
  (func $hidden (param $size i32) (result i32)
    (local $k i32)
    (local $c i32)
    (local $before i32)
    (local $found i32)
    (local $left i32)
    (if (i32.gt_u (local.get $size) (global.get $HTTP_PATH))
      (then (call $assert_fail (i32.const 50)) unreachable))
    (block $done
      (local.set $left (global.get $HTTP_PATH))
      (loop $byte  ;; bound: 1024
        (br_if $done (i32.ge_u (local.get $k) (local.get $size)))
        (if (i32.ge_u (local.get $k) (global.get $path.size))
          (then (call $assert_fail (i32.const 51)) unreachable))
        (local.set $c (i32.load8_u (i32.add (global.get $path) (local.get $k))))
        (if
          (i32.and (i32.eq (local.get $before) (global.get $SLASH))
            (i32.eq (local.get $c) (global.get $POINT)))
          (then
            (local.set $found (i32.const 1))
            (br $done)))
        (local.set $before (local.get $c))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $found))

  ;; Writes to $name the name to open for the path of $size bytes, which starts with /files/:
  ;; public, and what the path has after /files, without a slash at its end. Gives the name's
  ;; length.
  (func $file_name (param $size i32) (result i32)
    (local $k i32)
    (local $o i32)
    (local $last i32)
    (local $left i32)
    (if (i32.gt_u (local.get $size) (global.get $HTTP_PATH))
      (then (call $assert_fail (i32.const 52)) unreachable))
    (if (i32.lt_u (local.get $size) (global.get $FILES_SLASH_LENGTH))
      (then (call $assert_fail (i32.const 53)) unreachable))
    ;; public takes the place of /files: both have FILES_LENGTH bytes.
    (i64.store (global.get $name) (global.get $WORD_PUBLIC))
    (local.set $k (global.get $FILES_LENGTH))
    (local.set $o (global.get $FILES_LENGTH))
    (block $done
      (local.set $left (global.get $HTTP_PATH))
      (loop $byte  ;; bound: 1024
        (br_if $done (i32.ge_u (local.get $k) (local.get $size)))
        (if (i32.ge_u (local.get $k) (global.get $path.size))
          (then (call $assert_fail (i32.const 54)) unreachable))
        (if (i32.ge_u (local.get $o) (global.get $name.size))
          (then (call $assert_fail (i32.const 55)) unreachable))
        (i32.store8 (i32.add (global.get $name) (local.get $o))
          (i32.load8_u (i32.add (global.get $path) (local.get $k))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $o (i32.add (local.get $o) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    ;; A slash at the end is not a part of the name.
    (local.set $last (i32.sub (local.get $o) (i32.const 1)))
    (if (i32.ge_u (local.get $last) (global.get $name.size))
      (then (call $assert_fail (i32.const 56)) unreachable))
    (if
      (i32.eq (i32.load8_u (i32.add (global.get $name) (local.get $last))) (global.get $SLASH))
      (then (local.set $o (local.get $last))))
    (local.get $o))

  ;; Answers on $fd with the file that the $length bytes of $name name, for the path of $size
  ;; bytes: status 404 when it can't be opened or is no file, 500 when it is too large.
  (func $serve_file (param $fd i32) (param $length i32) (param $size i32)
    (local $err i32)
    (local $bytes i32)
    (local $crc i32)
    (if (i32.gt_u (local.get $length) (global.get $name.size))
      (then (call $assert_fail (i32.const 57)) unreachable))
    (block $done
      (block $broken
        (block $missing
          (local.set $err
            (call $path_open (global.get $ROOT) (global.get $NO_FLAGS) (global.get $name)
              (local.get $length) (global.get $NO_FLAGS) (global.get $RIGHT_READ)
              (global.get $NO_RIGHTS) (global.get $NO_FLAGS) (global.get $opened)))
          (br_if $missing (local.get $err))
          (local.set $bytes (call $file_read (i32.load (global.get $opened))))
          (br_if $missing (i32.eq (local.get $bytes) (global.get $NOT_A_FILE)))
          (br_if $broken (i32.eq (local.get $bytes) (global.get $FILE_BROKEN)))
          (if (i32.gt_u (local.get $bytes) (global.get $FILE_MAX))
            (then (call $assert_fail (i32.const 58)) unreachable))
          (local.set $crc (call $checksum (local.get $bytes)))
          (if (call $same_tag (local.get $crc))
            (then (call $not_modified (local.get $fd) (local.get $crc)))
            (else
              (call $send_file (local.get $fd) (local.get $bytes) (local.get $crc)
                (local.get $size))))
          (br $done))
        (call $fail (local.get $fd) (global.get $S_NOT_FOUND) (global.get $NO_TEXT))
        (br $done))
      (call $fail (local.get $fd) (global.get $S_ERROR) (global.get $NO_TEXT))))

  ;; Reads the open file $from whole into $content and closes it. Gives its length, at most
  ;; FILE_MAX; NOT_A_FILE for a folder or anything else that is no file; and FILE_BROKEN for
  ;; a file of more than FILE_MAX bytes or one that can't be read.
  (func $file_read (param $from i32) (result i32)
    (local $err i32)
    (local $got i32)
    (local $result i32)
    (local.set $result (global.get $NOT_A_FILE))
    (block $done
      (local.set $err (call $fd_filestat_get (local.get $from) (global.get $stat)))
      (br_if $done (local.get $err))
      (br_if $done
        (i32.ne (i32.load8_u offset=16 (global.get $stat)) (global.get $IS_FILE)))
      (local.set $result (global.get $FILE_BROKEN))
      (br_if $done
        (i64.gt_u (i64.load offset=32 (global.get $stat)) (global.get $FILE_MAX_64)))
      (local.set $got
        (call $fill (local.get $from) (global.get $content) (global.get $FILE_ROOM)
          (global.get $READ_TO_END)))
      (br_if $done (i32.lt_s (local.get $got) (i32.const 0)))
      (if (i32.gt_u (local.get $got) (global.get $FILE_ROOM))
        (then (call $assert_fail (i32.const 59)) unreachable))
      ;; A file that grew after its size was read.
      (br_if $done (i32.gt_u (local.get $got) (global.get $FILE_MAX)))
      (local.set $result (local.get $got)))
    (block $closed
      (local.set $err (call $fd_close (local.get $from)))
      (br_if $closed (local.get $err)))
    (local.get $result))

  ;; Gives the CRC-32 of the first $size bytes of $content, a piece of CRC32_TEXT bytes at a
  ;; time.
  (func $checksum (param $size i32) (result i32)
    (local $at i32)
    (local $piece i32)
    (local $crc i32)
    (local $left i32)
    (if (i32.gt_u (local.get $size) (global.get $FILE_MAX))
      (then (call $assert_fail (i32.const 60)) unreachable))
    (block $done
      (local.set $left (global.get $PIECES))
      (loop $next  ;; bound: 257
        (br_if $done (i32.ge_u (local.get $at) (local.get $size)))
        (if (i32.ge_u (local.get $at) (global.get $FILE_MAX))
          (then (call $assert_fail (i32.const 61)) unreachable))
        (local.set $piece (i32.sub (local.get $size) (local.get $at)))
        (if (i32.gt_u (local.get $piece) (global.get $CRC32_TEXT))
          (then (local.set $piece (global.get $CRC32_TEXT))))
        (local.set $crc
          (call $crc32_update (local.get $crc)
            (i32.add (global.get $content) (local.get $at)) (local.get $piece)))
        (local.set $at (i32.add (local.get $at) (local.get $piece)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $next (local.get $left))))
    (local.get $crc))

  ;; Gives 1 when the request's If-None-Match field is a star, or the ETag of a file whose
  ;; CRC-32 is $crc: the number in decimal between quotes.
  (func $same_tag (param $crc i32) (result i32)
    (local $head i32)
    (local $start i32)
    (local $length i32)
    (local $last i32)
    (local $same i32)
    (local.set $head (i32.load (global.get $sizes)))
    (if (i32.gt_u (local.get $head) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 62)) unreachable))
    (block $done
      (br_if $done
        (i32.eqz
          (call $http_header (global.get $request) (local.get $head)
            (i32.add (global.get $names) (global.get $N_MATCH)) (global.get $N_MATCH_LENGTH)
            (global.get $field))))
      (local.set $start (i32.load (global.get $field)))
      (local.set $length (i32.load offset=4 (global.get $field)))
      (br_if $done (i32.ge_u (local.get $start) (global.get $HTTP_HEAD)))
      (br_if $done (i32.eqz (local.get $length)))
      (if
        (i32.and (i32.eq (local.get $length) (i32.const 1))
          (i32.eq (i32.load8_u (i32.add (global.get $request) (local.get $start)))
            (global.get $STAR)))
        (then
          (local.set $same (i32.const 1))
          (br $done)))
      (br_if $done (i32.lt_u (local.get $length) (global.get $TAG_MIN)))
      (br_if $done (i32.gt_u (local.get $length) (global.get $TAG_MAX)))
      (local.set $last (i32.sub (i32.add (local.get $start) (local.get $length)) (i32.const 1)))
      (if (i32.ge_u (local.get $last) (global.get $request.size))
        (then (call $assert_fail (i32.const 63)) unreachable))
      (br_if $done
        (i32.ne (i32.load8_u (i32.add (global.get $request) (local.get $start)))
          (global.get $QUOTE)))
      (br_if $done
        (i32.ne (i32.load8_u (i32.add (global.get $request) (local.get $last)))
          (global.get $QUOTE)))
      (local.set $same
        (i64.eq
          (call $digits (i32.add (local.get $start) (i32.const 1))
            (i32.sub (local.get $length) (global.get $PAIR)))
          (i64.extend_i32_u (local.get $crc)))))
    (local.get $same))

  ;; Writes the ETag field of a file whose CRC-32 is $crc at place $at of $out, and gives the
  ;; place after it.
  (func $put_tag (param $at i32) (param $crc i32) (result i32)
    (local.set $at (call $put (local.get $at) (global.get $T_TAG)))
    (local.set $at (call $put_number (local.get $at) (i64.extend_i32_u (local.get $crc))))
    (call $put (local.get $at) (global.get $T_TAG_END)))

  ;; Answers on $fd with status 304 and the ETag of the file: the client has it.
  (func $not_modified (param $fd i32) (param $crc i32)
    (local $at i32)
    (local.set $at
      (call $head_start (global.get $S_NOT_MODIFIED) (global.get $NO_TEXT) (i32.const 0)))
    (local.set $at (call $put_tag (local.get $at) (local.get $crc)))
    (local.set $at (call $head_end (local.get $at)))
    (if (i32.gt_u (local.get $at) (global.get $out.size))
      (then (call $assert_fail (i32.const 64)) unreachable))
    (call $send (local.get $fd) (local.get $at)))

  ;; Reads the request's Range field when it is bytes= and one range: digits, a hyphen and
  ;; digits, where either run of digits may be empty. Writes the two numbers to $span, 8 bytes
  ;; each, DIGITS_NONE for an empty run, and gives 1. Gives 0 for no such field, and for a
  ;; field that is anything else, which the server doesn't follow.
  (func $range_read (result i32)
    (local $head i32)
    (local $start i32)
    (local $end i32)
    (local $dash i32)
    (local $first i64)
    (local $last i64)
    (local $read i32)
    (local.set $head (i32.load (global.get $sizes)))
    (if (i32.gt_u (local.get $head) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 65)) unreachable))
    (block $done
      (br_if $done
        (i32.eqz
          (call $http_header (global.get $request) (local.get $head)
            (i32.add (global.get $names) (global.get $N_RANGE)) (global.get $N_RANGE_LENGTH)
            (global.get $field))))
      (local.set $start (i32.load (global.get $field)))
      (local.set $end (i32.load offset=4 (global.get $field)))
      (br_if $done (i32.ge_u (local.get $start) (global.get $HTTP_HEAD)))
      (br_if $done (i32.lt_u (local.get $end) (global.get $RANGE_MIN)))
      (br_if $done (i32.gt_u (local.get $end) (global.get $HTTP_HEAD)))
      (br_if $done
        (i32.ne (i32.load (i32.add (global.get $request) (local.get $start)))
          (global.get $WORD_BYTE)))
      (br_if $done
        (i32.ne (i32.load16_u offset=4 (i32.add (global.get $request) (local.get $start)))
          (global.get $WORD_S_EQUALS)))
      ;; From the value's length to its end, and its start to after bytes=.
      (local.set $end (i32.add (local.get $start) (local.get $end)))
      (local.set $start (i32.add (local.get $start) (global.get $RANGE_PREFIX)))
      (br_if $done (i32.gt_u (local.get $end) (local.get $head)))
      (local.set $dash
        (call $http_span_end (global.get $request) (local.get $start) (local.get $end)
          (global.get $ZERO) (global.get $NINE)))
      (br_if $done
        (i32.ne (call $http_at (global.get $request) (local.get $dash) (local.get $end))
          (global.get $HYPHEN)))
      (if (i32.lt_u (local.get $dash) (local.get $start))
        (then (call $assert_fail (i32.const 66)) unreachable))
      (local.set $first
        (call $digits (local.get $start) (i32.sub (local.get $dash) (local.get $start))))
      (local.set $last
        (call $digits (i32.add (local.get $dash) (i32.const 1))
          (i32.sub (i32.sub (local.get $end) (local.get $dash)) (i32.const 1))))
      (br_if $done (i64.eq (local.get $first) (global.get $DIGITS_BAD)))
      (br_if $done (i64.eq (local.get $last) (global.get $DIGITS_BAD)))
      (i64.store (global.get $span) (local.get $first))
      (i64.store offset=8 (global.get $span) (local.get $last))
      (local.set $read (i32.const 1)))
    (local.get $read))

  ;; Says which bytes of a file of $size bytes the request asks for: RANGE_WHOLE for all of
  ;; it, RANGE_NONE when its Range field names no byte of the file, and RANGE_PART for a part,
  ;; whose first byte and count it writes to $span, 4 bytes each.
  (func $range_of (param $size i32) (result i32)
    (local $total i64)
    (local $first i64)
    (local $last i64)
    (local $kind i32)
    (if (i32.gt_u (local.get $size) (global.get $FILE_MAX))
      (then (call $assert_fail (i32.const 67)) unreachable))
    (local.set $total (i64.extend_i32_u (local.get $size)))
    (local.set $kind (global.get $RANGE_WHOLE))
    (block $done
      (br_if $done (i32.eqz (call $range_read)))
      (local.set $first (i64.load (global.get $span)))
      (local.set $last (i64.load offset=8 (global.get $span)))
      (if (i64.eq (local.get $first) (global.get $DIGITS_NONE))
        (then
          ;; The last $last bytes of the file. A hyphen alone is no range.
          (br_if $done (i64.eq (local.get $last) (global.get $DIGITS_NONE)))
          (local.set $kind (global.get $RANGE_NONE))
          (br_if $done (i64.eqz (local.get $last)))
          (br_if $done (i64.eqz (local.get $total)))
          (if (i64.gt_u (local.get $last) (local.get $total))
            (then (local.set $last (local.get $total))))
          (local.set $first (i64.sub (local.get $total) (local.get $last)))
          (local.set $last (i64.sub (local.get $total) (i64.const 1))))
        (else
          ;; From byte $first to byte $last, or to the file's end.
          (local.set $kind (global.get $RANGE_NONE))
          (br_if $done (i64.ge_u (local.get $first) (local.get $total)))
          (local.set $kind (global.get $RANGE_WHOLE))
          (if
            (i32.or (i64.eq (local.get $last) (global.get $DIGITS_NONE))
              (i64.ge_u (local.get $last) (local.get $total)))
            (then (local.set $last (i64.sub (local.get $total) (i64.const 1)))))
          ;; A range that ends before it starts is not followed.
          (br_if $done (i64.lt_u (local.get $last) (local.get $first)))))
      (if (i64.ge_u (local.get $first) (local.get $total))
        (then (call $assert_fail (i32.const 68)) unreachable))
      (i32.store (global.get $span) (i32.wrap_i64 (local.get $first)))
      (i32.store offset=4 (global.get $span)
        (i32.wrap_i64
          (i64.add (i64.sub (local.get $last) (local.get $first)) (i64.const 1))))
      (local.set $kind (global.get $RANGE_PART)))
    (local.get $kind))

  ;; Answers on $fd with the file of $size bytes in $content, whose CRC-32 is $crc, or with
  ;; the part of it that the request asks for. The type is from the path of $path_size bytes.
  (func $send_file (param $fd i32) (param $size i32) (param $crc i32) (param $path_size i32)
    (local $kind i32)
    (local $first i32)
    (local $count i32)
    (local $status i32)
    (local $at i32)
    (if (i32.gt_u (local.get $size) (global.get $FILE_MAX))
      (then (call $assert_fail (i32.const 69)) unreachable))
    (block $done
      (local.set $kind (call $range_of (local.get $size)))
      (if (i32.eq (local.get $kind) (global.get $RANGE_NONE))
        (then
          (call $no_range (local.get $fd) (local.get $size))
          (br $done)))
      (local.set $count (local.get $size))
      (local.set $status (global.get $S_OK))
      (if (i32.eq (local.get $kind) (global.get $RANGE_PART))
        (then
          (local.set $first (i32.load (global.get $span)))
          (local.set $count (i32.load offset=4 (global.get $span)))
          (local.set $status (global.get $S_PARTIAL))
          ;; The part moves to the start of $content, where it is sent from.
          (call $content_shift (local.get $first) (local.get $count))))
      (if (i32.gt_u (local.get $count) (global.get $FILE_MAX))
        (then (call $assert_fail (i32.const 70)) unreachable))
      (local.set $at
        (call $head_start (local.get $status) (call $type_of (local.get $path_size))
          (local.get $count)))
      (local.set $at (call $put_tag (local.get $at) (local.get $crc)))
      (if (i32.eq (local.get $kind) (global.get $RANGE_PART))
        (then
          (local.set $at (call $put (local.get $at) (global.get $T_RANGE)))
          (local.set $at
            (call $put_number (local.get $at) (i64.extend_i32_u (local.get $first))))
          (local.set $at (call $put (local.get $at) (global.get $T_HYPHEN)))
          (local.set $at
            (call $put_number (local.get $at)
              (i64.extend_i32_u
                (i32.sub (i32.add (local.get $first) (local.get $count)) (i32.const 1)))))
          (local.set $at (call $put (local.get $at) (global.get $T_SLASH)))
          (local.set $at
            (call $put_number (local.get $at) (i64.extend_i32_u (local.get $size))))
          (local.set $at (call $put (local.get $at) (global.get $T_LINE_END)))))
      (local.set $at (call $head_end (local.get $at)))
      (if (i32.gt_u (local.get $at) (global.get $out.size))
        (then (call $assert_fail (i32.const 71)) unreachable))
      ;; A client that has gone gets no more.
      (br_if $done (call $write_all (local.get $fd) (global.get $out) (local.get $at)))
      ;; A HEAD gets the head alone.
      (br_if $done (i32.load offset=8 (global.get $logged)))
      (br_if $done
        (call $write_all (local.get $fd) (global.get $content) (local.get $count)))))

  ;; Answers on $fd with status 416 and the size, $size, of the file: the request asks for
  ;; bytes that the file doesn't have.
  (func $no_range (param $fd i32) (param $size i32)
    (local $at i32)
    (if (i32.gt_u (local.get $size) (global.get $FILE_MAX))
      (then (call $assert_fail (i32.const 72)) unreachable))
    ;; The body's length: where the text would end at the start of $out, and its newline.
    (local.set $at (i32.add (call $put (i32.const 0) (global.get $R_416)) (i32.const 1)))
    (if (i32.gt_u (local.get $at) (global.get $TEXT_MAX))
      (then (call $assert_fail (i32.const 73)) unreachable))
    (local.set $at
      (call $head_start (global.get $S_RANGE) (global.get $M_PLAIN) (local.get $at)))
    (local.set $at (call $put (local.get $at) (global.get $T_RANGE)))
    (local.set $at (call $put (local.get $at) (global.get $T_ANY)))
    (local.set $at (call $put_number (local.get $at) (i64.extend_i32_u (local.get $size))))
    (local.set $at (call $put (local.get $at) (global.get $T_LINE_END)))
    (local.set $at (call $head_end (local.get $at)))
    (local.set $at (call $put (local.get $at) (global.get $R_416)))
    (local.set $at (call $put (local.get $at) (global.get $T_NEWLINE)))
    (call $send (local.get $fd) (local.get $at)))

  ;; Moves the $count bytes at place $first of $content to its start, one at a time, each
  ;; place checked.
  (func $content_shift (param $first i32) (param $count i32)
    (local $k i32)
    (local $source i32)
    (local $left i32)
    (if (i32.gt_u (local.get $count) (global.get $FILE_MAX))
      (then (call $assert_fail (i32.const 74)) unreachable))
    (block $done
      (local.set $left (global.get $FILE_MAX))
      (loop $byte  ;; bound: 1048576
        (br_if $done (i32.ge_u (local.get $k) (local.get $count)))
        (local.set $source (i32.add (local.get $first) (local.get $k)))
        (if (i32.ge_u (local.get $source) (global.get $FILE_MAX))
          (then (call $assert_fail (i32.const 75)) unreachable))
        (if (i32.ge_u (local.get $k) (global.get $FILE_MAX))
          (then (call $assert_fail (i32.const 76)) unreachable))
        (i32.store8 (i32.add (global.get $content) (local.get $k))
          (i32.load8_u (i32.add (global.get $content) (local.get $source))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left)))))

  ;; Gives the literal text of the type of the file that the path of $size bytes names, from
  ;; what its name has after its last point, capital letters as small ones: M_BYTES when the
  ;; name has no point or its end names no type the server knows.
  (func $type_of (param $size i32) (result i32)
    (local $k i32)
    (local $c i32)
    (local $end i64)
    (local $text i32)
    (local $left i32)
    (if (i32.gt_u (local.get $size) (global.get $HTTP_PATH))
      (then (call $assert_fail (i32.const 77)) unreachable))
    (local.set $text (global.get $M_BYTES))
    (local.set $k (local.get $size))
    (block $none
      (block $point
        (local.set $left (global.get $TYPE_ROOM))
        (loop $byte  ;; bound: 9
          (br_if $none (i32.eqz (local.get $k)))
          (local.set $k (i32.sub (local.get $k) (i32.const 1)))
          (if (i32.ge_u (local.get $k) (global.get $path.size))
            (then (call $assert_fail (i32.const 78)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $path) (local.get $k))))
          (br_if $point (i32.eq (local.get $c) (global.get $POINT)))
          (br_if $none (i32.eq (local.get $c) (global.get $SLASH)))
          (if (i32.le_u (i32.sub (local.get $c) (global.get $BIG_A)) (global.get $LETTERS))
            (then (local.set $c (i32.or (local.get $c) (global.get $CASE_BIT)))))
          ;; The bytes come last first, so the first ends lowest.
          (local.set $end
            (i64.or (i64.shl (local.get $end) (global.get $BYTE_BITS))
              (i64.extend_i32_u (local.get $c))))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $byte (local.get $left)))
        ;; No point in the last bytes of the name.
        (br $none))
      (if (i64.eq (local.get $end) (global.get $END_TXT))
        (then (local.set $text (global.get $M_PLAIN))))
      (if (i64.eq (local.get $end) (global.get $END_HTML))
        (then (local.set $text (global.get $M_HTML))))
      (if (i64.eq (local.get $end) (global.get $END_HTM))
        (then (local.set $text (global.get $M_HTML))))
      (if (i64.eq (local.get $end) (global.get $END_CSS))
        (then (local.set $text (global.get $M_CSS))))
      (if (i64.eq (local.get $end) (global.get $END_CSV))
        (then (local.set $text (global.get $M_CSV))))
      (if (i64.eq (local.get $end) (global.get $END_JS))
        (then (local.set $text (global.get $M_SCRIPT))))
      (if (i64.eq (local.get $end) (global.get $END_JSON))
        (then (local.set $text (global.get $M_JSON))))
      (if (i64.eq (local.get $end) (global.get $END_WASM))
        (then (local.set $text (global.get $M_WASM))))
      (if (i64.eq (local.get $end) (global.get $END_SVG))
        (then (local.set $text (global.get $M_SVG))))
      (if (i64.eq (local.get $end) (global.get $END_PNG))
        (then (local.set $text (global.get $M_PNG))))
      (if (i64.eq (local.get $end) (global.get $END_JPG))
        (then (local.set $text (global.get $M_JPEG))))
      (if (i64.eq (local.get $end) (global.get $END_JPEG))
        (then (local.set $text (global.get $M_JPEG))))
      (if (i64.eq (local.get $end) (global.get $END_GIF))
        (then (local.set $text (global.get $M_GIF)))))
    (local.get $text))

  ;; Answers on $fd with the list of the folder that the $length bytes of $name name: status
  ;; 404 when it can't be opened, and 500 when it has more than ENTRIES names.
  (func $serve_list (param $fd i32) (param $length i32)
    (local $err i32)
    (local $count i32)
    (local $bytes i32)
    (local $at i32)
    (if (i32.gt_u (local.get $length) (global.get $name.size))
      (then (call $assert_fail (i32.const 79)) unreachable))
    (block $done
      (block $broken
        (block $missing
          (local.set $err
            (call $path_open (global.get $ROOT) (global.get $NO_FLAGS) (global.get $name)
              (local.get $length) (global.get $OPEN_FOLDER) (global.get $RIGHT_READ)
              (global.get $NO_RIGHTS) (global.get $NO_FLAGS) (global.get $opened)))
          (br_if $missing (local.get $err))
          (local.set $count (call $list_read (i32.load (global.get $opened))))
          (br_if $broken (i32.eq (local.get $count) (global.get $LIST_BROKEN)))
          (if (i32.gt_u (local.get $count) (global.get $ENTRIES))
            (then (call $assert_fail (i32.const 80)) unreachable))
          (call $list_sort (local.get $count))
          (local.set $bytes (call $list_text (local.get $count)))
          (if (i32.gt_u (local.get $bytes) (global.get $listing.size))
            (then (call $assert_fail (i32.const 81)) unreachable))
          (local.set $at
            (call $head_start (global.get $S_OK) (global.get $M_PLAIN) (local.get $bytes)))
          (local.set $at (call $head_end (local.get $at)))
          (if (i32.gt_u (local.get $at) (global.get $out.size))
            (then (call $assert_fail (i32.const 82)) unreachable))
          ;; A client that has gone gets no more.
          (br_if $done (call $write_all (local.get $fd) (global.get $out) (local.get $at)))
          ;; A HEAD gets the head alone.
          (br_if $done (i32.load offset=8 (global.get $logged)))
          (br_if $done
            (call $write_all (local.get $fd) (global.get $listing) (local.get $bytes)))
          (br $done))
        (call $fail (local.get $fd) (global.get $S_NOT_FOUND) (global.get $NO_TEXT))
        (br $done))
      (call $fail (local.get $fd) (global.get $S_ERROR) (global.get $NO_TEXT))))

  ;; Reads the entries of the open folder $from into $entries and closes it. Writes where each
  ;; entry starts to $order, and gives how many there are, at most ENTRIES. Gives LIST_BROKEN
  ;; when the folder can't be read, or has more names than ENTRIES or than $entries holds.
  (func $list_read (param $from i32) (result i32)
    (local $err i32)
    (local $used i32)
    (local $at i32)
    (local $count i32)
    (local $length i32)
    (local $result i32)
    (local $left i32)
    (local.set $result (global.get $LIST_BROKEN))
    (block $done
      (local.set $err
        (call $fd_readdir (local.get $from) (global.get $entries) (global.get $entries.size)
          (global.get $FROM_FIRST) (global.get $listed)))
      (br_if $done (local.get $err))
      (local.set $used (i32.load (global.get $listed)))
      (if (i32.gt_u (local.get $used) (global.get $entries.size))
        (then (call $assert_fail (i32.const 83)) unreachable))
      ;; A full buffer may have cut the list short.
      (br_if $done (i32.eq (local.get $used) (global.get $entries.size)))
      (block $whole
        (local.set $left (global.get $ENTRIES_AND_ONE))
        (loop $entry  ;; bound: 257
          (br_if $whole
            (i32.gt_u (i32.add (local.get $at) (global.get $ENTRY_HEAD)) (local.get $used)))
          (br_if $done (i32.gt_u (local.get $at) (global.get $ENTRY_LAST)))
          (local.set $length
            (i32.load offset=16 (i32.add (global.get $entries) (local.get $at))))
          (br_if $done (i32.gt_u (local.get $length) (global.get $NAME_MAX)))
          (br_if $done (i32.ge_u (local.get $count) (global.get $ENTRIES)))
          (i32.store
            (i32.add (global.get $order) (i32.mul (local.get $count) (global.get $WORD)))
            (local.get $at))
          (local.set $count (i32.add (local.get $count) (i32.const 1)))
          (local.set $at
            (i32.add (i32.add (local.get $at) (global.get $ENTRY_HEAD)) (local.get $length)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $entry (local.get $left)))
        ;; ENTRIES entries and one more never fit: the count is checked each round.
        (call $assert_fail (i32.const 84))
        unreachable)
      (local.set $result (local.get $count)))
    (block $closed
      (local.set $err (call $fd_close (local.get $from)))
      (br_if $closed (local.get $err)))
    (local.get $result))

  ;; Gives 1 when the name of the entry at place $a of $entries comes before the name of the
  ;; entry at place $b, in byte order, where a name that another starts with comes first.
  (func $entry_before (param $a i32) (param $b i32) (result i32)
    (local $a_length i32)
    (local $b_length i32)
    (local $k i32)
    (local $a_byte i32)
    (local $b_byte i32)
    (local $before i32)
    (local $left i32)
    (if (i32.gt_u (local.get $a) (global.get $ENTRY_LAST))
      (then (call $assert_fail (i32.const 85)) unreachable))
    (if (i32.gt_u (local.get $b) (global.get $ENTRY_LAST))
      (then (call $assert_fail (i32.const 86)) unreachable))
    (local.set $a_length (i32.load offset=16 (i32.add (global.get $entries) (local.get $a))))
    (local.set $b_length (i32.load offset=16 (i32.add (global.get $entries) (local.get $b))))
    (block $done
      (local.set $left (global.get $NAME_ROOM))
      (loop $byte  ;; bound: 256
        ;; $a's name has ended: it comes first unless $b's has ended too.
        (if (i32.ge_u (local.get $k) (local.get $a_length))
          (then
            (local.set $before (i32.lt_u (local.get $k) (local.get $b_length)))
            (br $done)))
        (br_if $done (i32.ge_u (local.get $k) (local.get $b_length)))
        (if (i32.gt_u (local.get $k) (global.get $NAME_MAX))
          (then (call $assert_fail (i32.const 87)) unreachable))
        (local.set $a_byte
          (i32.load8_u offset=24
            (i32.add (i32.add (global.get $entries) (local.get $a)) (local.get $k))))
        (local.set $b_byte
          (i32.load8_u offset=24
            (i32.add (i32.add (global.get $entries) (local.get $b)) (local.get $k))))
        (if (i32.ne (local.get $a_byte) (local.get $b_byte))
          (then
            (local.set $before (i32.lt_u (local.get $a_byte) (local.get $b_byte)))
            (br $done)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $before))

  ;; Sorts the first $count places of $order by their entries' names, by insertion: each
  ;; place moves down past the places whose names come after its name.
  (func $list_sort (param $count i32)
    (local $i i32)
    (local $j i32)
    (local $below i32)
    (local $moving i32)
    (local $other i32)
    (local $outer i32)
    (local $inner i32)
    (if (i32.gt_u (local.get $count) (global.get $ENTRIES))
      (then (call $assert_fail (i32.const 88)) unreachable))
    (local.set $i (i32.const 1))
    (block $sorted
      (local.set $outer (global.get $ENTRIES))
      (loop $each  ;; bound: 256
        (br_if $sorted (i32.ge_u (local.get $i) (local.get $count)))
        (if (i32.ge_u (local.get $i) (global.get $ENTRIES))
          (then (call $assert_fail (i32.const 89)) unreachable))
        (local.set $moving
          (i32.load (i32.add (global.get $order) (i32.mul (local.get $i) (global.get $WORD)))))
        (local.set $j (local.get $i))
        (block $placed
          (local.set $inner (global.get $ENTRIES))
          (loop $down  ;; bound: 256
            (br_if $placed (i32.eqz (local.get $j)))
            (if (i32.ge_u (local.get $j) (global.get $ENTRIES))
              (then (call $assert_fail (i32.const 90)) unreachable))
            (local.set $below (i32.sub (local.get $j) (i32.const 1)))
            (if (i32.ge_u (local.get $below) (global.get $ENTRIES))
              (then (call $assert_fail (i32.const 91)) unreachable))
            (local.set $other
              (i32.load
                (i32.add (global.get $order) (i32.mul (local.get $below) (global.get $WORD)))))
            (br_if $placed
              (i32.eqz (call $entry_before (local.get $moving) (local.get $other))))
            (i32.store
              (i32.add (global.get $order) (i32.mul (local.get $j) (global.get $WORD)))
              (local.get $other))
            (local.set $j (local.get $below))
            (local.set $inner (i32.sub (local.get $inner) (i32.const 1)))
            (br_if $down (local.get $inner))))
        (if (i32.ge_u (local.get $j) (global.get $ENTRIES))
          (then (call $assert_fail (i32.const 92)) unreachable))
        (i32.store (i32.add (global.get $order) (i32.mul (local.get $j) (global.get $WORD)))
          (local.get $moving))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $outer (i32.sub (local.get $outer) (i32.const 1)))
        (br_if $each (local.get $outer)))))

  ;; Writes the list to $listing: the name of each of the first $count entries of $order on a
  ;; line, with a slash after the name of a folder. Gives the list's length.
  (func $list_text (param $count i32) (result i32)
    (local $i i32)
    (local $at i32)
    (local $length i32)
    (local $k i32)
    (local $o i32)
    (local $outer i32)
    (local $inner i32)
    (if (i32.gt_u (local.get $count) (global.get $ENTRIES))
      (then (call $assert_fail (i32.const 93)) unreachable))
    (block $done
      (local.set $outer (global.get $ENTRIES))
      (loop $entry  ;; bound: 256
        (br_if $done (i32.ge_u (local.get $i) (local.get $count)))
        (if (i32.ge_u (local.get $i) (global.get $ENTRIES))
          (then (call $assert_fail (i32.const 94)) unreachable))
        (local.set $at
          (i32.load (i32.add (global.get $order) (i32.mul (local.get $i) (global.get $WORD)))))
        (if (i32.gt_u (local.get $at) (global.get $ENTRY_LAST))
          (then (call $assert_fail (i32.const 95)) unreachable))
        (local.set $length (i32.load offset=16 (i32.add (global.get $entries) (local.get $at))))
        (local.set $k (i32.const 0))
        (block $named
          (local.set $inner (global.get $NAME_ROOM))
          (loop $byte  ;; bound: 256
            (br_if $named (i32.ge_u (local.get $k) (local.get $length)))
            (if (i32.gt_u (local.get $k) (global.get $NAME_MAX))
              (then (call $assert_fail (i32.const 96)) unreachable))
            (if (i32.ge_u (local.get $o) (global.get $listing.size))
              (then (call $assert_fail (i32.const 97)) unreachable))
            (i32.store8 (i32.add (global.get $listing) (local.get $o))
              (i32.load8_u offset=24
                (i32.add (i32.add (global.get $entries) (local.get $at)) (local.get $k))))
            (local.set $k (i32.add (local.get $k) (i32.const 1)))
            (local.set $o (i32.add (local.get $o) (i32.const 1)))
            (local.set $inner (i32.sub (local.get $inner) (i32.const 1)))
            (br_if $byte (local.get $inner))))
        ;; A slash after a folder's name, then the line's end: two bytes at most.
        (if (i32.gt_u (local.get $o) (global.get $LISTING_LAST))
          (then (call $assert_fail (i32.const 98)) unreachable))
        (if
          (i32.eq (i32.load8_u offset=20 (i32.add (global.get $entries) (local.get $at)))
            (global.get $IS_FOLDER))
          (then
            (i32.store8 (i32.add (global.get $listing) (local.get $o)) (global.get $SLASH))
            (local.set $o (i32.add (local.get $o) (i32.const 1)))))
        (i32.store8 (i32.add (global.get $listing) (local.get $o)) (global.get $NEWLINE))
        (local.set $o (i32.add (local.get $o) (i32.const 1)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $outer (i32.sub (local.get $outer) (i32.const 1)))
        (br_if $entry (local.get $outer))))
    (local.get $o))

  ;; GET /kv: the whole store, which is one JSON object.
  (func $serve_store (param $fd i32) (param $method i32)
    (local $length i32)
    (local $at i32)
    (block $done
      (if (i32.ne (local.get $method) (global.get $GET))
        (then
          (call $fail (local.get $fd) (global.get $S_METHOD) (global.get $T_ALLOW_GET))
          (br $done)))
      (local.set $length (call $store_load))
      (if (i32.eq (local.get $length) (global.get $STORE_BROKEN))
        (then
          (call $fail (local.get $fd) (global.get $S_ERROR) (global.get $NO_TEXT))
          (br $done)))
      (if (i32.gt_u (local.get $length) (global.get $STORE_MAX))
        (then (call $assert_fail (i32.const 99)) unreachable))
      (local.set $at
        (call $head_start (global.get $S_OK) (global.get $M_JSON) (local.get $length)))
      (local.set $at (call $head_end (local.get $at)))
      (if (i32.gt_u (local.get $at) (global.get $out.size))
        (then (call $assert_fail (i32.const 100)) unreachable))
      ;; A client that has gone gets no more.
      (br_if $done (call $write_all (local.get $fd) (global.get $out) (local.get $at)))
      ;; A HEAD gets the head alone.
      (br_if $done (i32.load offset=8 (global.get $logged)))
      (br_if $done (call $write_all (local.get $fd) (global.get $store) (local.get $length)))))

  ;; GET, PUT and DELETE /kv/ and a key, for the path of $size bytes: the value of the key is
  ;; given, put or removed.
  (func $serve_key (param $fd i32) (param $method i32) (param $size i32)
    (local $key_length i32)
    (local $length i32)
    (local $place i32)
    (block $done
      (block $broken
        (if (i32.eq (local.get $method) (global.get $OTHER_METHOD))
          (then
            (call $fail (local.get $fd) (global.get $S_METHOD) (global.get $T_ALLOW_ALL))
            (br $done)))
        (local.set $key_length (call $key_take (local.get $size)))
        (if (i32.eqz (local.get $key_length))
          (then
            (call $fail (local.get $fd) (global.get $S_BAD_REQUEST) (global.get $NO_TEXT))
            (br $done)))
        (if (i32.gt_u (local.get $key_length) (global.get $KEY_MAX))
          (then (call $assert_fail (i32.const 101)) unreachable))
        (if (i32.eq (local.get $method) (global.get $PUT))
          (then
            (call $key_put (local.get $fd) (local.get $key_length))
            (br $done)))
        (local.set $length (call $store_load))
        (br_if $broken (i32.eq (local.get $length) (global.get $STORE_BROKEN)))
        (if (i32.gt_u (local.get $length) (global.get $STORE_MAX))
          (then (call $assert_fail (i32.const 102)) unreachable))
        (local.set $place (call $store_seek (local.get $length) (local.get $key_length)))
        (if
          (i32.ne
            (call $key_order (local.get $place) (local.get $length) (local.get $key_length))
            (global.get $ORDER_SAME))
          (then
            (call $fail (local.get $fd) (global.get $S_NOT_FOUND) (global.get $NO_TEXT))
            (br $done)))
        (if (i32.eq (local.get $method) (global.get $GET))
          (then
            (call $key_get (local.get $fd) (local.get $length) (local.get $place)
              (local.get $key_length))
            (br $done)))
        ;; DELETE: the store without the member goes to the file.
        (local.set $length (call $store_remove (local.get $length) (local.get $place)))
        (br_if $broken (call $store_save (local.get $length)))
        (call $no_content (local.get $fd))
        (br $done))
      (call $fail (local.get $fd) (global.get $S_ERROR) (global.get $NO_TEXT))))

  ;; Copies the key that the path of $size bytes has after /kv/ to $key, and gives its
  ;; length. Gives 0 when it is no key: 1 to KEY_MAX letters, digits, hyphens, points and
  ;; underscores.
  (func $key_take (param $size i32) (result i32)
    (local $k i32)
    (local $from i32)
    (local $c i32)
    (local $length i32)
    (local $result i32)
    (local $left i32)
    (if (i32.gt_u (local.get $size) (global.get $HTTP_PATH))
      (then (call $assert_fail (i32.const 103)) unreachable))
    (if (i32.lt_u (local.get $size) (global.get $KEY_PREFIX))
      (then (call $assert_fail (i32.const 104)) unreachable))
    (local.set $length (i32.sub (local.get $size) (global.get $KEY_PREFIX)))
    (block $bad
      (br_if $bad (i32.eqz (local.get $length)))
      (br_if $bad (i32.gt_u (local.get $length) (global.get $KEY_MAX)))
      (block $whole
        (local.set $left (global.get $KEY_ROOM))
        (loop $byte  ;; bound: 65
          (br_if $whole (i32.ge_u (local.get $k) (local.get $length)))
          (if (i32.ge_u (local.get $k) (global.get $KEY_MAX))
            (then (call $assert_fail (i32.const 105)) unreachable))
          (local.set $from (i32.add (local.get $k) (global.get $KEY_PREFIX)))
          (if (i32.ge_u (local.get $from) (global.get $path.size))
            (then (call $assert_fail (i32.const 106)) unreachable))
          (local.set $c (i32.load8_u (i32.add (global.get $path) (local.get $from))))
          ;; A letter, small or capital; a digit; or one of the three marks.
          (br_if $bad
            (i32.eqz
              (i32.or
                (i32.or
                  (i32.le_u
                    (i32.sub (i32.or (local.get $c) (global.get $CASE_BIT))
                      (global.get $SMALL_A))
                    (global.get $LETTERS))
                  (i32.le_u (i32.sub (local.get $c) (global.get $ZERO)) (global.get $NINE)))
                (i32.or
                  (i32.or (i32.eq (local.get $c) (global.get $HYPHEN))
                    (i32.eq (local.get $c) (global.get $POINT)))
                  (i32.eq (local.get $c) (global.get $UNDERSCORE))))))
          (i32.store8 (i32.add (global.get $key) (local.get $k)) (local.get $c))
          (local.set $k (i32.add (local.get $k) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $byte (local.get $left))))
      (local.set $result (local.get $length)))
    (local.get $result))

  ;; GET /kv/ and a key: answers on $fd with the value of the member at place $place of the
  ;; store of $length bytes, whose key has $key_length bytes, and a newline.
  (func $key_get (param $fd i32) (param $length i32) (param $place i32) (param $key_length i32)
    (local $from i32)
    (local $end i32)
    (local $count i32)
    (local $at i32)
    (if (i32.gt_u (local.get $length) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 107)) unreachable))
    (if (i32.gt_u (local.get $key_length) (global.get $KEY_MAX))
      (then (call $assert_fail (i32.const 108)) unreachable))
    (block $done
      (block $broken
        ;; The value: after the quote, the key, the quote, the colon and the space, up to the
        ;; line's end or the comma before it.
        (local.set $from
          (i32.add (i32.add (local.get $place) (local.get $key_length))
            (global.get $KEY_PREFIX)))
        (local.set $end (call $store_line_end (local.get $place) (local.get $length)))
        (if
          (i32.eq
            (call $store_byte (i32.sub (local.get $end) (i32.const 1)) (local.get $length))
            (global.get $COMMA))
          (then (local.set $end (i32.sub (local.get $end) (i32.const 1)))))
        (br_if $broken (i32.ge_u (local.get $from) (local.get $end)))
        (local.set $count (i32.sub (local.get $end) (local.get $from)))
        (br_if $broken (i32.gt_u (local.get $count) (global.get $JSON_TEXT)))
        (call $value_copy (local.get $from) (local.get $count))
        (i32.store8 (i32.add (global.get $value) (local.get $count)) (global.get $NEWLINE))
        (local.set $count (i32.add (local.get $count) (i32.const 1)))
        (local.set $at
          (call $head_start (global.get $S_OK) (global.get $M_JSON) (local.get $count)))
        (local.set $at (call $head_end (local.get $at)))
        (if (i32.gt_u (local.get $at) (global.get $out.size))
          (then (call $assert_fail (i32.const 109)) unreachable))
        ;; A client that has gone gets no more.
        (br_if $done (call $write_all (local.get $fd) (global.get $out) (local.get $at)))
        ;; A HEAD gets the head alone.
        (br_if $done (i32.load offset=8 (global.get $logged)))
        (br_if $done (call $write_all (local.get $fd) (global.get $value) (local.get $count)))
        (br $done))
      ;; A member with no value, or one longer than a value may be.
      (call $fail (local.get $fd) (global.get $S_ERROR) (global.get $NO_TEXT))))

  ;; Copies $count bytes from place $from of the store to the start of $value, one at a time,
  ;; each place checked.
  (func $value_copy (param $from i32) (param $count i32)
    (local $k i32)
    (local $source i32)
    (local $left i32)
    (if (i32.gt_u (local.get $count) (global.get $JSON_TEXT))
      (then (call $assert_fail (i32.const 110)) unreachable))
    (block $done
      (local.set $left (global.get $JSON_TEXT))
      (loop $byte  ;; bound: 1024
        (br_if $done (i32.ge_u (local.get $k) (local.get $count)))
        (local.set $source (i32.add (local.get $from) (local.get $k)))
        (if (i32.ge_u (local.get $source) (global.get $STORE_MAX))
          (then (call $assert_fail (i32.const 111)) unreachable))
        (if (i32.ge_u (local.get $k) (global.get $JSON_TEXT))
          (then (call $assert_fail (i32.const 112)) unreachable))
        (i32.store8 (i32.add (global.get $value) (local.get $k))
          (i32.load8_u (i32.add (global.get $store) (local.get $source))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left)))))

  ;; PUT /kv/ and a key: checks the JSON value in the request's body, and puts its short form
  ;; in the store under the key of $key_length bytes in $key.
  (func $key_put (param $fd i32) (param $key_length i32)
    (local $head i32)
    (local $body i32)
    (local $count i32)
    (local $length i32)
    (local $status i32)
    (local.set $head (i32.load (global.get $sizes)))
    (local.set $body (i32.load offset=4 (global.get $sizes)))
    (if (i32.gt_u (local.get $head) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 113)) unreachable))
    (if (i32.gt_u (local.get $body) (global.get $BODY_MAX))
      (then (call $assert_fail (i32.const 114)) unreachable))
    (block $done
      (block $refused
        ;; A PUT states the length of its body.
        (local.set $status (global.get $S_LENGTH))
        (br_if $refused (i32.eqz (i32.load offset=8 (global.get $sizes))))
        (local.set $status (global.get $S_TOO_LARGE))
        (br_if $refused (i32.gt_u (local.get $body) (global.get $JSON_TEXT)))
        (local.set $status (global.get $S_BAD_REQUEST))
        (local.set $count
          (call $json_minify (i32.add (global.get $request) (local.get $head))
            (local.get $body) (global.get $value)))
        (br_if $refused (i32.eq (local.get $count) (global.get $JSON_BAD)))
        (if (i32.gt_u (local.get $count) (global.get $JSON_TEXT))
          (then (call $assert_fail (i32.const 115)) unreachable))
        (local.set $status (global.get $S_ERROR))
        (local.set $length (call $store_load))
        (br_if $refused (i32.eq (local.get $length) (global.get $STORE_BROKEN)))
        (if (i32.gt_u (local.get $length) (global.get $STORE_MAX))
          (then (call $assert_fail (i32.const 116)) unreachable))
        (local.set $length
          (call $store_put (local.get $length) (local.get $key_length) (local.get $count)))
        ;; The store with the value in it is larger than a store may be.
        (local.set $status (global.get $S_FULL))
        (br_if $refused (i32.gt_u (local.get $length) (global.get $STORE_MAX)))
        (local.set $status (global.get $S_ERROR))
        (br_if $refused (call $store_save (local.get $length)))
        (call $no_content (local.get $fd))
        (br $done))
      (call $fail (local.get $fd) (local.get $status) (global.get $NO_TEXT))))

  ;; Reads the file store.json into $store and gives its length, at most STORE_MAX. With no
  ;; such file, the store is an empty object. Gives STORE_BROKEN when the file can't be read,
  ;; is longer than STORE_MAX, or doesn't start with a brace on a line and end with one.
  (func $store_load (result i32)
    (local $failed i32)
    (local $err i32)
    (local $from i32)
    (local $got i32)
    (local $last i32)
    (local $result i32)
    (local.set $result (global.get $STORE_BROKEN))
    (block $done
      (block $none
        (local.set $err
          (call $path_open (global.get $ROOT) (global.get $NO_FLAGS)
            (i32.add (global.get $texts) (global.get $T_STORE_FILE))
            (global.get $STORE_FILE_LENGTH) (global.get $NO_FLAGS) (global.get $RIGHT_READ)
            (global.get $NO_RIGHTS) (global.get $NO_FLAGS) (global.get $opened)))
        (br_if $none (i32.eq (local.get $err) (global.get $NOENT)))
        (br_if $done (local.get $err))
        (local.set $from (i32.load (global.get $opened)))
        (local.set $got
          (call $fill (local.get $from) (global.get $store) (global.get $STORE_ROOM)
            (global.get $READ_TO_END)))
        ;; The file is closed before a read that failed ends the load.
        (if (i32.lt_s (local.get $got) (i32.const 0))
          (then (local.set $failed (i32.const 1))))
        (block $closed
          (local.set $err (call $fd_close (local.get $from)))
          (br_if $closed (local.get $err)))
        (br_if $done (local.get $failed))
        (if (i32.gt_u (local.get $got) (global.get $STORE_ROOM))
          (then (call $assert_fail (i32.const 117)) unreachable))
        (br_if $done (i32.gt_u (local.get $got) (global.get $STORE_MAX)))
        (br_if $done (i32.lt_u (local.get $got) (global.get $STORE_MIN)))
        (br_if $done
          (i32.ne (i32.load16_u (global.get $store)) (global.get $WORD_STORE_START)))
        ;; The last line is a brace, after the line end of the line before it.
        (local.set $last (i32.sub (local.get $got) (global.get $PAIR)))
        (if (i32.ge_u (local.get $last) (global.get $STORE_MAX))
          (then (call $assert_fail (i32.const 118)) unreachable))
        (br_if $done
          (i32.ne (i32.load16_u (i32.add (global.get $store) (local.get $last)))
            (global.get $WORD_STORE_END)))
        (br_if $done
          (i32.ne (call $store_byte (i32.sub (local.get $last) (i32.const 1)) (local.get $got))
            (global.get $NEWLINE)))
        (local.set $result (local.get $got))
        (br $done))
      (i32.store (global.get $store) (global.get $WORD_EMPTY_STORE))
      (local.set $result (global.get $STORE_MIN)))
    (local.get $result))

  ;; Gives the byte at place $at of the store of $length bytes, or 0 when $at is at or past
  ;; $length.
  (func $store_byte (param $at i32) (param $length i32) (result i32)
    (local $c i32)
    (if (i32.gt_u (local.get $length) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 119)) unreachable))
    (if (i32.lt_u (local.get $at) (local.get $length))
      (then
        (if (i32.ge_u (local.get $at) (global.get $STORE_MAX))
          (then (call $assert_fail (i32.const 120)) unreachable))
        (local.set $c (i32.load8_u (i32.add (global.get $store) (local.get $at))))))
    (local.get $c))

  ;; Gives the place of the first newline at or after place $from of the store of $length
  ;; bytes, or $length when there is none.
  (func $store_line_end (param $from i32) (param $length i32) (result i32)
    (local $i i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 121)) unreachable))
    (local.set $i (local.get $from))
    (block $done
      (local.set $left (global.get $STORE_MAX))
      (loop $byte  ;; bound: 65536
        (if (i32.ge_u (local.get $i) (local.get $length))
          (then
            (local.set $i (local.get $length))
            (br $done)))
        (if (i32.ge_u (local.get $i) (global.get $STORE_MAX))
          (then (call $assert_fail (i32.const 122)) unreachable))
        (br_if $done
          (i32.eq (i32.load8_u (i32.add (global.get $store) (local.get $i)))
            (global.get $NEWLINE)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $i))

  ;; Says how the key of the member whose line starts at place $place of the store of $length
  ;; bytes stands to the key of $key_length bytes in $key, in byte order: ORDER_LESS,
  ;; ORDER_SAME or ORDER_GREATER. A member's key ends at its second quote, which is below
  ;; every byte of a key, so the shorter of two keys that start the same is the lesser.
  (func $key_order (param $place i32) (param $length i32) (param $key_length i32) (result i32)
    (local $k i32)
    (local $member i32)
    (local $wanted i32)
    (local $order i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 123)) unreachable))
    (if (i32.gt_u (local.get $key_length) (global.get $KEY_MAX))
      (then (call $assert_fail (i32.const 124)) unreachable))
    (local.set $order (global.get $ORDER_GREATER))
    (block $done
      (local.set $left (global.get $KEY_ROOM))
      (loop $byte  ;; bound: 65
        ;; The member's byte, after its first quote; past the store, a key has ended.
        (local.set $member
          (call $store_byte (i32.add (i32.add (local.get $place) (i32.const 1)) (local.get $k))
            (local.get $length)))
        (if (i32.eqz (local.get $member))
          (then (local.set $member (global.get $QUOTE))))
        (local.set $wanted (global.get $QUOTE))
        (if (i32.lt_u (local.get $k) (local.get $key_length))
          (then
            (if (i32.ge_u (local.get $k) (global.get $KEY_MAX))
              (then (call $assert_fail (i32.const 125)) unreachable))
            (local.set $wanted (i32.load8_u (i32.add (global.get $key) (local.get $k))))))
        (if (i32.ne (local.get $member) (local.get $wanted))
          (then
            (if (i32.lt_u (local.get $member) (local.get $wanted))
              (then (local.set $order (global.get $ORDER_LESS))))
            (br $done)))
        (if (i32.eq (local.get $member) (global.get $QUOTE))
          (then
            (local.set $order (global.get $ORDER_SAME))
            (br $done)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $order))

  ;; Gives the place of the line of the first member of the store of $length bytes whose key
  ;; is not below the key of $key_length bytes in $key: the member with that key, or the one
  ;; a member with that key goes before. Gives the place of the last line, the brace, when
  ;; every key is below it.
  (func $store_seek (param $length i32) (param $key_length i32) (result i32)
    (local $place i32)
    (local $end i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 126)) unreachable))
    (if (i32.lt_u (local.get $length) (global.get $STORE_MIN))
      (then (call $assert_fail (i32.const 127)) unreachable))
    (local.set $end (i32.sub (local.get $length) (global.get $PAIR)))
    (local.set $place (global.get $PAIR))
    (block $found
      (local.set $left (global.get $STORE_MAX))
      (loop $member  ;; bound: 65536
        (br_if $found (i32.ge_u (local.get $place) (local.get $end)))
        (br_if $found
          (i32.ne
            (call $key_order (local.get $place) (local.get $length) (local.get $key_length))
            (global.get $ORDER_LESS)))
        (local.set $place
          (i32.add (call $store_line_end (local.get $place) (local.get $length)) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $member (local.get $left))))
    (if (i32.gt_u (local.get $place) (local.get $end))
      (then (local.set $place (local.get $end))))
    (local.get $place))

  ;; Copies the bytes from place $from to place $to of the store to place $o of $fresh, one
  ;; at a time, each place checked, and gives the place after them.
  (func $fresh_copy (param $o i32) (param $from i32) (param $to i32) (result i32)
    (local $i i32)
    (local $left i32)
    (if (i32.gt_u (local.get $to) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 128)) unreachable))
    (if (i32.gt_u (local.get $from) (local.get $to))
      (then (call $assert_fail (i32.const 129)) unreachable))
    (local.set $i (local.get $from))
    (block $done
      (local.set $left (global.get $STORE_MAX))
      (loop $byte  ;; bound: 65536
        (br_if $done (i32.ge_u (local.get $i) (local.get $to)))
        (if (i32.ge_u (local.get $i) (global.get $STORE_MAX))
          (then (call $assert_fail (i32.const 130)) unreachable))
        (if (i32.ge_u (local.get $o) (global.get $fresh.size))
          (then (call $assert_fail (i32.const 131)) unreachable))
        (i32.store8 (i32.add (global.get $fresh) (local.get $o))
          (i32.load8_u (i32.add (global.get $store) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $o (i32.add (local.get $o) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $o))

  ;; Copies the literal text $text to place $o of $fresh and gives the place after it.
  (func $fresh_text (param $o i32) (param $text i32) (result i32)
    (local $k i32)
    (local $c i32)
    (local $from i32)
    (local $left i32)
    (block $done
      (local.set $left (global.get $TEXT_MAX))
      (loop $byte  ;; bound: 64
        (local.set $from (i32.add (local.get $text) (local.get $k)))
        (if (i32.ge_u (local.get $from) (global.get $texts.rosize))
          (then (call $assert_fail (i32.const 132)) unreachable))
        (local.set $c (i32.load8_u (i32.add (global.get $texts) (local.get $from))))
        (br_if $done (i32.eqz (local.get $c)))
        (if (i32.ge_u (local.get $o) (global.get $fresh.size))
          (then (call $assert_fail (i32.const 133)) unreachable))
        (i32.store8 (i32.add (global.get $fresh) (local.get $o)) (local.get $c))
        (local.set $o (i32.add (local.get $o) (i32.const 1)))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $o))

  ;; Writes a member at place $o of $fresh: a quote, the key of $key_length bytes in $key, a
  ;; quote, a colon, a space, and the value of $value_length bytes in $value. Gives the place
  ;; after it.
  (func $fresh_member (param $o i32) (param $key_length i32) (param $value_length i32)
    (result i32)
    (local $k i32)
    (local $left i32)
    (if (i32.gt_u (local.get $key_length) (global.get $KEY_MAX))
      (then (call $assert_fail (i32.const 134)) unreachable))
    (if (i32.gt_u (local.get $value_length) (global.get $JSON_TEXT))
      (then (call $assert_fail (i32.const 135)) unreachable))
    (if (i32.gt_u (local.get $o) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 136)) unreachable))
    (i32.store8 (i32.add (global.get $fresh) (local.get $o)) (global.get $QUOTE))
    (local.set $o (i32.add (local.get $o) (i32.const 1)))
    (block $named
      (local.set $left (global.get $KEY_ROOM))
      (loop $name  ;; bound: 65
        (br_if $named (i32.ge_u (local.get $k) (local.get $key_length)))
        (if (i32.ge_u (local.get $k) (global.get $KEY_MAX))
          (then (call $assert_fail (i32.const 137)) unreachable))
        (if (i32.ge_u (local.get $o) (global.get $fresh.size))
          (then (call $assert_fail (i32.const 138)) unreachable))
        (i32.store8 (i32.add (global.get $fresh) (local.get $o))
          (i32.load8_u (i32.add (global.get $key) (local.get $k))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $o (i32.add (local.get $o) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $name (local.get $left))))
    (local.set $o (call $fresh_text (local.get $o) (global.get $T_NAME_END)))
    (local.set $k (i32.const 0))
    (block $valued
      (local.set $left (global.get $JSON_TEXT))
      (loop $byte  ;; bound: 1024
        (br_if $valued (i32.ge_u (local.get $k) (local.get $value_length)))
        (if (i32.ge_u (local.get $k) (global.get $JSON_TEXT))
          (then (call $assert_fail (i32.const 139)) unreachable))
        (if (i32.ge_u (local.get $o) (global.get $fresh.size))
          (then (call $assert_fail (i32.const 140)) unreachable))
        (i32.store8 (i32.add (global.get $fresh) (local.get $o))
          (i32.load8_u (i32.add (global.get $value) (local.get $k))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (local.set $o (i32.add (local.get $o) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (local.get $o))

  ;; Writes to $fresh the store of $length bytes with the value of $value_length bytes in
  ;; $value under the key of $key_length bytes in $key: in place of the member with that key,
  ;; or as a new member where the keys' order puts it. Gives the new store's length, which
  ;; may be above STORE_MAX.
  (func $store_put (param $length i32) (param $key_length i32) (param $value_length i32)
    (result i32)
    (local $place i32)
    (local $end i32)
    (local $line_end i32)
    (local $o i32)
    (if (i32.gt_u (local.get $length) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 141)) unreachable))
    (if (i32.lt_u (local.get $length) (global.get $STORE_MIN))
      (then (call $assert_fail (i32.const 142)) unreachable))
    (local.set $end (i32.sub (local.get $length) (global.get $PAIR)))
    (local.set $place (call $store_seek (local.get $length) (local.get $key_length)))
    (if (i32.gt_u (local.get $place) (local.get $end))
      (then (call $assert_fail (i32.const 143)) unreachable))
    (block $done
      (if (i32.eq (local.get $place) (local.get $end))
        (then
          ;; After the last member, whose line gets a comma; or the first member of all.
          (local.set $o
            (call $fresh_copy (i32.const 0) (i32.const 0)
              (i32.sub (local.get $end) (i32.const 1))))
          (if (i32.gt_u (local.get $end) (global.get $PAIR))
            (then (local.set $o (call $fresh_text (local.get $o) (global.get $T_COMMA)))))
          (local.set $o (call $fresh_text (local.get $o) (global.get $T_NEWLINE)))
          (local.set $o
            (call $fresh_member (local.get $o) (local.get $key_length)
              (local.get $value_length)))
          (local.set $o (call $fresh_text (local.get $o) (global.get $T_STORE_END)))
          (br $done)))
      (local.set $o (call $fresh_copy (i32.const 0) (i32.const 0) (local.get $place)))
      (local.set $o
        (call $fresh_member (local.get $o) (local.get $key_length) (local.get $value_length)))
      (if
        (i32.eq
          (call $key_order (local.get $place) (local.get $length) (local.get $key_length))
          (global.get $ORDER_SAME))
        (then
          ;; In place of the member there: what follows its value stays.
          (local.set $line_end (call $store_line_end (local.get $place) (local.get $length)))
          (if
            (i32.eq
              (call $store_byte (i32.sub (local.get $line_end) (i32.const 1))
                (local.get $length))
              (global.get $COMMA))
            (then (local.set $line_end (i32.sub (local.get $line_end) (i32.const 1)))))
          (local.set $o
            (call $fresh_copy (local.get $o) (local.get $line_end) (local.get $length))))
        (else
          ;; Before the member there.
          (local.set $o (call $fresh_text (local.get $o) (global.get $T_COMMA_NEWLINE)))
          (local.set $o
            (call $fresh_copy (local.get $o) (local.get $place) (local.get $length))))))
    (local.get $o))

  ;; Writes to $fresh the store of $length bytes without the member whose line starts at
  ;; place $place, and gives the new store's length.
  (func $store_remove (param $length i32) (param $place i32) (result i32)
    (local $end i32)
    (local $line_end i32)
    (local $o i32)
    (if (i32.gt_u (local.get $length) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 144)) unreachable))
    (if (i32.lt_u (local.get $place) (global.get $PAIR))
      (then (call $assert_fail (i32.const 145)) unreachable))
    (local.set $end (i32.sub (local.get $length) (global.get $PAIR)))
    (local.set $line_end (call $store_line_end (local.get $place) (local.get $length)))
    (if (i32.ge_u (local.get $line_end) (local.get $length))
      (then (call $assert_fail (i32.const 146)) unreachable))
    (if
      (i32.eq
        (call $store_byte (i32.sub (local.get $line_end) (i32.const 1)) (local.get $length))
        (global.get $COMMA))
      (then
        ;; A member with members after it: its line goes.
        (local.set $o (call $fresh_copy (i32.const 0) (i32.const 0) (local.get $place)))
        (local.set $o
          (call $fresh_copy (local.get $o) (i32.add (local.get $line_end) (i32.const 1))
            (local.get $length))))
      (else
        ;; The last member: the one before it, when there is one, loses its comma.
        (if (i32.eq (local.get $place) (global.get $PAIR))
          (then (local.set $o (call $fresh_copy (i32.const 0) (i32.const 0) (local.get $place))))
          (else
            (local.set $o
              (call $fresh_copy (i32.const 0) (i32.const 0)
                (i32.sub (local.get $place) (global.get $PAIR))))
            (local.set $o (call $fresh_text (local.get $o) (global.get $T_NEWLINE)))))
        (local.set $o (call $fresh_copy (local.get $o) (local.get $end) (local.get $length)))))
    (local.get $o))

  ;; Writes the first $length bytes of $fresh to the file store.json, which it makes or
  ;; empties first. Gives 0, or an error number when the file can't be written.
  (func $store_save (param $length i32) (result i32)  ;; returns: result
    (local $err i32)
    (local $to i32)
    (local $status i32)
    (if (i32.gt_u (local.get $length) (global.get $STORE_MAX))
      (then (call $assert_fail (i32.const 147)) unreachable))
    (local.set $status (global.get $IO))
    (block $done
      (local.set $err
        (call $path_open (global.get $ROOT) (global.get $NO_FLAGS)
          (i32.add (global.get $texts) (global.get $T_STORE_FILE))
          (global.get $STORE_FILE_LENGTH) (global.get $OPEN_NEW) (global.get $RIGHT_WRITE)
          (global.get $NO_RIGHTS) (global.get $NO_FLAGS) (global.get $opened)))
      (br_if $done (local.get $err))
      (local.set $to (i32.load (global.get $opened)))
      (block $failed
        (br_if $failed
          (call $write_all (local.get $to) (global.get $fresh) (local.get $length)))
        (local.set $status (i32.const 0)))
      (block $closed
        (local.set $err (call $fd_close (local.get $to)))
        (br_if $closed (local.get $err))))
    (local.get $status))

  ;; Gives the byte at place $i of the text at $text, or HTTP_NONE when $i is at or past
  ;; $length. $length is at most HTTP_HEAD.
  ;; from: ../http
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
  ;; from: ../http
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
  ;; from: ../http
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
  ;; from: ../http
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
  ;; from: ../http
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
  ;; from: ../http
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
  ;; from: ../http
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
  ;; from: ../http
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
  ;; from: ../http
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
  ;; from: ../http
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
  ;; from: ../http
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
  ;; from: ../http
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

  ;; Checks that the first $length bytes at $text are one JSON value, with only spaces, tabs
  ;; and line ends around its tokens, and writes the value at $into with none of those. Gives
  ;; how many bytes it wrote, at least 1 and never more than $length, or JSON_BAD when the text
  ;; isn't one JSON value or has more than 32 arrays and objects open at once. $length is at
  ;; most JSON_TEXT. Its caller lends $text and $into with room for JSON_TEXT bytes each.
  ;; from: ../json
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
  ;; from: ../json
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
  ;; from: ../json
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
  ;; from: ../json
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
  ;; from: ../json
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
  ;; from: ../json
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
  ;; from: ../json
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
  ;; from: ../json
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
  ;; from: ../json
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

  ;; Reads the whole number that the first $length bytes at $at write in decimal: an optional
  ;; minus sign, then 1 to DECIMAL_DIGITS digits, and nothing else. Writes it at $into, 8 bytes,
  ;; and gives DECIMAL_OK; or gives DECIMAL_INVALID, and writes nothing, when the text isn't a
  ;; number from -9223372036854775807 to 9223372036854775807. Its caller lends $at with room for
  ;; DECIMAL_TEXT bytes, whatever $length is.
  ;; from: ../decimal
  (func $decimal_parse (param $at i32) (param $length i32) (param $into i32) (result i32)  ;; returns: result
    (local $i i32)
    (local $c i32)
    (local $start i32)
    (local $left i32)
    (local $n i64)
    (local $status i32)
    (local.set $status (global.get $DECIMAL_INVALID))
    (block $invalid
      (br_if $invalid (i32.eqz (local.get $length)))
      (br_if $invalid (i32.gt_u (local.get $length) (global.get $DECIMAL_TEXT)))
      (if (i32.eq (i32.load8_u (local.get $at)) (global.get $DECIMAL_MINUS))
        (then (local.set $i (i32.add (local.get $i) (i32.const 1)))))
      ;; Where the digits start: 1 after a minus sign, else 0.
      (local.set $start (local.get $i))
      ;; A sign alone is no number, and a number has at most DECIMAL_DIGITS digits.
      (br_if $invalid (i32.ge_u (local.get $i) (local.get $length)))
      (br_if $invalid
        (i32.gt_u (i32.sub (local.get $length) (local.get $i)) (global.get $DECIMAL_DIGITS)))
      (block $end
        (local.set $left (global.get $DECIMAL_DIGITS))
        (loop $digit  ;; bound: 19
          (br_if $end (i32.ge_u (local.get $i) (local.get $length)))
          (if (i32.ge_u (local.get $i) (global.get $DECIMAL_TEXT))
            (then (call $assert_fail (i32.const 801)) unreachable))
          ;; Below '0' wraps to a large number, so one unsigned test catches both sides.
          (local.set $c
            (i32.sub (i32.load8_u (i32.add (local.get $at) (local.get $i)))
              (global.get $DECIMAL_ZERO)))
          (br_if $invalid (i32.gt_u (local.get $c) (global.get $DECIMAL_NINE)))
          (br_if $invalid (i64.gt_u (local.get $n) (global.get $DECIMAL_MAX_TENTH)))
          (local.set $n (i64.mul (local.get $n) (global.get $DECIMAL_TEN)))
          (br_if $invalid
            (i64.gt_u (local.get $n)
              (i64.sub (global.get $DECIMAL_MAX) (i64.extend_i32_u (local.get $c)))))
          (local.set $n (i64.add (local.get $n) (i64.extend_i32_u (local.get $c))))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (local.set $left (i32.sub (local.get $left) (i32.const 1)))
          (br_if $digit (local.get $left))))
      (if (local.get $start)
        (then (local.set $n (i64.sub (i64.const 0) (local.get $n)))))
      (i64.store (local.get $into) (local.get $n))
      (local.set $status (global.get $DECIMAL_OK)))
    (local.get $status))

  ;; Writes $value in decimal at $into: a minus sign when it is negative, then its digits, with
  ;; no zero at the start but for 0 itself; at most DECIMAL_TEXT bytes. Gives how many bytes it
  ;; wrote. $value is never the most negative number, which has no magnitude in 64 bits.
  ;; from: ../decimal
  (func $decimal_format (param $value i64) (param $into i32) (result i32)
    (local $magnitude i64)
    (local $rest i64)
    (local $length i32)
    (local $at i32)
    (local $left i32)
    (local.set $magnitude (local.get $value))
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then
        (local.set $magnitude (i64.sub (i64.const 0) (local.get $value)))
        ;; The minus sign's byte.
        (local.set $length (i32.add (local.get $length) (i32.const 1)))))
    (if (i64.lt_s (local.get $magnitude) (i64.const 0))
      (then (call $assert_fail (i32.const 802)) unreachable))
    ;; How many digits: one, and one more for each time the magnitude divides by ten.
    (local.set $rest (local.get $magnitude))
    (block $counted
      (local.set $left (global.get $DECIMAL_DIGITS))
      (loop $count  ;; bound: 19
        (local.set $length (i32.add (local.get $length) (i32.const 1)))
        (local.set $rest (i64.div_u (local.get $rest) (global.get $DECIMAL_TEN)))
        (br_if $counted (i64.eqz (local.get $rest)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $count (local.get $left)))
      ;; A magnitude below 2 to the 63rd has at most 19 digits.
      (call $assert_fail (i32.const 803))
      unreachable)
    ;; The digits, from the last to the first.
    (local.set $at (local.get $length))
    (block $written
      (local.set $left (global.get $DECIMAL_DIGITS))
      (loop $digit  ;; bound: 19
        (local.set $at (i32.sub (local.get $at) (i32.const 1)))
        (if (i32.ge_u (local.get $at) (global.get $DECIMAL_TEXT))
          (then (call $assert_fail (i32.const 804)) unreachable))
        (i32.store8 (i32.add (local.get $into) (local.get $at))
          (i32.add (global.get $DECIMAL_ZERO)
            (i32.wrap_i64 (i64.rem_u (local.get $magnitude) (global.get $DECIMAL_TEN)))))
        (local.set $magnitude (i64.div_u (local.get $magnitude) (global.get $DECIMAL_TEN)))
        (br_if $written (i64.eqz (local.get $magnitude)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $digit (local.get $left)))
      ;; The loop above counted these digits.
      (call $assert_fail (i32.const 805))
      unreachable)
    (if (i64.lt_s (local.get $value) (i64.const 0))
      (then (i32.store8 (local.get $into) (global.get $DECIMAL_MINUS))))
    (local.get $length))


  ;; Gives the CRC-32 of some bytes and then the $length bytes at $at, $crc being the CRC-32 of
  ;; the bytes before them, 0 when there are none: the CRC-32 of zlib, gzip and PNG. Each byte is
  ;; xored into the CRC, and then, eight times, the CRC moves one bit down and the polynomial
  ;; comes in when the bit that left was set, through a mask, with no branch on the data.
  ;; $length is at most CRC32_TEXT. Its caller lends $at with room for CRC32_TEXT bytes, whatever
  ;; $length is.
  ;; from: ../crc32
  (func $crc32_update (param $crc i32) (param $at i32) (param $length i32) (result i32)
    (local $i i32)
    (local $c i32)
    (local $bit i32)
    (local $left i32)
    (if (i32.gt_u (local.get $length) (global.get $CRC32_TEXT))
      (then (call $assert_fail (i32.const 831)) unreachable))
    (local.set $c (i32.xor (local.get $crc) (global.get $CRC32_ALL_ONES)))
    (block $done
      (local.set $left (global.get $CRC32_TEXT))
      (loop $byte  ;; bound: 4096
        (br_if $done (i32.ge_u (local.get $i) (local.get $length)))
        (if (i32.ge_u (local.get $i) (global.get $CRC32_TEXT))
          (then (call $assert_fail (i32.const 832)) unreachable))
        (local.set $c
          (i32.xor (local.get $c) (i32.load8_u (i32.add (local.get $at) (local.get $i)))))
        (local.set $bit (global.get $CRC32_BITS))
        (loop $shift  ;; bound: 8
          ;; All ones when the lowest bit is set, else none: what of the polynomial comes in.
          (local.set $c
            (i32.xor (i32.shr_u (local.get $c) (i32.const 1))
              (i32.and (global.get $CRC32_POLY)
                (i32.sub (i32.const 0) (i32.and (local.get $c) (i32.const 1))))))
          (local.set $bit (i32.sub (local.get $bit) (i32.const 1)))
          (br_if $shift (local.get $bit)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $byte (local.get $left))))
    (i32.xor (local.get $c) (global.get $CRC32_ALL_ONES)))
)
