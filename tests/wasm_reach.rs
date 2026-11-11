//! What a program opens by itself, through lengua's pool of workers as a test runs a program, on
//! every system: a module imports lengua's own network functions (`lengua_net_1`), and a test's
//! own internet stands in for every computer. The program looks up a name, sends a datagram and
//! reads the answer, opens a connection and talks on it, and is refused a computer its idea
//! doesn't grant; and a program that receives datagrams answers its sender and no other address.
//! Then two ideas' own tests, as `check` runs them: their steps give a program a computer for
//! its datagrams, a name's addresses, and a client that sends datagrams.
//! Nothing here reaches a real computer: each server is the test's own, on the loopback.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::sync::Arc;

use lengua::reach::{Grants, Stand};
use lengua::runner::{End, Limits, Run};
use lengua::wasm::host::policy::Reach;
use lengua::wasm::{Peer, Policy, Pool, Program, Spec};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// The error number of what an idea doesn't grant, which the programs here exit with.
const NOTCAPABLE: i32 = 76;

/// The longest a test waits for a program to reach one of its own servers, or to answer.
const WAIT: std::time::Duration = std::time::Duration::from_secs(20);

/// This binary, which the pool's workers are.
fn launcher() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// The module `text` builds.
fn program(text: &str) -> Program {
    Program::of_text(text).unwrap_or_else(|problem| panic!("{problem}"))
}

/// The code a run exited with.
fn exited(run: &Run) -> i32 {
    match run.end {
        End::Exited(code) => code,
        other => panic!(
            "it didn't exit: {} ({:?})",
            other.describe(Limits::default()),
            run.supplied
        ),
    }
}

/// What an idea with these `- Network:` lines, in lower case, lets a program reach.
fn policy(lines: &[&str]) -> Policy {
    let mut grants = Grants::default();
    for line in lines {
        assert_eq!(grants.line(line), Some(Ok(())), "{line}");
    }
    Policy {
        reach: Reach::of(&grants),
        ..Policy::default()
    }
}

/// A program that looks up `www.example.org`, sends `ping` as a datagram to port 53 of
/// 192.0.2.53 and prints the answer, opens a connection to port 80 of `www.example.org`, sends
/// `ping` there and prints what comes back, and then asks for `elsewhere.test`, whose error
/// number it exits with. An earlier failure exits with a code of its own, from 10 up.
const REACHES: &str = r#"(module
  (import "lengua_net_1" "tcp_open"
    (func $tcp_open (param i32 i32 i32 i32 i32) (result i32)))
  (import "lengua_net_1" "dns_lookup"
    (func $dns_lookup (param i32 i32 i32 i32 i32) (result i32)))
  (import "lengua_net_1" "udp_open" (func $udp_open (param i32 i32 i32 i32) (result i32)))
  (import "lengua_net_1" "udp_send" (func $udp_send (param i32 i32 i32 i32) (result i32)))
  (import "lengua_net_1" "udp_recv" (func $udp_recv (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (data (i32.const 100) "www.example.org")
  (data (i32.const 120) "192.0.2.53")
  (data (i32.const 140) "elsewhere.test")
  (data (i32.const 160) "ping")
  (func $print (param $at i32) (param $len i32)
    (i32.store (i32.const 0) (local.get $at))
    (i32.store (i32.const 4) (local.get $len))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8))))
  (func (export "_start")
    (local $errno i32)
    (local $fd i32)
    (local.set $errno
      (call $dns_lookup (i32.const 100) (i32.const 15) (i32.const 300) (i32.const 64)
        (i32.const 16)))
    (if (local.get $errno) (then (call $exit (i32.const 10))))
    (if (i32.ne (i32.load (i32.const 16)) (i32.const 2)) (then (call $exit (i32.const 11))))
    (call $print (i32.const 300) (i32.const 34))
    (local.set $errno
      (call $udp_open (i32.const 120) (i32.const 10) (i32.const 53) (i32.const 20)))
    (if (local.get $errno) (then (call $exit (i32.const 20))))
    (local.set $fd (i32.load (i32.const 20)))
    (local.set $errno
      (call $udp_send (local.get $fd) (i32.const 160) (i32.const 4) (i32.const 24)))
    (if (local.get $errno) (then (call $exit (i32.const 21))))
    (local.set $errno
      (call $udp_recv (local.get $fd) (i32.const 400) (i32.const 64) (i32.const 24)))
    (if (local.get $errno) (then (call $exit (i32.const 22))))
    (call $print (i32.const 400) (i32.load (i32.const 24)))
    (drop (call $fd_close (local.get $fd)))
    (local.set $errno
      (call $tcp_open (i32.const 100) (i32.const 15) (i32.const 80) (i32.const 0)
        (i32.const 20)))
    (if (local.get $errno) (then (call $exit (i32.const 30))))
    (local.set $fd (i32.load (i32.const 20)))
    (i32.store (i32.const 0) (i32.const 160))
    (i32.store (i32.const 4) (i32.const 4))
    (local.set $errno
      (call $fd_write (local.get $fd) (i32.const 0) (i32.const 1) (i32.const 8)))
    (if (local.get $errno) (then (call $exit (i32.const 31))))
    (i32.store (i32.const 0) (i32.const 500))
    (i32.store (i32.const 4) (i32.const 64))
    (local.set $errno
      (call $fd_read (local.get $fd) (i32.const 0) (i32.const 1) (i32.const 8)))
    (if (local.get $errno) (then (call $exit (i32.const 32))))
    (call $print (i32.const 500) (i32.load (i32.const 8)))
    (drop (call $fd_close (local.get $fd)))
    (call $exit
      (call $tcp_open (i32.const 140) (i32.const 14) (i32.const 80) (i32.const 0)
        (i32.const 20)))))
