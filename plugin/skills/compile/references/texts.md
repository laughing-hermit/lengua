# Literal texts: an answer built from many pieces

A program that answers with fixed words (a status line, the names of fields, the types of
files, error messages) has many literal texts and builds its output from them and from
numbers. This page shows the layout that the rules accept, and how `check` computes each text's
place and length, so that you add no lengths by hand.

## One read-only region, every text in it

Put all the literal texts in one read-only region, in one `data` segment with one string a
line, each ended by a NUL, and name each string:

```wat
  ;; The literal texts, read-only, one after another, each ended by a NUL.
  (global $texts i32 (i32.const 1024))
  (global $texts.rosize i32 (i32.const 40))  ;; len: texts
  (data (i32.const 1024)  ;; text: texts
    "HTTP/1.1 \00"  ;; text: http
    "\0d\0aContent-Length: \00"  ;; text: length
    "200 OK\00"  ;; text: ok
    "ok\0a\00")  ;; text: body

  ;; Where each literal text starts in $texts.
  (global $T_HTTP i32 (i32.const 0))  ;; offset: http
  (global $T_LENGTH i32 (i32.const 10))  ;; offset: length
  (global $T_OK i32 (i32.const 29))  ;; offset: ok
  (global $T_BODY i32 (i32.const 36))  ;; offset: body
  (global $BODY_LENGTH i32 (i32.const 4))  ;; len: body
```

The markers are comments: main.wat stays plain WebAssembly text, and it is the text that is
checked and run.

| Marker | On the line of | What it says |
|---|---|---|
| `;; text: NAME` | one string of a `data` segment | the string's bytes are the text NAME |
| `;; text: NAME` | the start of a `data` segment, with no string on that line | all the segment's bytes are the text NAME |
| `;; at: NAME` | a constant global | its number is the address of NAME's first byte |
| `;; offset: NAME` | a constant global | its number is how many bytes NAME is from the start of its region |
| `;; len: NAME` | a constant global | its number is how many bytes NAME has, its NUL counted when the string holds one |

A segment's strings lie one after another from the segment's address, so only the segment has
an address to write. One marker a line, one string a line where a string is named, and each
name once.

## `check` computes the numbers

1. Write the strings and their names. Write each marked number as 0.
2. Call `check`. It replies `code_problems`, with a problem for each number that is not its
   text's, which says the right number, and with the table of every text in `data`:

   ```text
   rule 8 (named constants) main.wat:14: $T_OK is 0, and the text `ok` is 29 bytes into $texts. Fix: write `(i32.const 29)`: …
   ok: at 1053, offset 29 in $texts, len 7
   ```

   At most 20 problems are listed; the table has every text.
3. Write the numbers from the table, and call `check` again.

After that, change a text as you like: `check` names each number that moved, with its new
value, and the table has them all again. When every text is named by a number and every marked
number is right, `data` is one line that says so. A number with no marker is checked by nothing:
mark every number that depends on a text.

## Copy by offset, not by address

A function that is lent an address reaches a fixed number of bytes from it, whatever length it
is given: the rules judge the call by the function's whole reach. So a copy function that takes
a text's address can't be lent one small text after another, since each text would need the
function's whole reach after it inside its region. Pass the text's offset, and let the function
index the one region itself, with its check:

```wat
  ;; Copies the literal text at offset $text of $texts to place $at of $out, up to its NUL, and
  ;; gives the place after it.
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
```

Each function that writes output takes the place to write at and gives the place after what it
wrote, so an answer is a chain: `(local.set $at (call $put (local.get $at) (global.get
$T_LENGTH)))`, then `$put_number`, then the next text. One function at the end checks the place
against the region's size and sends the bytes with `$write_all`.

A text that a host call takes as it is, such as a file's name for `path_open`, is passed by its
address and its length: `(i32.add (global.get $texts) (global.get $T_FILE))` and a constant
marked `;; len:`. Name such a string without its NUL, and put the NUL in a string of its own
after it, so that `;; len:` is the length the call needs.

A function taken from a part brings no data. When it compares against fixed text, such as the
name of a field, the text lies in a region of the program's own that is as long as the part's
function reads, padded with NULs, one such region for the texts of one length.

The web server `examples/httpd` of lengua's repository builds every answer this way from 54 texts.
