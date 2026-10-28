//! The server speaks MCP correctly in both protocol generations.

mod common;

use common::{Protocol, Session};
use serde_json::json;

#[tokio::test]
async fn lists_status_as_a_read_only_tool_in_both_protocols() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let tools = session.service.list_all_tools().await.expect("list tools");
        let status = tools
            .iter()
            .find(|tool| tool.name == "status")
            .unwrap_or_else(|| panic!("{protocol:?}: no status tool in {tools:?}"));
        let annotations = status.annotations.as_ref().expect("status has annotations");
        assert_eq!(annotations.read_only_hint, Some(true), "{protocol:?}");
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
async fn status_reports_machine_toolchain_and_client() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let report = session.call_ok("status", json!({})).await;
        assert_eq!(
            report["machine"]["os"],
            std::env::consts::OS,
            "{protocol:?}"
        );
        assert_eq!(
            report["machine"]["arch"],
            std::env::consts::ARCH,
            "{protocol:?}"
        );
        assert_eq!(report["machine"]["supported"], true, "{protocol:?}");
        assert_eq!(report["machine"]["profile"], "linux-x86_64", "{protocol:?}");
        assert!(
            report["toolchain"]["assembler"]["version"].is_string(),
            "{report}"
        );
        assert!(
            report["toolchain"]["linker"]["version"].is_string(),
            "{report}"
        );
        let expected_protocol = match protocol {
            Protocol::Handshake => "2025-11-25",
            Protocol::Discover => "2026-07-28",
        };
        assert_eq!(
            report["client"]["protocol_version"], expected_protocol,
            "{report}"
        );
        assert_eq!(
            report["data_dir"],
            session.data_dir.path().display().to_string(),
            "{protocol:?}"
        );
        assert!(
            report["next_step"]
                .as_str()
                .is_some_and(|step| step.starts_with("Ready")),
            "{report}"
        );
        session.service.cancel().await.expect("close the session");
    }
}
