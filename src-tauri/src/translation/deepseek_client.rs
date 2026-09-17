//! DeepSeek (OpenAI-compatible) chat-completions client for block translation
//! (design §5, AC-12/AC-13).
//!
//! Retry policy: 401/403 fail immediately (`deepseekAuthFailed`), 429 honours
//! `Retry-After` and then backs off, 5xx / transport / timeout / contract failures are
//! retried up to `translation.maxRetries`, other 4xx fail immediately. The API key is
//! supplied per request and only ever written into the `Authorization` header; it is never
//! logged or formatted (AC-12).

use crate::state::config_models::TranslationSettings;
use crate::stronghold::stronghold_state::ApiKey;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

use super::blocks::{build_system_prompt, build_user_prompt, Block, BlockContext};
use super::validate::{validate_block_response, TranslationItem};

/// Request timeout from design §5.
pub const REQUEST_TIMEOUT_SECONDS: u64 = 120;
/// First backoff step; it doubles per attempt and is capped.
const DEFAULT_BASE_DELAY: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(8);
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepseekError {
  /// 401/403 (or the key is not a valid header value): retrying cannot help.
  AuthFailed,
  /// 429 after the retries were exhausted.
  RateLimited,
  /// 5xx: retryable.
  ServerError { status: u16 },
  /// Other 4xx (bad request, wrong base URL, …): retrying cannot help.
  RequestRejected { status: u16 },
  /// The request exceeded [`REQUEST_TIMEOUT_SECONDS`].
  Timeout,
  /// Transport error (DNS, TLS, connection reset, …).
  Network(String),
  /// The response did not match the promised JSON contract.
  InvalidResponse { reason: String },
}

impl DeepseekError {
  /// Whether another attempt may succeed (contract failures included: the model may
  /// return valid JSON next time).
  pub fn is_retryable(&self) -> bool {
    matches!(
      self,
      Self::RateLimited
        | Self::ServerError { .. }
        | Self::Timeout
        | Self::Network(_)
        | Self::InvalidResponse { .. }
    )
  }
}

impl fmt::Display for DeepseekError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::AuthFailed => write!(f, "deepseek rejected the API key"),
      Self::RateLimited => write!(f, "deepseek rate limited the request"),
      Self::ServerError { status } => write!(f, "deepseek request failed (HTTP {status})"),
      Self::RequestRejected { status } => {
        write!(f, "deepseek rejected the request (HTTP {status})")
      }
      Self::Timeout => write!(f, "deepseek request timed out"),
      Self::Network(reason) => write!(f, "deepseek request failed: {reason}"),
      Self::InvalidResponse { reason } => {
        write!(f, "deepseek returned an invalid response: {reason}")
      }
    }
  }
}

impl std::error::Error for DeepseekError {}

/// Backoff before the next attempt: `Retry-After` wins when present, otherwise the base
/// delay doubles per attempt and never exceeds [`MAX_BACKOFF`].
pub fn retry_delay(attempt: u32, retry_after: Option<Duration>, base: Duration) -> Duration {
  if let Some(after) = retry_after {
    return after.min(MAX_RETRY_AFTER);
  }
  let factor = 1_u32 << attempt.min(10);
  (base * factor).min(MAX_BACKOFF)
}

/// `Retry-After` in seconds (the HTTP-date form is ignored and falls back to backoff).
pub fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
  let value = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
  let seconds = value.trim().parse::<u64>().ok()?;
  Some(Duration::from_secs(seconds).min(MAX_RETRY_AFTER))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
  pub prompt_tokens: u64,
  pub completion_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslatedBlock {
  pub items: Vec<TranslationItem>,
  pub usage: Usage,
}

/// One attempt's failure plus the server's suggested backoff, if any.
type AttemptError = (DeepseekError, Option<Duration>);

pub struct DeepseekClient {
  http: reqwest::Client,
  settings: TranslationSettings,
  base_delay: Duration,
}

impl DeepseekClient {
  pub fn new(settings: &TranslationSettings) -> Result<Self, DeepseekError> {
    let http = reqwest::Client::builder()
      .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
      .build()
      .map_err(|error| DeepseekError::Network(error.to_string()))?;

    Ok(Self {
      http,
      settings: settings.clone(),
      base_delay: DEFAULT_BASE_DELAY,
    })
  }

