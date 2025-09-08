use crate::openai::config::OpenAiConfig;
use bytes::Buf;
use fast_core::llm::{
    self, ChatDelta, ChatError, ChatOpts, ChatResult, ChatWire, Message, ModelClient, Role,
};
use futures::{Stream, StreamExt};
use reqwest::{header, Client, StatusCode};
use std::result::Result as StdResult;
use std::{pin::Pin, time::Instant};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

#[derive(Clone)]
pub struct OpenAiClient {
    http: Client,
    cfg: OpenAiConfig,
}

impl OpenAiClient {
    fn normalize_gpt5(model: &str) -> (String, Option<&'static str>) {
        // Map Codex-style presets to base model + verbosity for Responses API
        let m = model.trim();
        match m {
            "gpt-5-high" => ("gpt-5".to_string(), Some("high")),
            "gpt-5-medium" => ("gpt-5".to_string(), Some("medium")),
            "gpt-5-low" => ("gpt-5".to_string(), Some("low")),
            "gpt-5-minimal" => ("gpt-5".to_string(), Some("minimal")),
            _ => (m.to_string(), None),
        }
    }
    pub fn new(cfg: OpenAiConfig) -> anyhow::Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {}", cfg.api_key))?,
        );
        let mut builder = Client::builder()
            .default_headers(headers)
            .use_rustls_tls()
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(2)
            .timeout(cfg.timeout);
        if let Some(p) = &cfg.proxy {
            builder = builder.proxy(reqwest::Proxy::all(p)?);
        }
        let http = builder.build()?;
        Ok(Self { http, cfg })
    }

    fn map_messages(&self, msgs: &[Message]) -> Vec<serde_json::Value> {
        msgs.iter()
            .map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => "system",
                };
                serde_json::json!({"role": role, "content": m.content})
            })
            .collect()
    }
}

#[allow(async_fn_in_trait)]
impl ModelClient for OpenAiClient {
    async fn send_chat(&self, msgs: &[Message], opts: &ChatOpts) -> Result<ChatResult, ChatError> {
        let url = format!(
            "{}/chat/completions",
            self.cfg.base_url.trim_end_matches('/')
        );
        let body = serde_json::json!({
            "model": opts.model,
            "messages": self.map_messages(msgs),
            "stream": false,
            "temperature": opts.temperature,
            "top_p": opts.top_p,
            "max_tokens": opts.max_tokens,
        });
        let resp = self
            .http
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(map_reqwest_err)?;
        if !resp.status().is_success() {
            return Err(map_status_err(resp.status(), resp.text().await.ok()));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ChatError::Decode(e.to_string()))?;
        let text = v["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Ok(ChatResult {
            text,
            finish_reason: None,
            prompt_tokens: None,
            completion_tokens: None,
        })
    }

    async fn stream_chat<'a>(
        &'a self,
        msgs: Vec<Message>,
        opts: ChatOpts,
        wire: ChatWire,
    ) -> Result<fast_core::llm::ChatStream<'a>, ChatError> {
        let actual = match wire {
            ChatWire::Chat => ChatWire::Chat,
            ChatWire::Responses => ChatWire::Responses,
            ChatWire::Auto => ChatWire::Responses,
        };
        match actual {
            ChatWire::Chat => self.stream_chat_completions(msgs, opts).await,
            ChatWire::Responses => self.stream_responses_or_fallback(msgs, opts).await,
            ChatWire::Auto => unreachable!(),
        }
    }
}

