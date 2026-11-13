use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use lengua_idea::Grants;
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::sockets::SocketAddrUse;

fn named(host: &str) -> bool {
    host.rsplit_once(':')
        .is_some_and(|(name, _)| name.parse::<IpAddr>().is_err())
}

#[must_use]
pub(crate) fn granted_addresses(hosts: &[String]) -> Vec<SocketAddr> {
    let mut out = Vec::new();
    for host in hosts {
        if let Ok(found) = host.as_str().to_socket_addrs() {
            out.extend(found);
        }
    }
    out
}

#[must_use]
pub(crate) fn permitted(
    addr: SocketAddr,
    use_: SocketAddrUse,
    allowed: &[SocketAddr],
    listens: &[u16],
) -> bool {
    let local = addr.ip().is_unspecified() || addr.ip().is_loopback();
    match use_ {
        SocketAddrUse::TcpConnect => allowed.contains(&addr),
        SocketAddrUse::TcpBind => {
            (addr.port() == 0 && addr.ip().is_unspecified() && !allowed.is_empty())
                || (local && listens.contains(&addr.port()))
        }
        SocketAddrUse::TcpListen => local && listens.contains(&addr.port()),
        SocketAddrUse::TcpAccept => !listens.is_empty(),
        SocketAddrUse::UdpBind | SocketAddrUse::UdpSend | SocketAddrUse::UdpReceive => false,
    }
}

pub(crate) fn allow(builder: &mut WasiCtxBuilder, grants: &Grants) {
    if grants.hosts.is_empty() && grants.listens.is_empty() {
        return;
    }
    let allowed = granted_addresses(&grants.hosts);
    let listens = grants.listens.clone();
    builder
        .allow_tcp(true)
        .allow_udp(false)
        .allow_ip_name_lookup(grants.hosts.iter().any(|h| named(h)))
        .socket_addr_check(move |addr, use_| {
            let ok = permitted(addr, use_, &allowed, &listens);
            Box::pin(std::future::ready(ok))
        });
}

#[cfg(test)]
mod tests {
    use super::{granted_addresses, named, permitted};
    use wasmtime_wasi::sockets::SocketAddrUse;

    #[test]
    fn only_the_granted_addresses_and_ports_are_permitted() {
        let allowed = granted_addresses(&["127.0.0.1:7878".to_string()]);
        let granted = "127.0.0.1:7878".parse().expect("an address");
        let other = "127.0.0.1:7879".parse().expect("an address");
        let wildcard = "0.0.0.0:0".parse().expect("an address");
        assert!(permitted(granted, SocketAddrUse::TcpConnect, &allowed, &[]));
        assert!(!permitted(other, SocketAddrUse::TcpConnect, &allowed, &[]));
        assert!(permitted(wildcard, SocketAddrUse::TcpBind, &allowed, &[]));
        assert!(!permitted(wildcard, SocketAddrUse::TcpBind, &[], &[]));
        assert!(!permitted(granted, SocketAddrUse::UdpSend, &allowed, &[]));
        let listen = "0.0.0.0:7879".parse().expect("an address");
        assert!(permitted(listen, SocketAddrUse::TcpBind, &[], &[7879]));
        assert!(permitted(listen, SocketAddrUse::TcpListen, &[], &[7879]));
        assert!(!permitted(listen, SocketAddrUse::TcpListen, &[], &[7880]));
        let far = "10.0.0.1:7879".parse().expect("an address");
        assert!(!permitted(far, SocketAddrUse::TcpBind, &[], &[7879]));
        assert!(named("localhost:80"));
        assert!(!named("127.0.0.1:80"));
    }
}
