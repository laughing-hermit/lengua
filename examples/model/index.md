---
okf_version: "0.2"
---

# Where it runs

* A native program, built for this computer's system and CPU, for the fastest local inference. It runs outside the sandbox: it can do what the user's account can.

# Back end: program

* [model](idea.md) - The program is a small language model, and it is a native part. Label: `native`.

# Back end: crates

* Declared in the ideas: 1: `candle-core` 0.11.1.
* Brought in by them: 167.
* With `unsafe` in their Rust code: 122: `ahash` 0.8.12, `aho-corasick` 1.1.5, `allocator-api2` 0.2.21, `bit-set` 0.8.0, `bit-vec` 0.8.0, `bitflags` 2.13.2, `block2` 0.6.2, `bumpalo` 3.20.4, `bytemuck` 1.25.2, `bytemuck_derive` 1.12.1, `byteorder` 1.5.0, `candle-core` 0.11.1, `candle-metal-kernels` 0.11.1, `castaway` 0.2.4, `compact_str` 0.9.1, `constant_time_eq` 0.6.1, `crc32fast` 1.5.5, `crossbeam-deque` 0.8.8, `crossbeam-epoch` 0.9.21, `crossbeam-utils` 0.8.23, `daachorse` 3.0.3, `darling_core` 0.20.11, `dary_heap` 0.3.9, `derive_builder` 0.20.2, `dispatch2` 0.3.1, `dyn-stack` 0.13.2, `either` 1.19.0, `errno` 0.3.15, `esaxx-rs` 0.1.10, `float8` 0.7.0, `foldhash` 0.2.0, `gemm` 0.19.0, `gemm-c32` 0.19.0, `gemm-c64` 0.19.0, `gemm-common` 0.19.0, `gemm-f16` 0.19.0, `gemm-f32` 0.19.0, `gemm-f64` 0.19.0, `getrandom` 0.3.4, `getrandom` 0.4.3, `half` 2.7.1, `hashbrown` 0.16.1, `hashbrown` 0.17.1, `hermit-abi` 0.5.3, `indexmap` 2.14.2, `itertools` 0.14.0, `itoa` 1.0.19, `js-sys` 0.3.107, `libc` 0.2.191, `libm` 0.2.16, `linux-raw-sys` 0.12.1, `lock_api` 0.4.14, `log` 0.4.35, `memchr` 2.8.3, `memmap2` 0.9.12, `minimal-lexical` 0.2.1, `monostate` 0.1.18, `nix` 0.31.3, `nom` 7.1.3, `num-complex` 0.4.6, `num-traits` 0.2.19, `num_cpus` 1.17.0, `objc2` 0.6.5, `objc2-core-foundation` 0.3.2, `objc2-encode` 4.1.0, `objc2-foundation` 0.3.2, `objc2-metal` 0.3.2, `once_cell` 1.21.4, `parking_lot` 0.12.5, `parking_lot_core` 0.9.12, `pin-project-lite` 0.2.17, `ppv-lite86` 0.2.21, `proc-macro2` 1.0.107, `pulp` 0.22.3, `r-efi` 5.3.0, `r-efi` 6.0.0, `rand` 0.9.5, `raw-cpuid` 11.6.0, `rayon` 1.12.0, `rayon-core` 1.13.0, `redox_syscall` 0.5.18, `regex` 1.13.1, `regex-automata` 0.4.18, `rustix` 1.1.6, `rustversion` 1.0.23, `ryu` 1.0.23, `safetensors` 0.8.0, `same-file` 1.0.6, `scopeguard` 1.2.0, `serde` 1.0.229, `serde_core` 1.0.229, `serde_json` 1.0.154, `smallvec` 1.16.5, `stable_deref_trait` 1.2.1, `static_assertions` 1.1.0, `syn` 2.0.119, `syn` 3.0.10, `synstructure` 0.14.0, `sysctl` 0.6.0, `tempfile` 3.27.0, `thiserror-impl` 1.0.69, `thiserror-impl` 2.0.21, `tokenizers` 0.23.2, `tracing` 0.1.44, `tracing-attributes` 0.1.31, `tracing-core` 0.1.36, `typed-path` 0.13.0, `unicode-ident` 1.0.27, `unicode-normalization-alignments` 0.1.12, `wasip2` 1.0.4+wasi-0.2.12, `wasm-bindgen` 0.2.130, `wasm-bindgen-macro-support` 0.2.130, `winapi-util` 0.1.11, `windows-sys` 0.61.2, `wit-bindgen` 0.57.1, `yoke` 0.8.4, `yoke-derive` 0.8.4, `zerocopy` 0.8.63, `zerocopy-derive` 0.8.63, `zeroize` 1.9.1, `zip` 9.5.1, `zmij` 1.0.23.
* Not pure Rust (a build script that links a native library, or `cc`, `cmake`, `pkg-config`, `bindgen` or `vcpkg` among the build dependencies): none.
