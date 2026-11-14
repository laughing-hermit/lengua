use std::sync::{Arc, OnceLock};

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

pub const ROOTS: &str = "LENGUA_TLS_ROOTS";

pub(crate) fn connector_with(extra: &[CertificateDer<'static>]) -> Result<TlsConnector, String> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for certificate in extra {
        roots.add(certificate.clone()).map_err(|e| {
            format!("a certificate of `{ROOTS}` is not a root lengua can trust: {e}")
        })?;
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| format!("lengua could not make its TLS settings: {e}"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(TlsConnector::from(Arc::new(config)))
}

fn extra_roots() -> Result<Vec<CertificateDer<'static>>, String> {
    let Some(path) = std::env::var_os(ROOTS).filter(|p| !p.is_empty()) else {
        return Ok(Vec::new());
    };
    let shown = path.to_string_lossy().to_string();
    CertificateDer::pem_file_iter(&path)
        .map_err(|e| format!("lengua could not read `{ROOTS}`, `{shown}`: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("`{ROOTS}`, `{shown}`, is not a file of PEM certificates: {e}"))
}

pub(crate) fn connector() -> Result<TlsConnector, String> {
    static ONE: OnceLock<Result<TlsConnector, String>> = OnceLock::new();
    ONE.get_or_init(|| extra_roots().and_then(|extra| connector_with(&extra)))
        .clone()
}

pub(crate) async fn wrap(
    connector: &TlsConnector,
    host: &str,
    stream: TcpStream,
) -> std::io::Result<TlsStream<TcpStream>> {
    let name = ServerName::try_from(host.to_string())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    connector.connect(name, stream).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use http_body_util::BodyExt;
    use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair};
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_rustls::TlsAcceptor;

    use super::connector_with;
    use crate::outgoing::{granted_name, send_with, whole};

    struct Made {
        root: CertificateDer<'static>,
        leaf: CertificateDer<'static>,
        key: PrivateKeyDer<'static>,
    }

    fn made() -> Made {
        let root_key = KeyPair::generate().expect("a key");
        let mut root_params = CertificateParams::new(Vec::<String>::new()).expect("parameters");
        root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let root = root_params.self_signed(&root_key).expect("a root");
        let leaf_key = KeyPair::generate().expect("a key");
        let leaf_params =
            CertificateParams::new(vec!["127.0.0.1".to_string()]).expect("parameters");
        let issuer = Issuer::from_params(&root_params, &root_key);
        let leaf = leaf_params.signed_by(&leaf_key, &issuer).expect("a leaf");
        Made {
            root: root.der().clone(),
            leaf: leaf.der().clone(),
            key: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der())),
        }
    }

    async fn serve_once(made: &Made) -> u16 {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("versions")
            .with_no_client_auth()
            .with_single_cert(vec![made.leaf.clone()], made.key.clone_key())
            .expect("a server configuration");
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a port");
        let port = listener.local_addr().expect("an address").port();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let Ok(mut tls) = acceptor.accept(stream).await else {
                return;
            };
            let mut seen = Vec::new();
            let mut buffer = [0u8; 1024];
            while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                match tls.read(&mut buffer).await {
                    Ok(0) | Err(_) => return,
                    Ok(n) => seen.extend_from_slice(buffer.get(..n).unwrap_or_default()),
                }
            }
            let _ = tls
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-length: 6\r\nconnection: close\r\n\r\nsecret",
                )
                .await;
            let _ = tls.shutdown().await;
        });
        port
    }

    fn request(port: u16) -> hyper::Request<wasmtime_wasi_http::WasiBody> {
        hyper::Request::builder()
            .method("GET")
            .uri(format!("https://127.0.0.1:{port}/answer"))
            .header("host", format!("127.0.0.1:{port}"))
            .body(whole(Vec::new()))
            .expect("a request")
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime")
    }

    #[test]
    fn https_reaches_a_server_whose_certificate_a_trusted_root_signed() {
        let made = made();
        runtime().block_on(async {
            let port = serve_once(&made).await;
            let connector = connector_with(std::slice::from_ref(&made.root)).expect("a connector");
            let (response, _) = send_with(request(port), None, Some(&connector))
                .await
                .expect("an answer over TLS");
            assert_eq!(response.status().as_u16(), 200);
            let body = response
                .into_body()
                .collect()
                .await
                .expect("a body")
                .to_bytes();
            assert_eq!(&body[..], b"secret");
        });
    }

    #[test]
    fn https_refuses_a_certificate_no_trusted_root_signed() {
        let made = made();
        runtime().block_on(async {
            let port = serve_once(&made).await;
            let connector = connector_with(&[]).expect("a connector");
            let refused = send_with(request(port), None, Some(&connector)).await;
            assert!(
                matches!(refused, Err(wasmtime_wasi_http::Error::Tls(_))),
                "{:?}",
                refused.err()
            );
        });
    }

    #[test]
    fn an_https_grant_is_named_with_its_scheme_and_port_443() {
        let secure: hyper::Uri = "https://api.example.com/v1".parse().expect("a URI");
        let plain: hyper::Uri = "http://localhost:7813/".parse().expect("a URI");
        assert_eq!(granted_name(&secure), "https://api.example.com:443");
        assert_eq!(granted_name(&plain), "localhost:7813");
    }
}
