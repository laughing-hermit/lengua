//! The server speaks MCP correctly in both protocol generations.

mod common;

use common::{Program, Protocol, Session};

#[tokio::test]
async fn offers_check_and_run_in_both_protocols() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let tools = session.service.list_all_tools().await.expect("list tools");
        let mut names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
        names.sort_unstable();
        assert_eq!(names, ["check", "run"], "{protocol:?}");
        for tool in &tools {
            let hints = tool.annotations.as_ref().expect("annotations");
            assert_eq!(hints.read_only_hint, Some(false), "{}", tool.name);
            assert!(
                tool.description
                    .as_deref()
                    .is_some_and(|description| description.contains("path")),
                "{} explains its path",
                tool.name
            );
        }
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn handshake_reports_server_name_version_and_instructions() {
    let session = Session::start(Protocol::Handshake).await;
    let info = session
        .service
        .peer_info()
        .expect("server info after the handshake");
    let server = info.server_info.as_ref().expect("server identity");
    assert_eq!(server.name, "lengua");
    assert_eq!(server.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(
        info.instructions.as_deref(),
        Some(lengua::server::INSTRUCTIONS)
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_new_folder_shows_the_machine_and_the_first_step() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = Program::new();
        let report = session.check(&program).await;
        assert_eq!(report["state"], "write_idea", "{protocol:?}: {report}");
        let machine = report["machine"].as_str().unwrap_or_default();
        let host = format!("{}-{}, ", std::env::consts::OS, std::env::consts::ARCH);
        assert!(machine.starts_with(&host), "{report}");
        assert!(machine.contains("core"), "{report}");
        // What the agent can find out itself isn't repeated: tool versions, the protocol, the
        // data folder.
        for gone in ["toolchain", "client", "data_dir", "server_version"] {
            assert!(report.get(gone).is_none(), "{gone}: {report}");
        }
        session.service.cancel().await.expect("close the session");
    }
}

/// `run` reaches past this computer for a program whose idea names other computers or serves any
/// computer, so its open-world hint says so; `check` runs a program only against the test's own
/// servers, on the loopback.
#[tokio::test]
async fn run_says_it_reaches_the_open_world_and_check_does_not() {
    let session = Session::start(Protocol::Discover).await;
    let tools = session.service.list_all_tools().await.expect("list tools");
    for (name, open) in [("run", true), ("check", false)] {
        let tool = tools
            .iter()
            .find(|tool| tool.name == name)
            .expect("the tool");
        let hints = tool.annotations.as_ref().expect("annotations");
        assert_eq!(hints.open_world_hint, Some(open), "{name}");
    }
    session.service.cancel().await.expect("close the session");
}
