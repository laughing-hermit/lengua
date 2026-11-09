;; empty: does nothing and exits with code 0, so that a benchmark can measure the start and the
;; end of a program. In the WebAssembly dialect 1 (references/power-of-10-webassembly.md): its two
;; assertions check that the memory is the one page it declares, which costs no call to the host.
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)

  ;; Exit codes.
  (global $EXIT_OK i32 (i32.const 0))
  ;; The pages of memory the module declares.
  (global $PAGES i32 (i32.const 1))

  ;; Exits with code 0. Never returns.
  (func $main (export "_start")
    ;; The engine gives the module the memory it declares, no less and no more.
    (if (i32.lt_u (memory.size) (global.get $PAGES))
      (then (call $assert_fail (i32.const 1)) unreachable))
    (if (i32.gt_u (memory.size) (global.get $PAGES))
      (then (call $assert_fail (i32.const 2)) unreachable))
    (call $proc_exit (global.get $EXIT_OK))
    unreachable)
)
