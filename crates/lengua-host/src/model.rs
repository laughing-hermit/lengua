use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};

use http_body_util::BodyExt;
use hyper::body::{Body, Bytes, Frame};
use wasmtime_wasi_http::{Error as HttpError, WasiBody};

use crate::json::{self, Value};
use crate::outgoing::{Pieces, Reply, reply, send, whole};

const PROMPT_LIMIT: usize = 1 << 20;
const TEXT: &str = "text/plain; charset=utf-8";
const JSON: &str = "application/json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Budget {
    pub calls: u64,
    pub tokens: u64,
}

impl Budget {
    fn spend(&mut self, tokens: u64) -> bool {
        let fits = tokens <= self.tokens;
        self.tokens = self.tokens.saturating_sub(tokens);
        fits
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Endpoint {
    pub scheme: &'static str,
    pub authority: String,
    pub base: String,
    pub model: String,
    pub embed: String,
    pub key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Model {
    Given(Option<String>),
    Live(Result<Endpoint, String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ask {
    Whole,
    Stream,
    Embed,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

pub(crate) fn endpoint() -> Result<Endpoint, String> {
    let url = env("LENGUA_MODEL_URL").unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
    endpoint_of(
        &url,
        env("LENGUA_MODEL"),
        env("LENGUA_EMBED_MODEL"),
        env("LENGUA_MODEL_KEY"),
    )
}

pub(crate) fn endpoint_of(
    url: &str,
    model: Option<String>,
    embed: Option<String>,
    key: Option<String>,
) -> Result<Endpoint, String> {
    let (scheme, rest) = if let Some(rest) = url.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        ("http", rest)
    } else {
        return Err(format!(
            "`LENGUA_MODEL_URL` is `{url}`; lengua reaches a model server at `http://host:port` or `https://host`"
        ));
    };
    let (authority, base) = rest.split_once('/').unwrap_or((rest, ""));
    let model = model.ok_or_else(|| {
        "no model is named: set `LENGUA_MODEL` to the model's name, for example `llama3.2`"
            .to_string()
    })?;
    Ok(Endpoint {
        scheme,
        authority: authority.to_string(),
        base: if base.is_empty() {
            String::new()
        } else {
            format!("/{}", base.trim_end_matches('/'))
        },
        embed: embed.unwrap_or_else(|| model.clone()),
        model,
        key,
    })
}

pub(crate) fn estimate(bytes: usize) -> u64 {
    u64::try_from(bytes.div_ceil(4)).unwrap_or(u64::MAX)
}

fn text_reply(status: u16, message: &str) -> Reply {
    reply(status, TEXT, 0, whole(message.as_bytes().to_vec()))
}

fn take(budget: &Mutex<Budget>, prompt_tokens: u64) -> Result<(), String> {
    let mut left = budget.lock().unwrap_or_else(PoisonError::into_inner);
    if left.calls == 0 {
        return Err("the program asked the model more times than its idea grants".to_string());
    }
    left.calls -= 1;
    if !left.spend(prompt_tokens) {
        return Err("the prompt has more tokens than the idea grants".to_string());
    }
    Ok(())
}

fn spend(budget: &Mutex<Budget>, tokens: u64) -> bool {
    budget
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .spend(tokens)
}

fn kind_of(path: &str) -> Option<Ask> {
    match path {
        "/ask" => Some(Ask::Whole),
        "/stream" => Some(Ask::Stream),
        "/embed" => Some(Ask::Embed),
        _ => None,
    }
}

pub(crate) async fn ask(
    request: hyper::Request<WasiBody>,
    budget: Arc<Mutex<Budget>>,
    model: Arc<Model>,
) -> Reply {
    let Some(kind) = kind_of(request.uri().path()) else {
        return text_reply(
            404,
            "the model answers `POST /ask`, `POST /stream` and `POST /embed`",
        );
    };
    if request.method() != hyper::Method::POST {
        return text_reply(405, "the model answers only `POST`");
    }
    let structured = request
        .headers()
        .get("x-lengua-reply")
        .is_some_and(|v| v.as_bytes() == b"json");
    let bytes = request.into_body().collect().await?.to_bytes();
    if bytes.len() > PROMPT_LIMIT {
        return text_reply(413, "the prompt is larger than 1 MiB");
    }
    let prompt = String::from_utf8_lossy(&bytes).to_string();
    if let Err(message) = take(&budget, estimate(prompt.len())) {
        return text_reply(429, &message);
    }
    match model.as_ref() {
        Model::Given(None) => text_reply(
            503,
            "no model answers in this run: the example gives no `- Model answers:`",
        ),
        Model::Given(Some(answer)) => given(answer, kind, structured, &budget),
        Model::Live(Err(message)) => text_reply(503, message),
        Model::Live(Ok(endpoint)) => live(endpoint, &prompt, kind, structured, budget).await,
    }
}

fn given(answer: &str, kind: Ask, structured: bool, budget: &Mutex<Budget>) -> Reply {
    let tokens = estimate(answer.len());
    if !spend(budget, tokens) {
        return text_reply(429, "the answer has more tokens than the idea grants");
    }
    let json_wanted = structured || kind == Ask::Embed;
    if json_wanted && json::parse(answer).is_none() {
        return text_reply(502, "the model's answer is not JSON");
    }
    let content = if json_wanted { JSON } else { TEXT };
    if kind == Ask::Stream {
        let pieces: VecDeque<Bytes> = answer
            .split_inclusive(' ')
            .map(|piece| Bytes::from(piece.to_string()))
            .collect();
        return reply(200, content, tokens, Pieces(pieces).boxed_unsync());
    }
    reply(200, content, tokens, whole(answer.as_bytes().to_vec()))
}

fn request_body(
    endpoint: &Endpoint,
    prompt: &str,
    kind: Ask,
    structured: bool,
    left: u64,
) -> String {
    if kind == Ask::Embed {
        return format!(
            "{{\"model\":{},\"input\":{}}}",
            json::quote(&endpoint.embed),
            json::quote(prompt)
        );
    }
    let mut body = format!(
        "{{\"model\":{},\"messages\":[{{\"role\":\"user\",\"content\":{}}}],\"max_tokens\":{left}",
        json::quote(&endpoint.model),
        json::quote(prompt)
    );
    if structured {
        body.push_str(",\"response_format\":{\"type\":\"json_object\"}");
    }
    if kind == Ask::Stream {
        body.push_str(",\"stream\":true");
    }
    body.push('}');
    body
}

async fn live(
    endpoint: &Endpoint,
    prompt: &str,
    kind: Ask,
    structured: bool,
    budget: Arc<Mutex<Budget>>,
) -> Reply {
    let left = budget.lock().unwrap_or_else(PoisonError::into_inner).tokens;
    let path = if kind == Ask::Embed {
        "/v1/embeddings"
    } else {
        "/v1/chat/completions"
    };
    let mut builder = hyper::Request::builder()
        .method("POST")
        .uri(format!(
            "{}://{}{}{path}",
            endpoint.scheme, endpoint.authority, endpoint.base
        ))
        .header("host", &endpoint.authority)
        .header("content-type", JSON);
    if let Some(key) = &endpoint.key {
        builder = builder.header("authorization", format!("Bearer {key}"));
    }
    let body = request_body(endpoint, prompt, kind, structured, left);
    let request = builder
        .body(whole(body.into_bytes()))
        .map_err(|e| HttpError::InternalError(Some(e.to_string())))?;
    let (response, _) = match send(request, None).await {
        Ok(found) => found,
        Err(error) => {
            return text_reply(
                503,
                &format!("lengua could not reach the model server: {error}"),
            );
        }
    };
    let status = response.status().as_u16();
    if kind == Ask::Stream && (200..300).contains(&status) {
        let body = Events {
            inner: response.into_body(),
            buffer: Vec::new(),
            budget,
            ended: false,
        };
        return reply(200, TEXT, 0, body.boxed_unsync());
    }
    let bytes = response.into_body().collect().await?.to_bytes();
    let text = String::from_utf8_lossy(&bytes).to_string();
    if !(200..300).contains(&status) {
        return text_reply(502, &format!("the model server answered {status}: {text}"));
    }
    finish(&text, kind, structured, &budget)
}

fn finish(text: &str, kind: Ask, structured: bool, budget: &Mutex<Budget>) -> Reply {
    let Some(value) = json::parse(text) else {
        return text_reply(502, "the model server's answer is not JSON");
    };
    let used = value
        .get("usage")
        .and_then(|u| u.get("total_tokens"))
        .and_then(Value::number)
        .unwrap_or_else(|| estimate(text.len()));
    if !spend(budget, used) {
        return text_reply(429, "the answer has more tokens than the idea grants");
    }
    if kind == Ask::Embed {
        let Some(list) = value
            .get("data")
            .and_then(|d| d.at(0))
            .and_then(|d| d.get("embedding"))
        else {
            return text_reply(502, "the model server's answer has no embedding");
        };
        let mut out = String::new();
        list.write(&mut out);
        return reply(200, JSON, used, whole(out.into_bytes()));
    }
    let content = value
        .get("choices")
        .and_then(|c| c.at(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(Value::text);
    let Some(content) = content else {
        return text_reply(502, "the model server's answer has no message");
    };
    if structured && json::parse(content).is_none() {
        return text_reply(502, "the model's answer is not JSON");
    }
    let kind = if structured { JSON } else { TEXT };
    reply(200, kind, used, whole(content.as_bytes().to_vec()))
}

struct Events {
    inner: WasiBody,
    buffer: Vec<u8>,
    budget: Arc<Mutex<Budget>>,
    ended: bool,
}

pub(crate) fn event_text(line: &str) -> Option<String> {
    let data = line.strip_prefix("data:")?.trim();
    if data == "[DONE]" {
        return None;
    }
    let value = json::parse(data)?;
    value
        .get("choices")?
        .at(0)?
        .get("delta")?
        .get("content")?
        .text()
        .map(str::to_string)
}

impl Events {
    fn text(&mut self) -> String {
        let mut out = String::new();
        while let Some(end) = self.buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=end).collect();
            out.push_str(
                &event_text(String::from_utf8_lossy(&line).trim_end()).unwrap_or_default(),
            );
        }
        if self.ended && !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            out.push_str(
                &event_text(String::from_utf8_lossy(&line).trim_end()).unwrap_or_default(),
            );
        }
        out
    }
}

impl Body for Events {
    type Data = Bytes;
    type Error = HttpError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, HttpError>>> {
        let this = self.get_mut();
        loop {
            let text = this.text();
            if !text.is_empty() {
                if !spend(&this.budget, estimate(text.len())) {
                    this.ended = true;
                    this.buffer.clear();
                }
                return Poll::Ready(Some(Ok(Frame::data(Bytes::from(text)))));
            }
            if this.ended {
                return Poll::Ready(None);
            }
            match Pin::new(&mut this.inner).poll_frame(cx) {
                Poll::Ready(None) => this.ended = true,
                Poll::Ready(Some(Ok(frame))) => {
                    if let Ok(data) = frame.into_data() {
                        this.buffer.extend_from_slice(&data);
                    }
                }
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Some(Err(error))),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Ask, Budget, Endpoint, event_text, request_body};

    #[test]
    fn a_budget_counts_down_and_refuses_past_its_end() {
        let mut budget = Budget {
            calls: 1,
            tokens: 10,
        };
        assert!(budget.spend(4));
        assert!(!budget.spend(7));
        assert_eq!(budget.tokens, 0);
    }

    #[test]
    fn the_request_to_a_model_server_is_open_ai_s_shape() {
        let endpoint = Endpoint {
            scheme: "http",
            authority: "127.0.0.1:11434".to_string(),
            base: String::new(),
            model: "m".to_string(),
            embed: "e".to_string(),
            key: None,
        };
        assert_eq!(
            request_body(&endpoint, "Hi \"you\"", Ask::Stream, true, 50),
            "{\"model\":\"m\",\"messages\":[{\"role\":\"user\",\"content\":\"Hi \\\"you\\\"\"}],\"max_tokens\":50,\"response_format\":{\"type\":\"json_object\"},\"stream\":true}"
        );
        assert_eq!(
            request_body(&endpoint, "a", Ask::Embed, false, 5),
            "{\"model\":\"e\",\"input\":\"a\"}"
        );
        assert_eq!(
            event_text("data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}"),
            Some("Hel".to_string())
        );
        assert_eq!(event_text("data: [DONE]"), None);
        assert_eq!(event_text(": keep"), None);
    }
}