  /// Test/E2E seam: a zero delay keeps the retry tests fast.
  pub fn with_base_delay(mut self, base_delay: Duration) -> Self {
    self.base_delay = base_delay;
    self
  }

  pub fn endpoint(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
  }

  /// Translates one block, retrying according to [`DeepseekError::is_retryable`] and
  /// `translation.maxRetries`.
  pub async fn translate_block(
    &self,
    block: &Block,
    context: &BlockContext,
    api_key: &ApiKey,
  ) -> Result<TranslatedBlock, DeepseekError> {
    let system_prompt = build_system_prompt(&self.settings);
    let user_prompt = build_user_prompt(block, context);
    let expected_ids = block.ids();
    let request = ChatRequest {
      model: &self.settings.model,
      temperature: self.settings.temperature,
      response_format: ResponseFormat {
        kind: "json_object",
      },
      messages: vec![
        Message {
          role: "system",
          content: &system_prompt,
        },
        Message {
          role: "user",
          content: &user_prompt,
        },
      ],
    };

    let mut attempt = 0_u32;
    loop {
      match self.send_once(&request, &expected_ids, api_key).await {
        Ok(translated) => return Ok(translated),
        Err((error, retry_after)) => {
          if !error.is_retryable() || attempt >= self.settings.max_retries {
            return Err(error);
          }
          tokio::time::sleep(retry_delay(attempt, retry_after, self.base_delay)).await;
          attempt += 1;
        }
      }
    }
  }

  async fn send_once(
    &self,
    request: &ChatRequest<'_>,
    expected_ids: &[usize],
    api_key: &ApiKey,
  ) -> Result<TranslatedBlock, AttemptError> {
    let response = self
      .http
      .post(Self::endpoint(&self.settings.base_url))
      .header(
        reqwest::header::AUTHORIZATION,
        authorization_header(api_key)?,
      )
      .json(request)
      .send()
      .await
      .map_err(|error| (classify_transport_error(&error), None))?;

    let status = response.status();
    if !status.is_success() {
      let retry_after = parse_retry_after(response.headers());
      return Err((status_error(status.as_u16()), retry_after));
    }

    let completion = response
      .json::<ChatCompletion>()
      .await
      .map_err(|error| invalid_response(error.to_string()))?;
    let content = completion
      .choices
      .first()
      .map(|choice| choice.message.content.as_str())
      .ok_or_else(|| invalid_response("response has no choices".to_string()))?;
    let parsed = serde_json::from_str::<TranslationContent>(content)
      .map_err(|error| invalid_response(error.to_string()))?;
    validate_block_response(expected_ids, &parsed.translations)
      .map_err(|error| invalid_response(error.to_string()))?;

    Ok(TranslatedBlock {
      items: parsed.translations,
      usage: completion.usage.unwrap_or_default().into(),
    })
  }
}

fn authorization_header(api_key: &ApiKey) -> Result<reqwest::header::HeaderValue, AttemptError> {
  let mut value = Vec::with_capacity("Bearer ".len() + api_key.expose().len());
  value.extend_from_slice(b"Bearer ");
  value.extend_from_slice(api_key.expose());
  reqwest::header::HeaderValue::from_bytes(&value).map_err(|_| (DeepseekError::AuthFailed, None))
}

fn status_error(status: u16) -> DeepseekError {
  match status {
    401 | 403 => DeepseekError::AuthFailed,
    429 => DeepseekError::RateLimited,
    500..=599 => DeepseekError::ServerError { status },
    _ => DeepseekError::RequestRejected { status },
  }
}

fn classify_transport_error(error: &reqwest::Error) -> DeepseekError {
  if error.is_timeout() {
    DeepseekError::Timeout
  } else {
    DeepseekError::Network(error.to_string())
  }
}

fn invalid_response(reason: String) -> AttemptError {
  (DeepseekError::InvalidResponse { reason }, None)
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
  model: &'a str,
  temperature: f32,
  response_format: ResponseFormat<'a>,
  messages: Vec<Message<'a>>,
}

