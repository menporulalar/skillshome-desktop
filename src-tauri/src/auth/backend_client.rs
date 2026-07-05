//! Thin client for skillshome-app's `POST /api/auth/desktop/token` — the only backend
//! endpoint the desktop app talks to for sign-in. No cookies anywhere: JSON in, JSON out.
//!
//! `base_url` is swappable (see `with_base_url`) so verification can point this at a
//! stub server instead of a real deployment, per the plan's "buildable/testable now"
//! guidance — the live-provider round-trip is blocked on the two new OAuth app
//! registrations, but everything up to the HTTP call itself is not.

use serde::{Deserialize, Serialize};

/// Baked in at compile time. `option_env!` (not `env!`) so a dev build compiles even
/// before the two new OAuth app registrations exist — a real release pipeline should
/// supply these, but an unset var must not block local iteration.
fn default_backend_url() -> &'static str {
    option_env!("SKILLSHOME_BACKEND_URL").unwrap_or("http://localhost:3000")
}

#[derive(Serialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
enum DesktopTokenRequest<'a> {
    Google {
        code: &'a str,
        #[serde(rename = "codeVerifier")]
        code_verifier: &'a str,
        #[serde(rename = "redirectUri")]
        redirect_uri: &'a str,
    },
    Github {
        #[serde(rename = "accessToken")]
        access_token: &'a str,
    },
}

#[derive(Deserialize, Debug, Clone)]
pub struct TokenResponse {
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
}

#[derive(Serialize)]
struct RefreshRequest<'a> {
    #[serde(rename = "refreshToken")]
    refresh_token: &'a str,
}

#[derive(Deserialize, Debug)]
struct ErrorBody {
    error: String,
}

#[derive(Debug)]
pub enum BackendError {
    /// User account exists but hasn't been approved yet (invite-only access gate).
    AccessPending,
    /// The refresh token itself is invalid, expired, or already rotated — distinct
    /// from `AccessPending`, which means a valid account is just not approved yet.
    Unauthorized,
    /// Any other non-2xx response, with the backend's `error` message when present.
    Rejected(String),
    /// Transport-level failure (no response at all).
    Network(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::AccessPending => write!(f, "account is pending approval"),
            BackendError::Unauthorized => write!(f, "refresh token is invalid or expired"),
            BackendError::Rejected(msg) => write!(f, "{msg}"),
            BackendError::Network(msg) => write!(f, "network error: {msg}"),
        }
    }
}

pub struct BackendClient {
    base_url: String,
    http: reqwest::Client,
}

impl BackendClient {
    pub fn new() -> Self {
        Self {
            base_url: default_backend_url().to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http: reqwest::Client::new(),
        }
    }

    async fn post_token(&self, body: DesktopTokenRequest<'_>) -> Result<TokenResponse, BackendError> {
        let url = format!("{}/api/auth/desktop/token", self.base_url);
        let resp = self
            .http
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Network(e.to_string()))?;

        if resp.status().is_success() {
            resp.json::<TokenResponse>()
                .await
                .map_err(|e| BackendError::Network(e.to_string()))
        } else {
            let status = resp.status();
            let body: ErrorBody = resp.json().await.unwrap_or(ErrorBody {
                error: format!("request failed with status {status}"),
            });
            if status == reqwest::StatusCode::FORBIDDEN && body.error == "access_pending" {
                Err(BackendError::AccessPending)
            } else {
                Err(BackendError::Rejected(body.error))
            }
        }
    }

    pub async fn exchange_google_code(
        &self,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
    ) -> Result<TokenResponse, BackendError> {
        self.post_token(DesktopTokenRequest::Google {
            code,
            code_verifier,
            redirect_uri,
        })
        .await
    }

    pub async fn exchange_github_token(&self, access_token: &str) -> Result<TokenResponse, BackendError> {
        self.post_token(DesktopTokenRequest::Github { access_token })
            .await
    }

    /// Calls `POST /api/auth/desktop/refresh` — the cookie-free sibling of the web
    /// app's cookie-based refresh route, used by the silent-refresh background loop.
    pub async fn refresh_access_token(&self, refresh_token: &str) -> Result<TokenResponse, BackendError> {
        let url = format!("{}/api/auth/desktop/refresh", self.base_url);
        let resp = self
            .http
            .post(url)
            .json(&RefreshRequest { refresh_token })
            .send()
            .await
            .map_err(|e| BackendError::Network(e.to_string()))?;

        if resp.status().is_success() {
            resp.json::<TokenResponse>()
                .await
                .map_err(|e| BackendError::Network(e.to_string()))
        } else if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            Err(BackendError::Unauthorized)
        } else {
            let status = resp.status();
            let body: ErrorBody = resp.json().await.unwrap_or(ErrorBody {
                error: format!("request failed with status {status}"),
            });
            Err(BackendError::Rejected(body.error))
        }
    }
}

