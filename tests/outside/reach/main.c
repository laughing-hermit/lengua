/* reach: asks lengua's host for a TCP connection to port 443 of example.org over TLS, and prints
   what the host answered: `refused` when the idea doesn't grant that computer, `opened` when
   the connection opened. A plain C program with the standard library, which `cargo xtask
   outside-wasm` compiles with wasi-sdk's clang for wasm32-wasip1, and which lengua's suite then
   checks and runs as an outside module under the idea beside it (tests/outside.rs). It imports
   tcp_open from lengua's own module, as a program of any language may. */
#include <stdint.h>
#include <stdio.h>
#include <string.h>

/* lengua's tcp_open(name, name_len, port, tls, out_fd): 0, or a WASI error number. */
__attribute__((import_module("lengua_net_1"), import_name("tcp_open")))
extern int32_t tcp_open(const char *name, int32_t name_len, int32_t port, int32_t tls,
                        int32_t *fd);

/* WASI's error number for what an idea doesn't allow. */
#define NOTCAPABLE 76

int main(void) {
    static const char name[] = "example.org";
    int32_t fd = -1;
    int32_t answer = tcp_open(name, (int32_t)strlen(name), 443, 1, &fd);
    if (answer == NOTCAPABLE) {
        puts("refused");
    } else if (answer == 0) {
        puts("opened");
    } else {
        printf("error %d\n", (int)answer);
    }
    return 0;
}
