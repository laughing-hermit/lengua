---
okf_version: "0.2"
---

# Where it runs

* One WebAssembly file. Runs on any OS and CPU. Sandboxed: it reaches only what the idea grants.

# Back end: program

* [sleepy](idea.md) - The program waits for 30 seconds in a call to the host, and then it writes `awake`. Label: `tested`.

# Back end: crates

* Declared in the ideas: none; the code uses the Rust standard library alone.
