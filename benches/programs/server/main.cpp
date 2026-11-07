// server in C++: sends back each line its clients send, in capitals, for up to 64 clients at
// once, waiting on all of them with poll, and keeps each connection open until its client closes
// its side. Natively it listens on the loopback's port its argument names; under WASI the host
// hands it its listener as descriptor 4, as lengua's host does, and the argument is the same.
#include <array>
#include <cstdio>
#include <cstdlib>
#include <vector>
#if defined(_WIN32)
#include <winsock2.h>
#include <ws2tcpip.h>
using bench_socket = SOCKET;
using bench_pollfd = WSAPOLLFD;
constexpr bench_socket invalid = INVALID_SOCKET;
static int bench_poll(bench_pollfd* fds, std::size_t count) {
    return WSAPoll(fds, static_cast<ULONG>(count), -1);
}
static void bench_close(bench_socket socket) { closesocket(socket); }
#else
#include <poll.h>
#include <sys/socket.h>
#include <unistd.h>
#if !defined(__wasi__)
#include <arpa/inet.h>
#include <netinet/in.h>
#endif
using bench_socket = int;
using bench_pollfd = pollfd;
constexpr bench_socket invalid = -1;
static int bench_poll(bench_pollfd* fds, std::size_t count) {
    return poll(fds, static_cast<nfds_t>(count), -1);
}
static void bench_close(bench_socket socket) { close(socket); }
#endif

namespace {
constexpr std::size_t clients_at_once = 64;

// The listener: the host's under WASI, else one on the loopback's port.
bench_socket listener_on(const char* port) {
#if defined(__wasi__)
    (void)port;
    return 4;
#else
    long number = std::strtol(port, nullptr, 10);
    if (number < 1 || number > 65535) {
        return invalid;
    }
    bench_socket listener = socket(AF_INET, SOCK_STREAM, 0);
    if (listener == invalid) {
        return invalid;
    }
#if !defined(_WIN32)
    int on = 1;
    setsockopt(listener, SOL_SOCKET, SO_REUSEADDR, &on, sizeof on);
#endif
    sockaddr_in address{};
    address.sin_family = AF_INET;
    address.sin_port = htons(static_cast<unsigned short>(number));
    address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    if (bind(listener, reinterpret_cast<sockaddr*>(&address), sizeof address) != 0 ||
        listen(listener, 128) != 0) {
        bench_close(listener);
        return invalid;
    }
    return listener;
#endif
}

// Sends all of the count bytes at buffer on client; false when a send fails.
bool send_all(bench_socket client, const char* buffer, int count) {
    while (count > 0) {
        int sent = static_cast<int>(send(client, buffer, static_cast<std::size_t>(count), 0));
        if (sent <= 0) {
            return false;
        }
        buffer += sent;
        count -= sent;
    }
    return true;
}
}  // namespace

int main(int argc, char** argv) {
#if defined(_WIN32)
    WSADATA data;
    if (WSAStartup(MAKEWORD(2, 2), &data) != 0) {
        return 2;
    }
#endif
    if (argc != 2) {
        std::fputs("usage: server <port>\n", stderr);
        return 1;
    }
    bench_socket listener = listener_on(argv[1]);
    if (listener == invalid) {
        return 2;
    }
    std::vector<bench_socket> clients;
    clients.reserve(clients_at_once);
    std::array<bench_pollfd, clients_at_once + 1> fds{};
    std::array<char, 1024> chunk{};
    for (;;) {
        std::size_t waits = 0;
        for (; waits < clients.size(); waits++) {
            fds[waits] = bench_pollfd{};
            fds[waits].fd = clients[waits];
            fds[waits].events = POLLIN;
        }
        // The listener, while a slot is free: further clients wait their turn in its queue.
        bool listening = clients.size() < clients_at_once;
        if (listening) {
            fds[waits] = bench_pollfd{};
            fds[waits].fd = listener;
            fds[waits].events = POLLIN;
            waits++;
        }
        if (bench_poll(fds.data(), waits) < 0) {
            continue;
        }
        std::size_t was = clients.size();
        // From the last, so a client that leaves takes the place of none still to be served.
        for (std::size_t i = was; i-- > 0;) {
            if ((fds[i].revents & (POLLIN | POLLHUP | POLLERR)) == 0) {
                continue;
            }
            int got = static_cast<int>(recv(clients[i], chunk.data(), chunk.size(), 0));
            bool keep = got > 0;
            for (int k = 0; k < got; k++) {
                if (chunk[k] >= 'a' && chunk[k] <= 'z') {
                    chunk[k] = static_cast<char>(chunk[k] - ('a' - 'A'));
                }
            }
            if (keep && !send_all(clients[i], chunk.data(), got)) {
                keep = false;
            }
            if (!keep) {
                bench_close(clients[i]);
                clients[i] = clients.back();
                clients.pop_back();
            }
        }
        if (listening && (fds[was].revents & POLLIN) != 0) {
            bench_socket client = accept(listener, nullptr, nullptr);
            if (client != invalid) {
                clients.push_back(client);
            }
        }
    }
}