"#;

/// A program reaches the test's own servers for the computers its idea grants, by lengua's own
/// functions, and gets `NOTCAPABLE` for a computer it doesn't; the run's report says what
/// became of each thing it asked to open.
#[tokio::test(flavor = "multi_thread")]
async fn a_program_reaches_the_tests_own_servers_and_no_computer_its_idea_doesnt_grant() {
    let policy = policy(&[
        "opens tcp connections to port 80 on any subdomain of example.org",
        "looks up the addresses of www.example.org",
        "sends udp datagrams to port 53 on 192.0.2.53",
    ]);
    let stand = |datagrams, host: &str, port| Stand {
        datagrams,
        host: host.to_owned(),
        port,
    };
    let stands = [
        stand(false, "www.example.org", 80),
        stand(true, "192.0.2.53", 53),
    ];
    let names = [(
        "www.example.org".to_owned(),
        vec!["2001:db8::1".to_owned(), "192.0.2.7".to_owned()],
    )];
    let pool = Pool::new(launcher(), None);
    let spec = Spec {
        stands: Some(&stands),
        names: &names,
        ..Spec::new(&[], &[], policy)
    };
    // The module's file lasts as long as its program: the worker reads it after the job starts.
    let reaches = program(REACHES);
    let mut running = pool
        .start(&reaches, spec, Limits::default())
        .await
        .expect("it starts");
    let peers = running.take_stands();
    assert_eq!(peers.len(), 2);
    let mut servers = Vec::new();
    for (_, peer) in peers {
        servers.push(tokio::spawn(async move {
            match peer {
                Peer::Connections(listener) => {
                    let (mut stream, _) = listener.accept().await.expect("the connection");
                    let mut asked = [0_u8; 4];
                    stream.read_exact(&mut asked).await.expect("the request");
                    assert_eq!(&asked, b"ping");
                    stream.write_all(b"tcp!").await.expect("the answer");
                    stream.shutdown().await.expect("the end");
                }
                Peer::Datagrams(socket) => {
                    let mut chunk = [0_u8; 64];
                    let (count, from) = socket.recv_from(&mut chunk).await.expect("a datagram");
                    assert_eq!(&chunk[..count], b"ping");
                    socket.send_to(b"pong", from).await.expect("the answer");
                }
            }
        }));
    }
    let ran = running.wait().await.expect("it ends");
    assert_eq!(exited(&ran), NOTCAPABLE, "{:?}", ran.connections);
    // The program reached each of the test's servers before it ended.
    for server in servers {
        tokio::time::timeout(WAIT, server)
            .await
            .expect("the program reached the test's server")
            .expect("the test's server");
    }
    // The addresses, IPv4 first, 17 bytes each; then the datagram's answer and the connection's.
    let mut wanted = vec![4_u8];
    wanted.extend(Ipv4Addr::new(192, 0, 2, 7).octets());
    wanted.extend([0_u8; 12]);
    wanted.push(6);
    wanted.extend(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1).octets());
    wanted.extend(b"pong");
    wanted.extend(b"tcp!");
    assert_eq!(ran.stdout, wanted);
    let problems: Vec<Option<&str>> = ran
        .connections
        .iter()
        .map(|made| made.problem.as_deref())
        .collect();
    assert_eq!(problems.len(), 4, "{:?}", ran.connections);
    assert_eq!(&problems[..3], &[None, None, None]);
    assert!(
        problems[3]
            .is_some_and(|why| why.contains("grants no TCP connection to elsewhere.test:80")),
        "{:?}",
        ran.connections
    );
    assert!(
        ran.connections[3].place.contains("`elsewhere.test`"),
        "{:?}",
        ran.connections
    );

    // With no server in the test, a computer the idea grants takes no connection, a name has no
    // address, and nothing answers a datagram: the program's first step fails.
    let spec = Spec::new(&[], &[], policy);
    let alone = pool
        .run(&reaches, spec, Limits::default())
        .await
        .expect("it runs");
    assert_eq!(exited(&alone), 10);
    // And a program whose idea grants nothing is refused at once.
    let bare = pool
        .run(
            &reaches,
            Spec {
                stands: Some(&stands),
                names: &names,
                ..Spec::new(&[], &[], Policy::default())
            },
            Limits::default(),
        )
        .await
        .expect("it runs");
    assert_eq!(exited(&bare), 10);
}

