//! Requirement 10.3 — the one-time connectivity/format self-check required before a
//! `Local_Model`/`BYOK_Frontier` source can be activated: a trivial structured-JSON
//! round-trip, using the exact request shapes `packages/agents-core`'s `llmCaller.ts`
//! uses for real extraction, so a passing check actually validates what a real
//! extraction call will later hit.
//!
//! Deliberately minimal — no retry/backoff, no credit-exhaustion fallback, no lenient
//! markdown-fence-stripping JSON extraction. `llmCaller.ts`'s leniency exists to salvage
//! a real extraction call across model quirks; a health probe should be strict — if a
//! trivial prompt can't come back as clean JSON, that's a real failure to surface, not
//! something to paper over. This is intentionally a smaller, different thing than
//! reimplementing the full provider-calling logic a second time in Rust.

use crate::extraction::settings::ByokProvider;
use serde_json::json;
use std::time::Duration;

const CHECK_PROMPT: &str = "Reply with the JSON object {\"ok\": true} and nothing else.";
const CHECK_SYSTEM_PROMPT: &str = "Respond only with JSON.";
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);

const OPENAI_BASE: &str = "https://api.openai.com/v1";
const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";
const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";

/// Local_Model check — POSTs the same OpenAI-compatible `/v1/chat/completions` shape
/// llmCaller.ts uses for Ollama/OpenAI/OpenRouter.
pub async fn run_local_model_check(endpoint: &str, model: &str) -> Result<(), String> {
    openai_compatible_check(endpoint, "Bearer ollama".to_string(), model).await
}

/// BYOK_Frontier check — one of two request shapes depending on provider, matching
/// llmCaller.ts's OpenAI/OpenRouter (OpenAI-compatible) and Anthropic (native Messages
/// API) branches exactly.
pub async fn run_byok_check(provider: ByokProvider, api_key: &str, model: &str) -> Result<(), String> {
    match provider {
        ByokProvider::OpenAI => openai_compatible_check(OPENAI_BASE, format!("Bearer {api_key}"), model).await,
        ByokProvider::OpenRouter => openai_compatible_check(OPENROUTER_BASE, format!("Bearer {api_key}"), model).await,
        ByokProvider::Anthropic => anthropic_check(ANTHROPIC_URL, api_key, model).await,
    }
}

/// Shared by Local_Model, OpenAI, and OpenRouter — all three speak the same
/// OpenAI-compatible `/v1/chat/completions` shape, differing only in base URL and
/// auth header. `base` is injectable so tests can point this at a local stub instead
/// of a hardcoded provider URL.
async fn openai_compatible_check(base: &str, auth_header: String, model: &str) -> Result<(), String> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));

    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": CHECK_SYSTEM_PROMPT },
            { "role": "user", "content": CHECK_PROMPT },
        ],
        "max_tokens": 32,
        "temperature": 0.1,
        "response_format": { "type": "json_object" },
    });

    let resp = client
        .post(&url)
        .header("content-type", "application/json")
        .header("Authorization", auth_header)
        .timeout(CHECK_TIMEOUT)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Could not reach {url}: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Endpoint returned HTTP {status}: {text}"));
    }

    let parsed: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Response was not valid JSON: {e}"))?;
    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "Response missing choices[0].message.content".to_string())?;
    serde_json::from_str::<serde_json::Value>(content)
        .map_err(|e| format!("Model did not return parseable JSON content: {e}"))?;

    Ok(())
}

/// Anthropic's native Messages API shape. `url` is injectable so tests can point this
/// at a local stub instead of the hardcoded `api.anthropic.com`.
async fn anthropic_check(url: &str, api_key: &str, model: &str) -> Result<(), String> {
    let client = reqwest::Client::new();

    let body = json!({
        "model": model,
        "system": CHECK_SYSTEM_PROMPT,
        "max_tokens": 32,
        "messages": [{ "role": "user", "content": CHECK_PROMPT }],
    });
    let resp = client
        .post(url)
        .header("content-type", "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .timeout(CHECK_TIMEOUT)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Anthropic returned HTTP {status}: {text}"));
    }

    let parsed: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Response was not valid JSON: {e}"))?;
    let text = parsed["content"][0]["text"]
        .as_str()
        .ok_or_else(|| "Response missing content[0].text".to_string())?;
    serde_json::from_str::<serde_json::Value>(text)
        .map_err(|e| format!("Model did not return parseable JSON: {e}"))?;

    Ok(())
}

