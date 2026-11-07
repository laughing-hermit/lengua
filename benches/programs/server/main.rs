//! server in Rust: sends back each line its clients send, in capitals, and keeps each connection
//! open until its client closes its side. Natively it listens on the loopback's port its argument
//! names and serves each client on a thread of its own; under WASI, which has no threads, the
//! host hands it its listener as descriptor 4, as lengua's host does, and it waits on up to 64
//! clients at once with wasi-libc's poll.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::ExitCode;

/// Reads what has come on `stream` and sends it back in capitals: false once the client has
/// closed its side or the connection failed.
fn shout(stream: &mut TcpStream, chunk: &mut [u8]) -> bool {
    match stream.read(chunk) {
        Ok(0) | Err(_) => false,
        Ok(got) => {
            chunk[..got].make_ascii_uppercase();
            stream.write_all(&chunk[..got]).is_ok()
        }
    }
}

#[cfg(not(target_os = "wasi"))]
fn serve(port: &str) -> ExitCode {
    let Ok(port) = port.parse::<u16>() else {
        return ExitCode::from(1);
    };
    let Ok(listener) = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)) else {
        return ExitCode::from(2);
    };
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        std::thread::spawn(move || {
            let mut chunk = [0_u8; 1024];
            while shout(&mut stream, &mut chunk) {}
        });
    }
    ExitCode::SUCCESS
}

/// wasi-libc's poll, which waits with poll_oneoff.
#[cfg(target_os = "wasi")]
mod wasi_poll {
    /// wasi-libc's `struct pollfd`.
    #[repr(C)]
    pub struct PollFd {
        pub fd: i32,
        pub events: i16,
        pub revents: i16,
    }

    /// wasi-libc's POLLIN, POLLERR and POLLHUP.
    pub const POLLIN: i16 = 0x1;
    pub const POLLERR: i16 = 0x1000;
    pub const POLLHUP: i16 = 0x2000;

    unsafe extern "C" {
        pub fn poll(fds: *mut PollFd, count: usize, timeout: i32) -> i32;
    }
}

#[cfg(target_os = "wasi")]
fn serve(_port: &str) -> ExitCode {
    use std::os::fd::{AsRawFd, FromRawFd};
    use wasi_poll::{POLLERR, POLLHUP, POLLIN, PollFd, poll};
    const CLIENTS: usize = 64;
    // SAFETY: the host hands the program its listener as descriptor 4, which nothing else owns.
    let listener = unsafe { TcpListener::from_raw_fd(4) };
    let mut clients: Vec<TcpStream> = Vec::with_capacity(CLIENTS);
    let mut chunk = [0_u8; 1024];
    loop {
        let mut fds: Vec<PollFd> = clients
            .iter()
            .map(|client| PollFd {
                fd: client.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            })
            .collect();
        // The listener, while a slot is free: further clients wait their turn in its queue.
        let listening = clients.len() < CLIENTS;
        if listening {
            fds.push(PollFd {
                fd: listener.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            });
        }
        // SAFETY: `fds` holds `fds.len()` records, which outlive the call.
        if unsafe { poll(fds.as_mut_ptr(), fds.len(), -1) } < 0 {
            continue;
        }
        let was = clients.len();
        // From the last, so a client that leaves takes the place of none still to be served.
        for index in (0..was).rev() {
            if fds[index].revents & (POLLIN | POLLHUP | POLLERR) == 0 {
                continue;
            }
            if !shout(&mut clients[index], &mut chunk) {
                clients.swap_remove(index);
            }
        }
        if listening
            && fds[was].revents & POLLIN != 0
            && let Ok((client, _)) = listener.accept()
        {
            clients.push(client);
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: server <port>");
        return ExitCode::from(1);
    }
    serve(&args[1])
}
