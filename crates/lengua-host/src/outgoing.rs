use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use http_body_util::BodyExt;
use hyper::body::{Body, Bytes, Frame};
use tokio_rustls::TlsConnector;
use wasmtime_wasi_http::io::TokioIo;
use wasmtime_wasi_http::{Error as HttpError, RequestOptions, WasiBody, WasiHttpHooks};

use crate::model::{self, Budget, Model};
use crate::tls;

pub(crate) const AI_HOST: &str = "ai.lengua";
const CONNECT: Duration = Duration::from_secs(30);

pub(crate) type Sent = Box<dyn Future<Output = Result<(), HttpError>> + Send>;
pub(crate) type Reply = Result<(hyper::Response<WasiBody>, Sent), HttpError>;
type Pending = Box<dyn Future<Output = Reply> + Send>;

pub(crate) struct Hooks {
    pub calls: Vec<String>,
    pub budget: Option<Arc<Mutex<Budget>>>,
    pub model: Arc<Model>,
}

pub(crate) struct Pieces(pub VecDeque<Bytes>);

impl Body for Pieces {
    type Data = Bytes;
    type Error = HttpError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, HttpError>>> {
        Poll::Ready(self.0.pop_front().map(|bytes| Ok(Frame::data(bytes))))
    }
}

pub(crate) fn whole(bytes: Vec<u8>) -> WasiBody {
    Pieces(VecDeque::from([Bytes::from(bytes)])).boxed_unsync()
}

pub(crate) fn done() -> Sent {
    Box::new(std::future::ready(Ok(())))
}

pub(crate) fn reply(status: u16, kind: &str, tokens: u64, body: WasiBody) -> Reply {
    let response = hyper::Response::builder()
        .status(status)
        .header("content-type", kind)
        .header("x-lengua-tokens", tokens.to_string())
        .body(body)
        .map_err(|e| HttpError::InternalError(Some(e.to_string())))?;
    Ok((response, done()))
}

pub(crate) fn secure(uri: &hyper::Uri) -> bool {
    uri.scheme() == Some(&http::uri::Scheme::HTTPS)
}

pub(crate) fn authority_of(uri: &hyper::Uri) -> Option<String> {
    let authority = uri.authority()?;
    let port = if secure(uri) { 443 } else { 80 };
    Some(if authority.port().is_some() {
        authority.to_string()
    } else {
        format!("{}:{port}", authority.host())
    })
}

pub(crate) fn granted_name(uri: &hyper::Uri) -> String {
    let address = authority_of(uri).unwrap_or_default();
    if secure(uri) {
        format!("https://{address}")
    } else {
        address
    }
}

async fn over<S>(stream: S, mut request: hyper::Request<WasiBody>) -> Reply
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake::<_, WasiBody>(TokioIo::new(stream)).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let path = request
        .uri()
        .path_and_query()
        .map_or("/", |p| p.as_str())
        .to_string();
    *request.uri_mut() = hyper::Uri::builder()
        .path_and_query(path)
        .build()
        .map_err(|_| HttpError::HttpRequestUriInvalid)?;
    let response = sender.send_request(request).await?;
    Ok((
        response.map(|b| b.map_err(HttpError::from).boxed_unsync()),
        done(),
    ))
}

pub(crate) async fn send(
    request: hyper::Request<WasiBody>,
    options: Option<RequestOptions>,
) -> Reply {
    let tls = if secure(request.uri()) {
        Some(tls::connector().map_err(|m| HttpError::InternalError(Some(m)))?)
    } else {
        None
    };
    send_with(request, options, tls).await
}

pub(crate) async fn send_with(
    request: hyper::Request<WasiBody>,
    options: Option<RequestOptions>,
    tls: Option<TlsConnector>,
) -> Reply {
    let address = authority_of(request.uri()).ok_or(HttpError::HttpRequestUriInvalid)?;
    let wait = options.and_then(|o| o.connect_timeout).unwrap_or(CONNECT);
    let stream = tokio::time::timeout(wait, tokio::net::TcpStream::connect(&address))
        .await
        .map_err(|_| HttpError::ConnectionTimeout)?
        .map_err(HttpError::Connect)?;
    match tls {
        Some(connector) => {
            let host = request.uri().host().unwrap_or_default().to_string();
            let wrapped = tls::wrap(&connector, &host, stream)
                .await
                .map_err(HttpError::Tls)?;
            over(wrapped, request).await
        }
        None => over(stream, request).await,
    }
}

fn refuse(message: String) -> Pending {
    Box::new(async move {
        reply(
            403,
            "text/plain; charset=utf-8",
            0,
            whole(message.into_bytes()),
        )
    })
}

impl WasiHttpHooks for Hooks {
    fn is_supported_scheme(&mut self, scheme: &http::uri::Scheme) -> bool {
        *scheme == http::uri::Scheme::HTTP || *scheme == http::uri::Scheme::HTTPS
    }

    fn default_scheme(&mut self) -> Option<http::uri::Scheme> {
        Some(http::uri::Scheme::HTTP)
    }

    fn send_request(
        &mut self,
        request: hyper::Request<WasiBody>,
        options: Option<RequestOptions>,
        _fut: Sent,
    ) -> Pending {
        let host = request.uri().host().unwrap_or_default().to_string();
        if host == AI_HOST {
            return match &self.budget {
                Some(budget) => Box::new(model::ask(request, budget.clone(), self.model.clone())),
                None => refuse(
                    "the idea does not grant `- Asks: an AI model, for ...`, so the program can't ask one"
                        .to_string(),
                ),
            };
        }
        if self.calls.contains(&granted_name(request.uri())) {
            return Box::new(send(request, options));
        }
        Box::new(std::future::ready(Err(HttpError::HttpRequestDenied)))
    }
}