/// Requirement 10.2 — a Local_Model endpoint defaults to loopback; anything else
/// requires explicit opt-in. Factored out as a pure function so it's unit-testable
/// without a Tauri command context.
pub fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_server(status: u16, body: &'static str) -> String {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind ephemeral port");
        let addr = server.server_addr().to_ip().expect("ip address");
        std::thread::spawn(move || {
            if let Ok(request) = server.recv() {
                let header =
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
                let response = tiny_http::Response::from_string(body)
                    .with_status_code(status)
                    .with_header(header);
                let _ = request.respond(response);
            }
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn local_model_check_succeeds_on_valid_json_content() {
        let base = stub_server(
            200,
            r#"{"choices":[{"message":{"content":"{\"ok\": true}"}}]}"#,
        );
        assert!(run_local_model_check(&base, "llama3.2:3b").await.is_ok());
    }

    #[tokio::test]
    async fn local_model_check_fails_on_http_error_status() {
        let base = stub_server(500, r#"{"error":"boom"}"#);
        let err = run_local_model_check(&base, "llama3.2:3b").await.unwrap_err();
        assert!(err.contains("HTTP 500"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn local_model_check_fails_on_malformed_top_level_json() {
        let base = stub_server(200, "not json at all");
        let err = run_local_model_check(&base, "llama3.2:3b").await.unwrap_err();
        assert!(err.contains("not valid JSON"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn local_model_check_fails_when_content_is_not_itself_json() {
        let base = stub_server(
            200,
            r#"{"choices":[{"message":{"content":"sure, here you go!"}}]}"#,
        );
        let err = run_local_model_check(&base, "llama3.2:3b").await.unwrap_err();
        assert!(err.contains("did not return parseable JSON"), "unexpected error: {err}");
    }

    // OpenAI/OpenRouter share `openai_compatible_check` with Local_Model — exercised
    // directly here (with an injected stub base URL) rather than through
    // `run_byok_check`, since that function's public API hardcodes the real provider
    // URLs by design (they should never be swappable in production).

    #[tokio::test]
    async fn openai_compatible_check_succeeds_on_valid_json_content() {
        let base = stub_server(
            200,
            r#"{"choices":[{"message":{"content":"{\"ok\": true}"}}]}"#,
        );
        assert!(openai_compatible_check(&base, "Bearer sk-test".to_string(), "gpt-4o-mini").await.is_ok());
    }

    #[tokio::test]
    async fn openai_compatible_check_fails_on_unauthorized() {
        let base = stub_server(401, r#"{"error":{"message":"Incorrect API key provided"}}"#);
        let err = openai_compatible_check(&base, "Bearer sk-bad".to_string(), "gpt-4o-mini").await.unwrap_err();
        assert!(err.contains("HTTP 401"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn anthropic_check_succeeds_on_valid_json_content() {
        let url = format!("{}/messages", stub_server(200, r#"{"content":[{"type":"text","text":"{\"ok\": true}"}]}"#));
        assert!(anthropic_check(&url, "sk-ant-test", "claude-3-7-sonnet-latest").await.is_ok());
    }

    #[tokio::test]
    async fn anthropic_check_fails_on_http_error_status() {
        let url = format!("{}/messages", stub_server(401, r#"{"error":{"message":"invalid x-api-key"}}"#));
        let err = anthropic_check(&url, "sk-ant-bad", "claude-3-7-sonnet-latest").await.unwrap_err();
        assert!(err.contains("HTTP 401"), "unexpected error: {err}");
    }

    #[test]
    fn loopback_hosts_are_recognized() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("::1"));
    }

    #[test]
    fn non_loopback_hosts_are_rejected() {
        assert!(!is_loopback_host("192.168.1.5"));
        assert!(!is_loopback_host("my-server.local"));
        assert!(!is_loopback_host("0.0.0.0"));
    }
}
