use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const DEVTOOLS: u16 = 9223;
const PAGE_PORT: u16 = 7812;

struct Killed(Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn chrome() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("LENGUA_CHROME").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    [
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
}

fn http(port: u16, path: &str) -> Option<Vec<u8>> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).ok()?;
    let at = answer.windows(4).position(|w| w == b"\r\n\r\n")?;
    Some(answer.split_off(at + 4))
}

fn wait_for<T>(seconds: u64, mut probe: impl FnMut() -> Option<T>) -> Option<T> {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(seconds) {
        if let Some(found) = probe() {
            return Some(found);
        }
        thread::sleep(Duration::from_millis(250));
    }
    None
}

struct Socket {
    stream: TcpStream,
    next: u64,
    events: Vec<Value>,
}

impl Socket {
    fn open(url: &str) -> Self {
        let rest = url.strip_prefix("ws://").expect("a ws url");
        let (host, path) = rest.split_once('/').expect("a path");
        let mut stream = TcpStream::connect(host).expect("the devtools socket");
        let request = format!(
            "GET /{path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).expect("the handshake");
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            stream
                .read_exact(&mut byte)
                .expect("the handshake's answer");
            head.push(byte[0]);
        }
        assert!(
            String::from_utf8_lossy(&head).starts_with("HTTP/1.1 101"),
            "{}",
            String::from_utf8_lossy(&head)
        );
        stream
            .set_read_timeout(Some(Duration::from_secs(60)))
            .expect("a timeout");
        Self {
            stream,
            next: 1,
            events: Vec::new(),
        }
    }

    fn send_text(&mut self, text: &str) {
        let payload = text.as_bytes();
        let mut frame = vec![0x81u8];
        let length = payload.len();
        if length < 126 {
            frame.push(0x80 | u8::try_from(length).expect("a small length"));
        } else if length < 65_536 {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&u16::try_from(length).expect("a length").to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&u64::try_from(length).expect("a length").to_be_bytes());
        }
        let mask = [0x12u8, 0x34, 0x56, 0x78];
        frame.extend_from_slice(&mask);
        frame.extend(payload.iter().zip(mask.iter().cycle()).map(|(b, m)| b ^ m));
        self.stream.write_all(&frame).expect("a frame");
    }

    fn read_message(&mut self) -> Value {
        let mut message = Vec::new();
        loop {
            let mut head = [0u8; 2];
            self.stream.read_exact(&mut head).expect("a frame");
            let fin = head[0] & 0x80 != 0;
            let opcode = head[0] & 0x0f;
            let mut length = u64::from(head[1] & 0x7f);
            if length == 126 {
                let mut more = [0u8; 2];
                self.stream.read_exact(&mut more).expect("a length");
                length = u64::from(u16::from_be_bytes(more));
            } else if length == 127 {
                let mut more = [0u8; 8];
                self.stream.read_exact(&mut more).expect("a length");
                length = u64::from_be_bytes(more);
            }
            let mut payload = vec![0u8; usize::try_from(length).expect("a size")];
            self.stream.read_exact(&mut payload).expect("a payload");
            if opcode == 0x9 || opcode == 0xa {
                continue;
            }
            message.extend_from_slice(&payload);
            if fin {
                return serde_json::from_slice(&message).expect("a JSON message");
            }
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send_text(&json!({"id": id, "method": method, "params": params}).to_string());
        loop {
            let message = self.read_message();
            if message["id"] == json!(id) {
                return message;
            }
            self.events.push(message);
        }
    }

    fn evaluate(&mut self, expression: &str) -> Value {
        let reply = self.call(
            "Runtime.evaluate",
            json!({"expression": expression, "returnByValue": true}),
        );
        reply["result"]["result"]["value"].clone()
    }
}

fn decode(text: &str) -> Vec<u8> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for byte in text.bytes() {
        let Some(value) = alphabet.iter().position(|a| *a == byte) else {
            continue;
        };
        buffer = (buffer << 6) | u32::try_from(value).expect("a digit");
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((buffer >> bits) & 0xff).expect("a byte"));
        }
    }
    out
}

fn page_socket() -> Socket {
    let list = wait_for(60, || {
        let body = http(DEVTOOLS, "/json/list")?;
        let pages: Value = serde_json::from_slice(&body).ok()?;
        pages.as_array()?.iter().find_map(|page| {
            let url = page["url"].as_str()?;
            (page["type"] == "page" && url.contains(&format!(":{PAGE_PORT}")))
                .then(|| page["webSocketDebuggerUrl"].as_str().map(str::to_string))
                .flatten()
        })
    })
    .expect("the page in Chrome's list of targets");
    Socket::open(&list)
}