/// A program that takes one datagram on the port its idea receives on, descriptor 5, sends it
/// back to its sender, and then tries an address that sent nothing, whose error number it exits
/// with.
const ANSWERS: &str = r#"(module
  (import "lengua_net_1" "udp_recv_from"
    (func $recv_from (param i32 i32 i32 i32 i32) (result i32)))
  (import "lengua_net_1" "udp_send_to"
    (func $send_to (param i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (local $errno i32)
    (local.set $errno
      (call $recv_from (i32.const 5) (i32.const 200) (i32.const 64) (i32.const 8)
        (i32.const 100)))
    (if (local.get $errno) (then (call $exit (i32.const 10))))
    (local.set $errno
      (call $send_to (i32.const 5) (i32.const 200) (i32.load (i32.const 8)) (i32.const 100)
        (i32.const 12)))
    (if (local.get $errno) (then (call $exit (i32.const 11))))
    ;; The same computer's port 9, which sent nothing.
    (i32.store16 (i32.const 117) (i32.const 9))
    (call $exit
      (call $send_to (i32.const 5) (i32.const 200) (i32.const 1) (i32.const 100)
        (i32.const 12)))))
"#;

/// A program whose idea receives datagrams gets its port from lengua, after the connections
/// lengua made, answers the test's client there, and is refused an address that sent nothing.
#[tokio::test(flavor = "multi_thread")]
async fn a_program_that_receives_datagrams_answers_its_sender_and_no_other_address() {
    let policy = policy(&["receives udp datagrams on port 5353"]);
    let pool = Pool::new(launcher(), None);
    let answers = program(ANSWERS);
    let running = pool
        .start(&answers, Spec::new(&[], &[], policy), Limits::default())
        .await
        .expect("it starts");
    // In a test the port is a free one of the loopback, which the test's client is told.
    let port = running.port().expect("the port it receives on");
    assert_ne!(port, 5353);
    let client = tokio::net::UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("a free port");
    client
        .send_to(b"hello", (Ipv4Addr::LOCALHOST, port))
        .await
        .expect("a datagram");
    let mut chunk = [0_u8; 64];
    let (count, from) = tokio::time::timeout(WAIT, client.recv_from(&mut chunk))
        .await
        .expect("the program answered")
        .expect("the answer");
    assert_eq!(&chunk[..count], b"hello");
    assert_eq!(from.port(), port);
    let ran = running.wait().await.expect("it ends");
    assert_eq!(exited(&ran), NOTCAPABLE);
}

/// An idea whose `# Behaviors` and `# Constraints` are `behaviors` and `constraints`, with words
/// of `1` to `8` letters.
fn idea_of(behaviors: &str, constraints: &str) -> String {
    format!(
        "---\ntype: lengua/program-idea\ntitle: t\ndescription: A test.\n---\n# Purpose\nIt takes \
         words of `1` to `8` letters.\n\n# Behaviors\n{behaviors}\n\n# Constraints\n{constraints}\n"
    )
}

/// Runs every test of the idea `text` on the module `code` builds, as `check` runs a program's
/// tests, and gives how many runs there were and what failed.
async fn tests_of(text: &str, code: &str) -> (usize, Vec<String>) {
    let idea = lengua::idea::parse(text).unwrap_or_else(|problems| panic!("{problems:?}"));
    let suite = lengua::testfile::check(&idea).unwrap_or_else(|problems| panic!("{problems:?}"));
    let pool = Pool::new(launcher(), None);
    let outcome = lengua::testrun::run_wasm_tests(
        &suite,
        &pool,
        Arc::new(program(code)),
        7,
        &idea.uses,
        &idea.network,
        4,
    )
    .await
    .expect("its tests run");
    (outcome.runs, outcome.failures)
}

/// A program that sends its argument as a datagram to port 53 of 192.0.2.53 and prints the
/// answer, once `www.example.org` has the one IPv4 address its test gives it. A failure exits
/// with a code of its own, from 10 up.
const ASKS: &str = r#"(module
  (import "lengua_net_1" "dns_lookup"
    (func $dns_lookup (param i32 i32 i32 i32 i32) (result i32)))
  (import "lengua_net_1" "udp_open" (func $udp_open (param i32 i32 i32 i32) (result i32)))
  (import "lengua_net_1" "udp_send" (func $udp_send (param i32 i32 i32 i32) (result i32)))
  (import "lengua_net_1" "udp_recv" (func $udp_recv (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (data (i32.const 100) "www.example.org")
  (data (i32.const 120) "192.0.2.53")
  (func (export "_start")
    (local $fd i32)
    (local $word i32)
    (local $len i32)
    (drop (call $args_sizes_get (i32.const 0) (i32.const 4)))
    (drop (call $args_get (i32.const 600) (i32.const 700)))
    (local.set $word (i32.load (i32.const 604)))
    (block $ended
      (loop $byte
        (br_if $ended (i32.eqz (i32.load8_u (i32.add (local.get $word) (local.get $len)))))
        (local.set $len (i32.add (local.get $len) (i32.const 1)))
        (br $byte)))
    (if (call $dns_lookup (i32.const 100) (i32.const 15) (i32.const 300) (i32.const 64)
          (i32.const 16))
      (then (call $exit (i32.const 10))))
    (if (i32.ne (i32.load (i32.const 16)) (i32.const 1)) (then (call $exit (i32.const 11))))
    (if (i32.ne (i32.load8_u (i32.const 300)) (i32.const 4)) (then (call $exit (i32.const 12))))
    (if (call $udp_open (i32.const 120) (i32.const 10) (i32.const 53) (i32.const 20))
      (then (call $exit (i32.const 20))))
    (local.set $fd (i32.load (i32.const 20)))
    (if (call $udp_send (local.get $fd) (local.get $word) (local.get $len) (i32.const 24))
      (then (call $exit (i32.const 21))))
    (if (call $udp_recv (local.get $fd) (i32.const 400) (i32.const 64) (i32.const 24))
      (then (call $exit (i32.const 22))))
    (i32.store (i32.const 0) (i32.const 400))
    (i32.store (i32.const 4) (i32.load (i32.const 24)))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8)))
    (call $exit (i32.const 0))))