#[derive(Debug, Serialize)]
struct ResponseFormat<'a> {
  #[serde(rename = "type")]
  kind: &'a str,
}

#[derive(Debug, Serialize)]
struct Message<'a> {
  role: &'a str,
  content: &'a str,
}

#[derive(Debug, Deserialize)]
struct ChatCompletion {
  choices: Vec<Choice>,
  #[serde(default)]
  usage: Option<ApiUsage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
  message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
  content: String,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
struct ApiUsage {
  #[serde(default)]
  prompt_tokens: u64,
  #[serde(default)]
  completion_tokens: u64,
}

impl From<ApiUsage> for Usage {
  fn from(value: ApiUsage) -> Self {
    Self {
      prompt_tokens: value.prompt_tokens,
      completion_tokens: value.completion_tokens,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TranslationContent {
  translations: Vec<TranslationItem>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::translation::blocks::BlockItem;
  use std::io::{Read, Write};
  use std::net::TcpListener;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::{Arc, Mutex as StdMutex};

  fn settings(base_url: &str, max_retries: u32) -> TranslationSettings {
    TranslationSettings {
      base_url: base_url.to_string(),
      max_retries,
      ..TranslationSettings::default()
    }
  }

  fn block() -> Block {
    Block {
      index: 0,
      paragraph_range: 0..2,
      items: vec![
        BlockItem {
          id: 0,
          text: "Hello.".to_string(),
        },
        BlockItem {
          id: 1,
          text: "World.".to_string(),
        },
      ],
    }
  }

  fn translation_content(translations: &serde_json::Value) -> String {
    serde_json::json!({
      "choices": [{"message": {"content": translations.to_string()}}],
      "usage": {"prompt_tokens": 12, "completion_tokens": 7},
    })
    .to_string()
  }

  fn valid_body() -> String {
    translation_content(&serde_json::json!({
      "translations": [
        {"id": 0, "zh": "你好。"},
        {"id": 1, "zh": "世界。"},
      ],
    }))
  }

  #[test]
  fn endpoint_joins_the_base_url_without_doubling_slashes() {
    assert_eq!(
      DeepseekClient::endpoint("https://api.deepseek.com"),
      "https://api.deepseek.com/chat/completions"
    );
    assert_eq!(
      DeepseekClient::endpoint("https://api.deepseek.com/"),
      "https://api.deepseek.com/chat/completions"
    );
  }

  #[test]
  fn status_mapping_and_retryability_match_the_policy() {
    assert_eq!(status_error(401), DeepseekError::AuthFailed);
    assert_eq!(status_error(403), DeepseekError::AuthFailed);
    assert_eq!(status_error(429), DeepseekError::RateLimited);
    assert_eq!(
      status_error(400),
      DeepseekError::RequestRejected { status: 400 }
    );
    assert_eq!(
      status_error(503),
      DeepseekError::ServerError { status: 503 }
    );

    assert!(status_error(429).is_retryable());
    assert!(status_error(503).is_retryable());
    assert!(!status_error(400).is_retryable());
    assert!(!status_error(401).is_retryable());
    assert!(!DeepseekError::AuthFailed.is_retryable());
  }

  #[test]
  fn backoff_honours_retry_after_then_doubles_and_caps() {
    let base = Duration::from_millis(100);

    assert_eq!(
      retry_delay(0, Some(Duration::from_secs(3)), base),
      Duration::from_secs(3)
    );
    assert_eq!(
      retry_delay(0, Some(Duration::from_secs(600)), base),
      Duration::from_secs(60)
    );
    assert_eq!(retry_delay(0, None, base), Duration::from_millis(100));
    assert_eq!(retry_delay(1, None, base), Duration::from_millis(200));
    assert_eq!(retry_delay(2, None, base), Duration::from_millis(400));
    assert_eq!(retry_delay(9, None, base), Duration::from_secs(8));
  }

  #[test]
  fn retry_after_parsing_rejects_junk_and_caps_the_value() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::RETRY_AFTER, "2".parse().expect("header"));
    assert_eq!(parse_retry_after(&headers), Some(Duration::from_secs(2)));

    headers.insert(
      reqwest::header::RETRY_AFTER,
      "Wed, 21 Oct 2015 07:28:00 GMT".parse().expect("header"),
    );
    assert_eq!(parse_retry_after(&headers), None);

    assert_eq!(parse_retry_after(&reqwest::header::HeaderMap::new()), None);
  }

  /// Minimal one-shot HTTP/1.1 mock: each connection consumes the next canned response
  /// (the last one repeats) and records the raw request for assertions.
  struct MockServer {
    base_url: String,
    hits: Arc<AtomicUsize>,
    requests: Arc<StdMutex<Vec<String>>>,
  }

  struct MockResponse {
    status: u16,
    retry_after: Option<&'static str>,
    body: String,
  }

  impl MockServer {
    fn start(responses: Vec<MockResponse>) -> Self {
      let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
      let port = listener.local_addr().expect("mock address").port();
      let hits = Arc::new(AtomicUsize::new(0));
      let requests = Arc::new(StdMutex::new(Vec::new()));
      let thread_hits = hits.clone();
      let thread_requests = requests.clone();

      std::thread::spawn(move || {
        for stream in listener.incoming() {
          let Ok(mut stream) = stream else {
            break;
          };
          let index = thread_hits.fetch_add(1, Ordering::SeqCst);
          let Some(response) = responses.get(index).or_else(|| responses.last()) else {
            break;
          };

          let request = read_http_request(&mut stream);
          thread_requests.lock().expect("mock lock").push(request);

          let reason = if (200..300).contains(&response.status) {
            "OK"
          } else {
            "ERR"
          };
          let mut head = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            response.status,
            reason,
            response.body.len()
          );
          if let Some(after) = response.retry_after {
            head.push_str(&format!("Retry-After: {after}\r\n"));
          }
          head.push_str("\r\n");

          let _ = stream.write_all(head.as_bytes());
          let _ = stream.write_all(response.body.as_bytes());
          let _ = stream.flush();
        }
      });

      Self {
        base_url: format!("http://127.0.0.1:{port}"),
        hits,
        requests,
      }
    }

    fn hits(&self) -> usize {
      self.hits.load(Ordering::SeqCst)
    }

    fn requests(&self) -> Vec<String> {
      self.requests.lock().expect("mock lock").clone()
    }
  }

  fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
      match stream.read(&mut chunk) {
        Ok(0) | Err(_) => break,
        Ok(read) => raw.extend_from_slice(&chunk[..read]),
      }

      let text = String::from_utf8_lossy(&raw).to_string();
      if let Some(header_end) = text.find("\r\n\r\n") {
        let content_length = text[..header_end]
          .lines()
          .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
              value.trim().parse::<usize>().ok()
            } else {
              None
            }
          })
          .unwrap_or(0);
        if raw.len() >= header_end + 4 + content_length {
          break;
        }
      }
    }
    String::from_utf8_lossy(&raw).to_string()
  }

  fn json_response(status: u16, body: String) -> MockResponse {
    MockResponse {
      status,
      retry_after: None,
      body,
    }
  }

  async fn translate(settings: &TranslationSettings) {
    let client = DeepseekClient::new(settings)
      .expect("client")
      .with_base_delay(Duration::ZERO);
    let key = ApiKey::new(b"sk-test-DO-NOT-LEAK".to_vec());
    let result = client
      .translate_block(&block(), &BlockContext::default(), &key)
      .await;
    assert!(result.is_ok(), "expected success, got {result:?}");
  }

  #[tokio::test]
  async fn success_returns_items_and_usage() {
    let server = MockServer::start(vec![json_response(200, valid_body())]);
    let client = DeepseekClient::new(&settings(&server.base_url, 2))
      .expect("client")
      .with_base_delay(Duration::ZERO);
    let key = ApiKey::new(b"sk-test-DO-NOT-LEAK".to_vec());

    let translated = client
      .translate_block(&block(), &BlockContext::default(), &key)
      .await
      .expect("translated");

    assert_eq!(translated.items.len(), 2);
    assert_eq!(translated.items[0].zh, "你好。");
    assert_eq!(
      translated.usage,
      Usage {
        prompt_tokens: 12,
        completion_tokens: 7
      }
    );
    assert_eq!(server.hits(), 1);
  }

  #[tokio::test]
  async fn auth_failure_does_not_retry() {
    let server = MockServer::start(vec![json_response(401, "{}".into())]);
    let client = DeepseekClient::new(&settings(&server.base_url, 2))
      .expect("client")
      .with_base_delay(Duration::ZERO);
    let key = ApiKey::new(b"sk-test-DO-NOT-LEAK".to_vec());

    let error = client
      .translate_block(&block(), &BlockContext::default(), &key)
      .await
      .expect_err("auth failure");

    assert_eq!(error, DeepseekError::AuthFailed);
    assert_eq!(server.hits(), 1);
  }

  #[tokio::test]
  async fn rate_limit_retries_after_the_retry_after_delay() {
    let server = MockServer::start(vec![
      MockResponse {
        status: 429,
        retry_after: Some("0"),
        body: "{}".into(),
      },
      json_response(200, valid_body()),
    ]);

    translate(&settings(&server.base_url, 2)).await;
    assert_eq!(server.hits(), 2);
  }

  #[tokio::test]
  async fn server_errors_stop_after_max_retries() {
    let server = MockServer::start(vec![
      json_response(500, "{}".into()),
      json_response(500, "{}".into()),
      json_response(500, "{}".into()),
    ]);
    let client = DeepseekClient::new(&settings(&server.base_url, 2))
      .expect("client")
      .with_base_delay(Duration::ZERO);
    let key = ApiKey::new(b"sk-test-DO-NOT-LEAK".to_vec());

    let error = client
      .translate_block(&block(), &BlockContext::default(), &key)
      .await
      .expect_err("server error");

    assert_eq!(error, DeepseekError::ServerError { status: 500 });
    assert_eq!(server.hits(), 3, "one attempt plus maxRetries retries");
  }

  #[tokio::test]
  async fn contract_failure_is_retried_then_succeeds() {
    let incomplete = translation_content(&serde_json::json!({
      "translations": [{"id": 0, "zh": "你好。"}],
    }));
    let server = MockServer::start(vec![
      json_response(200, incomplete),
      json_response(200, valid_body()),
    ]);

    translate(&settings(&server.base_url, 2)).await;
    assert_eq!(server.hits(), 2);
  }

  #[tokio::test]
  async fn contract_failure_exhausts_retries() {
    let incomplete = translation_content(&serde_json::json!({
      "translations": [{"id": 0, "zh": "你好。"}],
    }));
    let server = MockServer::start(vec![
      json_response(200, incomplete.clone()),
      json_response(200, incomplete.clone()),
      json_response(200, incomplete),
    ]);
    let client = DeepseekClient::new(&settings(&server.base_url, 2))
      .expect("client")
      .with_base_delay(Duration::ZERO);
    let key = ApiKey::new(b"sk-test-DO-NOT-LEAK".to_vec());

    let error = client
      .translate_block(&block(), &BlockContext::default(), &key)
      .await
      .expect_err("contract failure");

    assert!(matches!(error, DeepseekError::InvalidResponse { .. }));
    assert_eq!(server.hits(), 3);
  }

  #[tokio::test]
  async fn request_uses_json_mode_and_the_key_stays_in_the_header() {
    let server = MockServer::start(vec![json_response(200, valid_body())]);

    translate(&settings(&server.base_url, 2)).await;

    let request = server
      .requests()
      .first()
      .cloned()
      .expect("request captured");
    assert!(request.contains("POST /chat/completions"));
    assert!(request.contains("authorization: Bearer sk-test-DO-NOT-LEAK"));
    assert!(request.contains("\"type\":\"json_object\""));
    assert!(request.contains("\"model\":\"deepseek-chat\""));
    assert!(request.contains("\"temperature\":0.3"));
    assert!(request.contains("\"role\":\"system\""));
    assert!(request.contains("\"role\":\"user\""));

    let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
    assert!(
      !body.contains("sk-test-DO-NOT-LEAK"),
      "key leaked into the body"
    );
  }
}