fn names(tree: &Value) -> Vec<String> {
    tree["result"]["nodes"]
        .as_array()
        .map(|nodes| {
            nodes
                .iter()
                .filter(|n| n["ignored"] != json!(true))
                .map(|n| {
                    format!(
                        "{} {:?}",
                        n["role"]["value"].as_str().unwrap_or("?"),
                        n["name"]["value"].as_str().unwrap_or("")
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn start_chrome(chrome: &Path) -> Killed {
    let profile = std::env::temp_dir().join(format!("lengua-chrome-{}", std::process::id()));
    Killed(
        Command::new(chrome)
            .args([
                "--headless=new",
                "--no-sandbox",
                "--no-first-run",
                "--use-angle=swiftshader",
                "--enable-unsafe-swiftshader",
                "--window-size=1024,768",
                &format!("--remote-debugging-port={DEVTOOLS}"),
                &format!("--user-data-dir={}", profile.display()),
                &format!("http://127.0.0.1:{PAGE_PORT}/"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Chrome starts"),
    )
}

fn events(socket: &Socket, method: &str, show: impl Fn(&Value) -> String) -> Vec<String> {
    socket
        .events
        .iter()
        .filter(|e| e["method"] == method)
        .map(show)
        .collect()
}

fn screenshot(socket: &mut Socket) -> Vec<u8> {
    let shot = socket.call("Page.captureScreenshot", json!({"format": "png"}));
    decode(shot["result"]["data"].as_str().unwrap_or_default())
}

fn serve_the_notes(folder: &Path) -> Killed {
    let server = Killed(
        Command::new(env!("CARGO_BIN_EXE_lengua-run"))
            .arg(folder)
            .stdin(Stdio::null())
            .spawn()
            .expect("lengua-run starts"),
    );
    let page = wait_for(60, || http(PAGE_PORT, "/").filter(|b| !b.is_empty())).expect("the page");
    let page = String::from_utf8_lossy(&page).to_string();
    assert!(page.contains("<title>notes</title>"), "{page}");
    let code = http(PAGE_PORT, "/app_bg.wasm").expect("the code");
    assert!(code.starts_with(b"\0asm"), "{} bytes", code.len());
    println!(
        "the page: {} bytes; the code: {} bytes",
        page.len(),
        code.len()
    );
    server
}

#[test]
#[ignore = "needs a checked examples/notes folder (LENGUA_NOTES) and Chrome; CI runs it after the tests"]
fn the_notes_open_in_a_browser() {
    let folder = PathBuf::from(std::env::var("LENGUA_NOTES").expect("LENGUA_NOTES"));
    let out = PathBuf::from(
        std::env::var("LENGUA_BROWSER_OUT").unwrap_or_else(|_| "browser".to_string()),
    );
    fs::create_dir_all(&out).expect("the output folder");
    let Some(chrome) = chrome() else {
        println!("no Chrome on this system; the browser test did not run");
        return;
    };
    let _server = serve_the_notes(&folder);
    let _chrome = start_chrome(&chrome);
    let mut socket = page_socket();
    socket.call("Runtime.enable", json!({}));
    socket.call("Network.enable", json!({}));
    let started = wait_for(90, || {
        let state = socket.evaluate(
            "document.body.dataset.lengua + ' ' + document.querySelectorAll('canvas').length",
        );
        let text = state.as_str().unwrap_or_default().to_string();
        (text == "started 1" || text.starts_with("failed")).then_some(text)
    });
    assert_eq!(
        started.as_deref(),
        Some("started 1"),
        "{:#?}",
        socket.events
    );
    thread::sleep(Duration::from_secs(5));
    let png = screenshot(&mut socket);
    fs::write(out.join("notes.png"), &png).expect("the screenshot");
    socket.call("Accessibility.enable", json!({}));
    let read = names(&socket.call("Accessibility.getFullAXTree", json!({})));
    fs::write(out.join("notes-accessibility.txt"), read.join("\n")).expect("the tree");
    println!(
        "what a screen reader can read ({} nodes):\n{}",
        read.len(),
        read.join("\n")
    );
    socket.call("Page.navigate", json!({"url": "about:blank"}));
    thread::sleep(Duration::from_secs(2));
    let blank = screenshot(&mut socket);
    let mut console = events(&socket, "Runtime.consoleAPICalled", |e| {
        e["params"]["args"].to_string()
    });
    console.extend(events(&socket, "Runtime.exceptionThrown", |e| {
        e["params"].to_string()
    }));
    let fetched = events(&socket, "Network.responseReceived", |e| {
        let response = &e["params"]["response"];
        format!(
            "{} {}",
            response["status"],
            response["url"].as_str().unwrap_or_default()
        )
    });
    println!("the console:\n{}", console.join("\n"));
    println!("the answers the page got:\n{}", fetched.join("\n"));
    println!(
        "the screenshot: {} bytes; a blank page: {} bytes",
        png.len(),
        blank.len()
    );
    assert!(png.starts_with(b"\x89PNG"), "a PNG");
    assert!(
        png.len() > blank.len() * 3 / 2,
        "the screen is drawn: {} and {}",
        png.len(),
        blank.len()
    );
    assert!(
        !console.iter().any(|c| c.contains("panicked")),
        "{console:#?}"
    );
    assert!(
        fetched
            .iter()
            .any(|f| f.starts_with("200 ") && f.ends_with("/api/notes")),
        "the screens asked their server for the notes: {fetched:#?}"
    );
    assert!(!read.is_empty(), "the accessibility tree");
    assert!(
        !read.iter().any(|r| r.contains("\"Milk\"")),
        "GPUI's web target now gives its text to screen readers; write it down in Lengua3.md: {read:#?}"
    );
}