"#;

/// An idea's steps give its program a computer for its datagrams and a name's addresses: the
/// test's own computer reads exactly the datagram the step says and answers with the one it
/// says, in the example and in every generated run.
#[tokio::test(flavor = "multi_thread")]
async fn an_ideas_steps_give_a_program_a_computer_for_its_datagrams_and_a_names_addresses() {
    let idea = idea_of(
        "## B1: Prints the answer to its word\n- Given the argument `abc`\n- And the addresses \
         `192.0.2.7` for the name `www.example.org`\n- And a computer at `192.0.2.53` on UDP \
         port `53` that receives `abc` and replies `xyz`\n- When the program runs\n- Then it \
         prints `xyz`\n- And it exits with code `0`\n\n### For any `word` and `answer`\n- Given \
         the argument `{word}`\n- And the addresses `192.0.2.7` for the name \
         `www.example.org`\n- And a computer at `192.0.2.53` on UDP port `53` that receives \
         `{word}` and replies `{answer}`\n- When the program runs\n- Then it prints \
         `{answer}`\n- And it exits with code `0`\n\n| word | answer | runs |\n|---|---|---|\n| \
         `string(1, 8, \"abc\")` | `string(1, 8, \"xyz\")` | 20 |",
        "- Network: looks up the addresses of any subdomain of example.org\n- Network: sends UDP \
         datagrams to port 53 on 192.0.2.53",
    );
    let (runs, failures) = tests_of(&idea, ASKS).await;
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(runs > 1, "{runs} runs");
}

