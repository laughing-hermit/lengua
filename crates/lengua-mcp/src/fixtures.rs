use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const BIND_WAIT: Duration = Duration::from_secs(10);
const STEP: Duration = Duration::from_millis(20);
const READ: Duration = Duration::from_secs(2);

pub(crate) struct Fixtures {
    stop: Arc<AtomicBool>,
    servers: Vec<JoinHandle<()>>,
    clients: Vec<JoinHandle<Option<Vec<u8>>>>,
}

fn bind(address: &str) -> Result<TcpListener, String> {
    let started = Instant::now();
    loop {
        match TcpListener::bind(address) {
            Ok(listener) => return Ok(listener),
            Err(error) if started.elapsed() >= BIND_WAIT => {
                return Err(format!(
                    "lengua could not start the test server at `{address}`: {error}"
                ));
            }
            Err(_) => thread::sleep(STEP),
        }
    }
}

fn read_all(stream: &mut TcpStream) -> Vec<u8> {
    let _ = stream.set_read_timeout(Some(READ));
    let mut bytes = Vec::new();
    let _ = stream.read_to_end(&mut bytes);
    bytes
}

fn serve(listener: &TcpListener, answer: &[u8], stop: &AtomicBool, deadline: Instant) {
    if listener.set_nonblocking(true).is_err() {
        return;
    }
    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.write_all(answer);
                let _ = stream.flush();
                let _ = stream.shutdown(Shutdown::Write);
                let _ = read_all(&mut stream);
            }
            Err(_) => thread::sleep(STEP),
        }
    }
}

fn client(port: u16, sent: Vec<u8>, stop: &AtomicBool, deadline: Instant) -> Option<Vec<u8>> {
    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = stream.write_all(&sent);
            let _ = stream.flush();
            let _ = stream.shutdown(Shutdown::Write);
            return Some(read_all(&mut stream));
        }
        thread::sleep(STEP);
    }
    None
}

impl Fixtures {
    pub(crate) fn start(test: &lengua_idea::Test, limit: Duration) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let deadline = Instant::now() + limit;
        let mut servers = Vec::new();
        for (address, answer) in &test.servers {
            let listener = bind(address)?;
            let answer = answer.clone().into_bytes();
            let flag = Arc::clone(&stop);
            servers.push(thread::spawn(move || {
                serve(&listener, &answer, &flag, deadline);
            }));
        }
        let clients = test
            .clients
            .iter()
            .map(|(port, sent)| {
                let port = *port;
                let sent = sent.clone().into_bytes();
                let flag = Arc::clone(&stop);
                thread::spawn(move || client(port, sent, &flag, deadline))
            })
            .collect();
        Ok(Self {
            stop,
            servers,
            clients,
        })
    }

    pub(crate) fn finish(self) -> Vec<Option<Vec<u8>>> {
        self.stop.store(true, Ordering::Relaxed);
        for server in self.servers {
            let _ = server.join();
        }
        self.clients
            .into_iter()
            .map(|c| c.join().ok().flatten())
            .collect()
    }
}