impl OpenAiClient {
    async fn stream_responses_or_fallback<'a>(
        &'a self,
        msgs: Vec<Message>,
        opts: ChatOpts,
    ) -> Result<fast_core::llm::ChatStream<'a>, ChatError> {
        match self.stream_responses(msgs.clone(), opts.clone()).await {
            Ok(s) => Ok(s),
            Err(ChatError::Protocol(e)) if e.contains("404") => {
                self.stream_chat_completions(msgs, opts).await
            }
            Err(ChatError::Other(e)) if e.contains("404") => {
                self.stream_chat_completions(msgs, opts).await
            }
            Err(e) => Err(e),
        }
    }

    async fn stream_chat_completions<'a>(
        &'a self,
        msgs: Vec<Message>,
        opts: ChatOpts,
    ) -> Result<fast_core::llm::ChatStream<'a>, ChatError> {
        let url = format!(
            "{}/chat/completions",
            self.cfg.base_url.trim_end_matches('/')
        );
        info!(target:"providers::openai","start chat stream model={} url={}", opts.model, url);
        let (model_slug, _verbosity) = Self::normalize_gpt5(&opts.model);
        let body = serde_json::json!({
            "model": model_slug,
            "messages": self.map_messages(&msgs),
            "stream": true,
            "temperature": opts.temperature,
            "top_p": opts.top_p,
            "max_tokens": opts.max_tokens,
        });
        let mut attempt = 0u32;
        let max_attempts = self.cfg.stream_max_retries.max(1);
        let idle = self.cfg.stream_idle_timeout;
        let client = self.http.clone();
        let req = move || client.post(&url).json(&body).send();

        async fn sse_stream(
            send_fut: impl std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
            idle: Duration,
        ) -> Result<impl Stream<Item = Result<ChatDelta, ChatError>>, ChatError> {
            let resp = send_fut.await.map_err(map_reqwest_err)?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.ok();
                error!(target:"providers::openai","chat stream non-200 status={} body={:?}", status, body);
                return Err(map_status_err(status, body));
            }
            let mut stream = resp.bytes_stream();
            let mut buf = bytes::BytesMut::new();
            let mut last = Instant::now();
            let s = async_stream::stream! {
                use futures::StreamExt;
                'outer: loop {
                    tokio::select! {
                        chunk = stream.next() => {
                            match chunk {
                                Some(Ok(b)) => {
                                    buf.extend_from_slice(&b);
                                    last = Instant::now();
                                    loop {
                                        if let Some(pos) = find_event_boundary(&buf) {
                                            let ev = buf.split_to(pos).freeze();
                                            let _ = if buf.starts_with(b"\r\n\r\n") { buf.split_to(4) } else { buf.split_to(2) };
                                            match parse_chat_sse_event(&ev) {
                                                Ok(Some(delta)) => { yield Ok(delta); }
                                                Ok(None) => {}
                                                Err(e) => { yield Err(e); break 'outer; }
                                            }
                                        } else { break; }
                                    }
                                }
                                Some(Err(e)) => { yield Err(map_reqwest_err(e)); break 'outer; }
                                None => { break 'outer; }
                            }
                        }
                        _ = tokio::time::sleep(Duration::from_millis(500)) => {
                            if last.elapsed() > idle { yield Err(ChatError::Timeout("idle".into())); break 'outer; }
                        }
                    }
                }
            };
            Ok(s)
        }

        let merged = async_stream::try_stream! {
            let mut acc_len: usize = 0;
            loop {
                let s = sse_stream(req(), idle).await;
                match s {
                    Ok(st) => {
                        let mut st = Box::pin(st);
                        while let Some(it) = st.as_mut().next().await {
                            let d = it?;
                            if let ChatDelta::Text(ref t) = d { acc_len += t.len(); }
                            yield d;
                        }
                        break;
                    }
                    Err(e) => {
                        attempt += 1;
                        if attempt >= max_attempts { Err(e)? } else {
                            let backoff = Duration::from_millis(300 * attempt as u64);
                            sleep(backoff).await;
                            continue;
                        }
                    }
                }
            }
        };
        Ok(Box::pin(merged))
    }

    async fn stream_responses<'a>(
        &'a self,
        msgs: Vec<Message>,
        opts: ChatOpts,
    ) -> Result<llm::ChatStream<'a>, ChatError> {
        let url = format!("{}/responses", self.cfg.base_url.trim_end_matches('/'));
        info!(target:"providers::openai","start responses stream model={} url={}", opts.model, url);
        let (model_slug, verbosity) = Self::normalize_gpt5(&opts.model);
        // Responses API expects input to be a list of role/content items.
        // Map our chat history into the required shape.
        let input_items: Vec<serde_json::Value> = msgs
            .iter()
            .filter_map(|m| {
                // Skip placeholder assistant messages with empty content
                let is_assistant = matches!(m.role, Role::Assistant);
                if is_assistant && m.content.trim().is_empty() {
                    return None;
                }

                let role = match m.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };
                let content_type = match m.role {
                    Role::Assistant => "output_text", // prior model outputs
                    _ => "input_text",                // user/system instructions
                };
                Some(serde_json::json!({
                    "role": role,
                    "content": [ { "type": content_type, "text": m.content } ]
                }))
            })
            .collect();
        let mut body = serde_json::json!({
            "model": model_slug,
            "input": input_items,
            "stream": true,
        });
        if let Some(v) = verbosity {
            if let Some(map) = body.as_object_mut() {
                map.insert("text".to_string(), serde_json::json!({ "verbosity": v }));
            }
        }
        let client = self.http.clone();
        let send = client.post(url).json(&body).send();
        let idle = self.cfg.stream_idle_timeout;
        let s = async_stream::stream! {
            let resp = send.await.map_err(map_reqwest_err)?;
            if !resp.status().is_success() { let status=resp.status(); let body=resp.text().await.ok(); error!(target:"providers::openai","responses non-200 status={} body={:?}",status,body); yield Err(map_status_err(status, body)); return; }
            let mut stream = resp.bytes_stream();
            let mut buf = bytes::BytesMut::new();
            let mut last = Instant::now();
            'outer: loop {
                tokio::select! {
                    chunk = stream.next() => {
                        match chunk {
                            Some(Ok(b)) => {
                                buf.extend_from_slice(&b);
                                last = Instant::now();
                                loop {
                                    match parse_responses_event(&mut buf) {
                                        Ok(Some((event, data))) => match event.as_str() {
                                            "response.output_text.delta" => yield Ok(ChatDelta::Text(data)),
                                            "response.completed" => { yield Ok(ChatDelta::Finish(None)); break 'outer; },
                                            "response.error" => { yield Err(ChatError::Protocol(data)); break 'outer; },
                                            _ => {}
                                        },
                                        Ok(None) => { break; }
                                        Err(e) => { yield Err(e); break 'outer; }
                                    }
                                }
                            }
                            Some(Err(e)) => { yield Err(map_reqwest_err(e)); break 'outer; }
                            None => { break 'outer; }
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {
                        if last.elapsed() > idle { yield Err(ChatError::Timeout("idle".into())); break 'outer; }
                    }
                }
            }
        };
        Ok(Box::pin(s))
    }
}

fn map_reqwest_err(e: reqwest::Error) -> ChatError {
    if e.is_timeout() {
        ChatError::Timeout(e.to_string())
    } else if e.is_request() || e.is_connect() {
        ChatError::Network(e.to_string())
    } else {
        ChatError::Other(e.to_string())
    }
}

fn map_status_err(status: StatusCode, body: Option<String>) -> ChatError {
    let s = format!("{} {}", status.as_u16(), body.unwrap_or_default());
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ChatError::Auth(s),
        StatusCode::TOO_MANY_REQUESTS => ChatError::RateLimit(s),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => ChatError::Network(s),
        StatusCode::NOT_FOUND => ChatError::Protocol("404".into()),
        _ => ChatError::Other(s),
    }
}

fn find_event_boundary(buf: &bytes::BytesMut) -> Option<usize> {
    if let Some(p) = twoway::find_bytes(&buf, b"\r\n\r\n") {
        return Some(p);
    }
    twoway::find_bytes(&buf, b"\n\n")
}

fn parse_chat_sse_event(ev: &bytes::Bytes) -> Result<Option<ChatDelta>, ChatError> {
    let s = std::str::from_utf8(ev).map_err(|e| ChatError::Decode(e.to_string()))?;
    let mut data_lines = Vec::new();
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim_start());
        }
    }
    if data_lines.is_empty() {
        return Ok(None);
    }
    if data_lines.len() == 1 && data_lines[0] == "[DONE]" {
        return Ok(Some(ChatDelta::Finish(None)));
    }
    let json_text = data_lines.join("\n");
    let v: serde_json::Value =
        serde_json::from_str(&json_text).map_err(|e| ChatError::Decode(e.to_string()))?;
    if let Some(content) = v["choices"][0]["delta"]["content"].as_str() {
        return Ok(Some(ChatDelta::Text(content.to_string())));
    }
    if let Some(role) = v["choices"][0]["delta"]["role"].as_str() {
        let r = match role {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => Role::System,
            _ => Role::Assistant,
        };
        return Ok(Some(ChatDelta::RoleStart(r)));
    }
    if let Some(fr) = v["choices"][0]["finish_reason"].as_str() {
        return Ok(Some(ChatDelta::Finish(Some(fr.to_string()))));
    }
    Ok(None)
}

fn parse_responses_event(buf: &mut bytes::BytesMut) -> Result<Option<(String, String)>, ChatError> {
    // Extract one SSE block (terminated by a blank line), parse event+data.
    let content = match std::str::from_utf8(&buf) {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };
    let (block_end, adv) = if let Some(p) = content.find("\r\n\r\n") {
        (p, 4)
    } else if let Some(p) = content.find("\n\n") {
        (p, 2)
    } else {
        return Ok(None);
    };
    let block = &content[..block_end];

    let mut event: Option<String> = None;
    let mut data_lines: Vec<&str> = Vec::new();
    for line in block.lines() {
        if let Some(v) = line.strip_prefix("event:") {
            event = Some(v.trim().to_string());
        }
        if let Some(v) = line.strip_prefix("data:") {
            data_lines.push(v.trim());
        }
    }
    let data_text = data_lines.join("\n");

    // Fallback: if no explicit event header, infer from JSON `type` field.
    let ev = if let Some(e) = event {
        e
    } else if !data_text.is_empty() {
        match serde_json::from_str::<serde_json::Value>(&data_text) {
            Ok(v) => v["type"].as_str().unwrap_or("").to_string(),
            Err(_) => String::new(),
        }
    } else {
        String::new()
    };

    // Prepare returned `data` based on the event kind for convenience.
    let ret = if ev == "response.output_text.delta" {
        if data_text.trim().starts_with('{') {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_text) {
                v["delta"].as_str().unwrap_or("").to_string()
            } else {
                data_text.clone()
            }
        } else {
            data_text.clone()
        }
    } else if ev == "response.error" {
        if data_text.trim().starts_with('{') {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_text) {
                v["error"]["message"]
                    .as_str()
                    .unwrap_or(&data_text)
                    .to_string()
            } else {
                data_text.clone()
            }
        } else {
            data_text.clone()
        }
    } else {
        data_text.clone()
    };

    // Consume this block from buffer
    buf.advance(block_end + adv);

    if ev.is_empty() {
        return Ok(None);
    }
    Ok(Some((ev, ret)))
}
