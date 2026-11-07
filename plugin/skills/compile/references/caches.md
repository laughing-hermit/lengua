# Laying out work for the CPU's caches

`check`'s `machine` line for a `main.wat` folder ends with this computer's caches, as its system
describes them:

```text
…; the CPU's caches are L1 data 48 KiB, L2 2 MiB and L3 36 MiB, in 64-byte lines
```

Each level that holds data is named with its size, and the lines' size once when every level
has the same (on Linux, from the CPU's folders under `/sys`; on macOS, from `sysctl`, the
performance cores'; on Windows, from `GetLogicalProcessorInformationEx`). A level the system
doesn't describe isn't named, and lengua never guesses one: then the line says less, or "the
system describes none of the CPU's caches".

`main.wat` runs the same on every computer, so lay it out for every computer, not for this one,
by sizes that the CPUs of lengua's five runners have, as almost every 64-bit CPU of the last
decade does: an L1 data cache of at least 32 KiB, an L2 of at least 256 KiB, and lines of 64 bytes
(128 on Apple's CPUs). Where the machine line names larger caches, the same layout still fits;
don't size a buffer to this computer's L2, which another computer's may halve.

## Blocks within L1 and L2

A loop over a large input touches each byte once, and its speed is the memory's; a loop that comes
back to the same data is fast only while that data stays in a cache.

- **Block what a loop comes back to:** split the work into blocks whose data, every region the
  block reads and writes, fits within 32 KiB, half an L1 at the least, and finish each block
  before the next. A sort's runs, a matrix's tiles and a hash table's chunks are blocks.
- **A second level for larger work:** groups of blocks within 256 KiB fit the L2.
- **Stream what a loop touches once:** read it in large pieces (`$fill` into a 64 KiB buffer, as
  the stream template does), forward, so the CPU fetches the next lines before the loop needs
  them.

## A region's hot data together

- **What a step reads at once, in one place:** keep the fields one pass uses side by side, in a
  region of their own, rather than spread among fields the pass doesn't read. A tally that reads
  only the counts keeps them apart from the keys.
- **Whole lines:** a 64-byte line is the least the cache moves. A record of 16 bytes at a
  16-byte offset never crosses a line; one of 24 bytes at a 24-byte offset sometimes does, and
  then costs two.
- **Align large regions to 64 bytes**, as wasm.md says: a vector loop's loads then never cross a
  line.

## 128 bytes between what changes apart

Data written at different times, such as an input buffer and the counts kept while it's read,
belongs in different lines. Start each writable region at a multiple of 128 bytes and leave its
end 128 bytes before the next: Apple's lines are 128 bytes, and other CPUs fetch pairs of 64-byte
lines together, so 128 bytes keeps apart what changes apart on all of them.

## Strides that avoid 4 KiB aliasing

A CPU tells a load from an earlier store by their addresses' low bits first. When a loop reads one
buffer and writes another whose addresses differ by a multiple of 4096, each load looks as if it
waits for the store before it, and the loop slows.

- **Offset buffers:** place a buffer the loop writes 64 or 128 bytes past a multiple of 4096 from
  the one it reads, such as an output at 4096 × n + 128 for an input at 4096 × m.
- **Pad a power of two:** rows of 4096 bytes walked down a column fall in the same few cache sets
  and push each other out; give each row a line more, 4160 bytes, and the column spreads over
  every set.
- **Walk in order:** a stride of a line or less uses every byte of each line the cache fetched; a
  larger one uses a part of each, so where the work allows, walk memory in order.

## Tables

A table a loop looks things up in fits the L1 when it's small: 256 entries of 4 bytes are 1 KiB.
A table of 64 KiB or more is read from L2 or memory at each lookup; split the lookup into smaller
tables, or compute instead of looking up. A table indexed by a secret is a different matter: the
secret rule refuses that address ([constant-time.md](constant-time.md)).