impl Default for BackendClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spawns a one-shot HTTP stub on an ephemeral loopback port that always replies
    /// with `(status, body)`, and returns the base URL to point a `BackendClient` at.
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
    async fn exchange_google_code_success() {
        let base = stub_server(200, r#"{"accessToken":"at-1","refreshToken":"rt-1"}"#);
        let client = BackendClient::with_base_url(base);

        let token = client
            .exchange_google_code("code", "verifier", "http://127.0.0.1:53791/callback")
            .await
            .expect("should succeed");

        assert_eq!(token.access_token, "at-1");
        assert_eq!(token.refresh_token, "rt-1");
    }

    #[tokio::test]
    async fn exchange_github_token_success() {
        let base = stub_server(200, r#"{"accessToken":"at-2","refreshToken":"rt-2"}"#);
        let client = BackendClient::with_base_url(base);

        let token = client
            .exchange_github_token("gh-access-token")
            .await
            .expect("should succeed");

        assert_eq!(token.access_token, "at-2");
        assert_eq!(token.refresh_token, "rt-2");
    }

    #[tokio::test]
    async fn access_pending_maps_to_dedicated_variant() {
        let base = stub_server(403, r#"{"error":"access_pending"}"#);
        let client = BackendClient::with_base_url(base);

        let err = client
            .exchange_github_token("gh-access-token")
            .await
            .expect_err("should fail");

        assert!(matches!(err, BackendError::AccessPending));
    }

    #[tokio::test]
    async fn other_errors_carry_the_backend_message() {
        let base = stub_server(500, r#"{"error":"boom"}"#);
        let client = BackendClient::with_base_url(base);

        let err = client
            .exchange_github_token("gh-access-token")
            .await
            .expect_err("should fail");

        match err {
            BackendError::Rejected(message) => assert_eq!(message, "boom"),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn network_failure_is_reported() {
        // Nothing listening on this port — connection should be refused.
        let client = BackendClient::with_base_url("http://127.0.0.1:1");

        let err = client
            .exchange_github_token("gh-access-token")
            .await
            .expect_err("should fail");

        assert!(matches!(err, BackendError::Network(_)));
    }

    #[tokio::test]
    async fn refresh_access_token_success() {
        let base = stub_server(200, r#"{"accessToken":"at-3","refreshToken":"rt-3"}"#);
        let client = BackendClient::with_base_url(base);

        let token = client
            .refresh_access_token("old-refresh-token")
            .await
            .expect("should succeed");

        assert_eq!(token.access_token, "at-3");
        assert_eq!(token.refresh_token, "rt-3");
    }

    #[tokio::test]
    async fn refresh_access_token_401_maps_to_unauthorized() {
        let base = stub_server(401, r#"{"error":"Unauthorized"}"#);
        let client = BackendClient::with_base_url(base);

        let err = client
            .refresh_access_token("stale-or-invalid-token")
            .await
            .expect_err("should fail");

        assert!(matches!(err, BackendError::Unauthorized));
    }

    #[tokio::test]
    async fn refresh_access_token_other_error_is_rejected() {
        let base = stub_server(429, r#"{"error":"Rate limit exceeded. Please try again later."}"#);
        let client = BackendClient::with_base_url(base);

        let err = client
            .refresh_access_token("some-token")
            .await
            .expect_err("should fail");

        match err {
            BackendError::Rejected(message) => assert_eq!(message, "Rate limit exceeded. Please try again later."),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn refresh_access_token_network_failure_is_reported() {
        let client = BackendClient::with_base_url("http://127.0.0.1:1");

        let err = client
            .refresh_access_token("some-token")
            .await
            .expect_err("should fail");

        assert!(matches!(err, BackendError::Network(_)));
    }
}
