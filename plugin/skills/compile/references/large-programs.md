# A large program: 60 functions or more

The templates have a few functions. A program of thousands of lines, such as a server with many
routes, is laid out the same way and needs a plan for three things before its first function:
where its state lives, how a request passes between functions, and how the rules' budgets are
kept. This page is drawn from the web server `examples/httpd` of lengua's repository: 3,600
lines, 85 functions, built on four parts.

## Check early and often

`check` answers in seconds, and it reads the rules before any test runs. Call it after each
group of functions, not after the whole program: a rule that a pattern breaks is then found
once, not in every function that copied the pattern. Its reply has `functions` (how full each
function is, [rule 4](power-of-10-webassembly.md)) whether or not the rules pass.

## The order of the file

1. The imports and the memory.
2. The named constants, in groups, each under a comment: descriptors, limits from the idea,
   statuses, what each function gives (`$HTTP_OK`, `$NOT_A_FILE`), characters, words.
3. The literal texts and where each is ([literal texts](texts.md)).
4. The regions, lowest address first, each with a comment that says what it holds and how its
   bytes are laid out.
5. The functions, from `$main` down: the loop, the request, each route, then the helpers.
6. The functions taken from parts, each under its `;; from:` line ([parts](parts.md)).

Give the assertions of each group of functions a range of numbers of its own (1 to 99 for the
loop, 100 to 199 for the files), so that a new assertion needs no search for a free number. A
part's assertions keep the part's numbers, and your own stay within 1 to 799.

## Where state lives

All state is in regions, each sized from a limit that the idea states. Rule 6 gives a region to
one function. A region that several functions use is marked `;; shared:` with who writes it and
who reads it, and in a large program most are: 23 of the server's 31 writable regions.

```wat
  ;; What $http_request_line writes: the method's length, where the target starts, the target's
  ;; length and the version's last digit, 4 bytes each.
  (global $line i32 (i32.const 3168))  ;; shared: examine has it written, route and log read it
  (global $line.size i32 (i32.const 16))
```

Three kinds of region carry a program this size:

- **A buffer for each large thing**, as long as its limit and one byte more, to see input that
  is too long: the request, a file, the output.
- **A small record for each group of facts**, 4 bytes a number: where the request line's parts
  are, what was found of the request (the head's length, the body's length), what the answer is
  (its status, its body's length) for the log. A function writes its record once; the others
  load from it.
- **One place for each thing the host writes**: a descriptor, a count, the 64 bytes of a file's
  state.

A program has no heap: two things alive at once are two regions, or two slots of one region
(`slot * SLOT`, with the slot checked against the count of slots).

## How a request passes between functions

- **The bytes stay where they are.** One function fills `$request`. Every other function reads
  it by place: it takes a place and a length as parameters, never the address of a place, and
  checks each index against the region's limit itself (rule 9).
- **Parameters are plain numbers**: a descriptor, a place, a length, a status. A function that
  is lent an address reaches a fixed number of bytes from it, so an address is passed only for
  a whole region, as to a part's function (`(global.get $request)`).
- **A number that comes from a record is checked again where it is used.** A loaded number is
  one that the rules know nothing of, so the function that loads it bounds it with an assertion
  before it indexes with it. These are most of a large program's assertions, and they are what
  rule 5 asks for.

```wat
    (local.set $start (i32.load offset=4 (global.get $line)))
    (if (i32.gt_u (local.get $start) (global.get $HTTP_HEAD))
      (then (call $assert_fail (i32.const 27)) unreachable))
```

- **A number from two paths is checked on each path**, before the paths meet
  ([rule 9](power-of-10-webassembly.md)).
- **A function that can fail gives a status**, a named constant, and its caller tests it at
  once. Mark it `;; returns: result` when it gives an error number (rule 7): then nothing may
  come between the call and the test, not even the close of a file. Test first and keep the
  outcome in a local, close, then let the local decide.
- **One function chooses the route**, with one `if` a route that calls the route's function and
  leaves by `(br $done)`. Each route's function sends its own answer. A function ends once, at
  its `end`.

## Output

One region holds the answer. Each function that writes to it takes the place to write at and
gives the place after what it wrote ([literal texts](texts.md)). One function sends the bytes
and one writes the log, from the record that says what the answer was.

## The budgets

- **Rule 4, 180 operators a function,** is met first. Plan a function for one thing: read one
  field, write one line, choose one route. When `functions` names a function (135 operators or
  more), split it before you add to it: move a loop, or the code of one block, into a function
  of its own. A machine with states splits into the step for one byte and the loop around it.
- **Rule 5, two assertions a function on average.** A small helper with no assertion costs two
  elsewhere. Give each helper the check of what it relies on (its index in range, its count
  below a limit); `functions` says how many assertions are to spare.
- **Each copy between two regions is a loop of its own**, with a bound and a check of each
  index, since a function reaches one fixed stretch from an address. Write one copy function
  for each pair of regions that needs one.

## Parts

Take what reads text from a part: numbers, a request's head, JSON. A part's own program is
proven for its inputs, and a server's tests are not proven, so what moves into a part gains its
proof there. A part and the programs that copy from it change in one step: after a change to a
part, copy its functions and their constants again, and check the part first.
