/* server in C: sends back each line its clients send, in capitals, for up to 64 clients at once,
   waiting on all of them with poll, and keeps each connection open until its client closes its
   side. Natively it listens on the loopback's port its argument names; under WASI the host hands
   it its listener as descriptor 4, as lengua's host does, and the argument is the same. */
#include <stdio.h>
#include <stdlib.h>
#if defined(_WIN32)
#include <winsock2.h>
#include <ws2tcpip.h>
typedef SOCKET bench_socket;
typedef WSAPOLLFD bench_pollfd;
#define BENCH_INVALID INVALID_SOCKET
#define bench_poll(fds, count) WSAPoll((fds), (ULONG)(count), -1)
#define bench_close closesocket
#else
#include <poll.h>
#include <sys/socket.h>
#include <unistd.h>
#if !defined(__wasi__)
#include <arpa/inet.h>
#include <netinet/in.h>
#endif
typedef int bench_socket;
typedef struct pollfd bench_pollfd;
#define BENCH_INVALID (-1)
#define bench_poll(fds, count) poll((fds), (nfds_t)(count), -1)
#define bench_close close
#endif

#define CLIENTS 64
#define CHUNK 1024

/* The listener: the host's under WASI, else one on the loopback's port. */
static bench_socket listener_on(const char *port) {
#if defined(__wasi__)
    (void)port;
    return 4;
#else
    long number = strtol(port, NULL, 10);
    if (number < 1 || number > 65535) {
        return BENCH_INVALID;
    }
    bench_socket listener = socket(AF_INET, SOCK_STREAM, 0);
    if (listener == BENCH_INVALID) {
        return BENCH_INVALID;
    }
#if !defined(_WIN32)
    int on = 1;
    setsockopt(listener, SOL_SOCKET, SO_REUSEADDR, &on, sizeof on);
#endif
    struct sockaddr_in address = {0};
    address.sin_family = AF_INET;
    address.sin_port = htons((unsigned short)number);
    address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    if (bind(listener, (struct sockaddr *)&address, sizeof address) != 0 ||
        listen(listener, 128) != 0) {
        bench_close(listener);
        return BENCH_INVALID;
    }
    return listener;
#endif
}

/* Sends all of the count bytes at buffer on client. Gives 0, or -1 when a send fails. */
static int send_all(bench_socket client, const char *buffer, int count) {
    while (count > 0) {
        int sent = (int)send(client, buffer, (size_t)count, 0);
        if (sent <= 0) {
            return -1;
        }
        buffer += sent;
        count -= sent;
    }
    return 0;
}

int main(int argc, char **argv) {
#if defined(_WIN32)
    WSADATA data;
    if (WSAStartup(MAKEWORD(2, 2), &data) != 0) {
        return 2;
    }
#endif
    if (argc != 2) {
        fputs("usage: server <port>\n", stderr);
        return 1;
    }
    bench_socket listener = listener_on(argv[1]);
    if (listener == BENCH_INVALID) {
        return 2;
    }
    bench_socket clients[CLIENTS];
    bench_pollfd fds[CLIENTS + 1];
    char chunk[CHUNK];
    size_t count = 0;
    for (;;) {
        size_t waits = 0;
        for (; waits < count; waits++) {
            fds[waits].fd = clients[waits];
            fds[waits].events = POLLIN;
            fds[waits].revents = 0;
        }
        /* The listener, while a slot is free: further clients wait their turn in its queue. */
        int listening = count < CLIENTS;
        if (listening) {
            fds[waits].fd = listener;
            fds[waits].events = POLLIN;
            fds[waits].revents = 0;
            waits++;
        }
        if (bench_poll(fds, waits) < 0) {
            continue;
        }
        size_t was = count;
        /* From the last, so a client that leaves takes the place of none still to be served. */
        for (size_t i = was; i-- > 0;) {
            if ((fds[i].revents & (POLLIN | POLLHUP | POLLERR)) == 0) {
                continue;
            }
            int got = (int)recv(clients[i], chunk, sizeof chunk, 0);
            int keep = got > 0;
            for (int k = 0; k < got; k++) {
                if (chunk[k] >= 'a' && chunk[k] <= 'z') {
                    chunk[k] = (char)(chunk[k] - ('a' - 'A'));
                }
            }
            if (keep && send_all(clients[i], chunk, got) != 0) {
                keep = 0;
            }
            if (!keep) {
                bench_close(clients[i]);
                clients[i] = clients[--count];
            }
        }
        if (listening && (fds[was].revents & POLLIN) != 0) {
            bench_socket client = accept(listener, NULL, NULL);
            if (client != BENCH_INVALID) {
                clients[count++] = client;
            }
        }
    }
}