/// A program that sends each datagram that arrives on the port its idea receives on, descriptor
/// 5, back to its sender, for ever.
const ECHOES: &str = r#"(module
  (import "lengua_net_1" "udp_recv_from"
    (func $recv_from (param i32 i32 i32 i32 i32) (result i32)))
  (import "lengua_net_1" "udp_send_to"
    (func $send_to (param i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (loop $serve
      (if (call $recv_from (i32.const 5) (i32.const 200) (i32.const 64) (i32.const 8)
            (i32.const 100))
        (then (call $exit (i32.const 10))))
      (if (call $send_to (i32.const 5) (i32.const 200) (i32.load (i32.const 8)) (i32.const 100)
            (i32.const 12))
        (then (call $exit (i32.const 11))))
      (br $serve))))
"#;

/// An idea that receives datagrams has a test's client that sends them: each of its steps is one
/// datagram to the program's port, and the datagram the program answers with, and the program
/// still waits for the next one at the end.
#[tokio::test(flavor = "multi_thread")]
async fn an_ideas_client_sends_datagrams_to_a_program_that_receives_them() {
    let idea = idea_of(
        "## B1: Sends a datagram back to its sender\n- Given a client that sends `abc`\n- When \
         the program runs\n- Then the client receives `abc`\n- And it does not stop\n\n### For \
         any `word`\n- Given a client that sends `{word}`\n- When the program runs\n- Then the \
         client receives `{word}`\n- And it does not stop\n\n| word | runs |\n|---|---|\n| \
         `string(1, 8, \"abc\")` | 20 |",
        "- Network: receives UDP datagrams on port 5353",
    );
    let (runs, failures) = tests_of(&idea, ECHOES).await;
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(runs > 1, "{runs} runs");
}
