#!/usr/bin/env bash
# For CI: compiles the benchmark's C, C++ and Rust programs (benches/programs/<workload>/main.c,
# main.cpp and main.rs) to WebAssembly modules of WASI preview 1, once, on linux-x86_64; every
# runner's bench then runs these same modules in lengua's host, as it runs lengua's own
# (LENGUA_BENCH_WASM names the folder). Usage: bash scripts/ci/bench-wasm.sh <output folder>
#
# It needs `cargo xtask fetch linux-x86_64 wasi`. The output folder gets <workload>/c.wasm, cpp.wasm and
# rust.wasm, and toolchains.tsv: each version's compiler and flags, which the report gives.
#
# The flags are for speed, as a release build's: -O3 and link-time optimization, and every
# WebAssembly feature lengua's engine has on, but no other (FEATURES in src/wasm/engine.rs): LLVM's
# generic CPU, whose sign extension, saturating float-to-int conversions, multiple values,
# reference types, bulk memory and mutable globals are all among them, and SIMD128. Each memory may
# grow to 4 GiB, as lengua's host holds a module to the maximum its memory declares and wasm-ld
# declares none unless told. The stack is 1 MiB, Rust's own for WebAssembly, and the names are
# stripped, as a module that ships has none.
set -euo pipefail

out="${1:?usage: bench-wasm.sh <output folder>}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
sdk="${WASI_SDK:?WASI_SDK names wasi-sdk: run cargo xtask fetch linux-x86_64 wasi first}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

memory=4294967296
common=(--target=wasm32-wasip1 "--sysroot=$sdk/share/wasi-sysroot" -O3 -flto -msimd128
  "-Wl,--max-memory=$memory" -Wl,-z,stack-size=1048576 -Wl,--strip-all)
cflags=("${common[@]}")
cxxflags=("${common[@]}" -fno-exceptions)
rustflags=(--edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat
  -C panic=abort -C target-feature=+simd128 -C strip=symbols -C "link-arg=--max-memory=$memory")

sdk_version="wasi-sdk $(head -1 "$sdk/VERSION" 2> /dev/null || echo "(version unknown)")"
clang_version="$("$sdk/bin/clang" --version | head -1)"
rustc_version="$(rustc --version)"
{
  printf 'c-wasm\t%s, %s\tclang %s\n' "$sdk_version" "$clang_version" "${cflags[*]}"
  printf 'cpp-wasm\t%s, %s\tclang++ %s\n' "$sdk_version" "$clang_version" "${cxxflags[*]}"
  printf 'rust-wasm\t%s\trustc %s\n' "$rustc_version" "${rustflags[*]}"
} > "$out/toolchains.tsv"
cat "$out/toolchains.tsv"

built=0
for dir in "$root"/benches/programs/*/; do
  name="$(basename "$dir")"
  # A folder of lengua's old native versions alone (count) has no C to build.
  [ -f "$dir/main.c" ] || continue
  mkdir -p "$out/$name"
  "$sdk/bin/clang" "${cflags[@]}" -o "$out/$name/c.wasm" "$dir/main.c"
  "$sdk/bin/clang++" "${cxxflags[@]}" -o "$out/$name/cpp.wasm" "$dir/main.cpp"
  rustc "${rustflags[@]}" -o "$out/$name/rust.wasm" "$dir/main.rs"
  ls -l "$out/$name"
  built=$((built + 1))
done
echo "built the C, C++ and Rust modules of $built workloads in $out"
[ "$built" -gt 0 ]
