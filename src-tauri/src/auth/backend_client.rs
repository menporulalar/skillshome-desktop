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
///
/// `pub(crate)` — task 4.12's sidecar-spawning commands (`lib.rs`) also need this
/// same value to tell the sidecar which backend to talk to over MCP, and should
/// stay in sync with the REST client's own default rather than re-deriving it.
pub(crate) fn default_backend_url() -> &'static str {
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

/// Minimal fields off a Prisma `Profile` row — `GET /api/profiles` returns the full
/// row shape, but serde ignores the many fields this struct doesn't name.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProfileSummary {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

/// Mirrors `GET /api/profiles/[id]/ingest/status`'s response body exactly.
/// `review_package` is kept as an opaque `serde_json::Value` — this client never
/// inspects `ReviewPackage`'s 6-variant nested shape, only transports it between
/// this GET and the `confirm_ingest` POST. Real typing for it belongs in whatever
/// TypeScript review UI eventually renders/edits it.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct IngestStatusResponse {
    #[serde(rename = "jobId")]
    pub job_id: String,
    /// Kept as a raw string, not an enum — avoids drift if the backend adds a
    /// status value this client doesn't know about yet.
    pub status: String,
    pub progress: Option<u8>,
    #[serde(rename = "reviewPackage")]
    pub review_package: Option<serde_json::Value>,
    #[serde(rename = "extractedSkills")]
    pub extracted_skills: u32,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "statusLabel")]
    pub status_label: String,
}

#[derive(Debug)]
pub enum BackendError {
    /// User account exists but hasn't been approved yet (invite-only access gate).
    AccessPending,
    /// The refresh token itself is invalid, expired, or already rotated — distinct
    /// from `AccessPending`, which means a valid account is just not approved yet.
    Unauthorized,
    /// The caller's daily ingestion budget is used up (429 from `POST .../ingest`).
    IngestLimitReached(String),
    /// A job is already in flight for this profile (409 from `POST .../ingest`) —
    /// only one active ingestion job per profile at a time.
    IngestInProgress(String),
    /// No ingestion job exists yet for this profile (404 from
    /// `GET .../ingest/status`) — an expected state when polling before
    /// `start_ingest` has ever been called for this profile, not a real failure.
    NoActiveJob,
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
            BackendError::IngestLimitReached(msg) => write!(f, "{msg}"),
            BackendError::IngestInProgress(msg) => write!(f, "{msg}"),
            BackendError::NoActiveJob => write!(f, "no ingestion job found for this profile"),
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

    /// Shared `Authorization: Bearer` request/response handling for the
    /// server-fallback ingestion endpoints below — separate from `post_token`'s
    /// unauthenticated branching (which has its own 403/access_pending special
    /// case) since the two error-mapping tables genuinely differ.
    async fn send_authenticated<T: serde::de::DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
        access_token: &str,
    ) -> Result<T, BackendError> {
        let resp = req
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| BackendError::Network(e.to_string()))?;

        if resp.status().is_success() {
            return resp
                .json::<T>()
                .await
                .map_err(|e| BackendError::Network(e.to_string()));
        }

        let status = resp.status();
        let body: ErrorBody = resp.json().await.unwrap_or(ErrorBody {
            error: format!("request failed with status {status}"),
        });

        Err(match status {
            reqwest::StatusCode::UNAUTHORIZED => BackendError::Unauthorized,
            reqwest::StatusCode::NOT_FOUND => BackendError::NoActiveJob,
            reqwest::StatusCode::CONFLICT => BackendError::IngestInProgress(body.error),
            reqwest::StatusCode::TOO_MANY_REQUESTS => BackendError::IngestLimitReached(body.error),
            _ => BackendError::Rejected(body.error),
        })
    }

    /// `GET /api/profiles` — used to resolve which profile the Server_Fallback
    /// path should ingest into. Assumes at least one profile already exists from
    /// prior web onboarding; creating a new one from the desktop app is out of scope.
    pub async fn list_profiles(&self, access_token: &str) -> Result<Vec<ProfileSummary>, BackendError> {
        let url = format!("{}/api/profiles", self.base_url);
        self.send_authenticated(self.http.get(url), access_token).await
    }

    /// `POST /api/profiles/{id}/ingest` — multipart file upload. Returns the new
    /// job's id. The backend hardcodes `autoConfirm: true` for every caller of this
    /// route, so extracted items are already committed by the time the status
    /// endpoint reports `awaiting_review` — see `confirm_ingest`'s doc comment.
    pub async fn start_ingest(
        &self,
        access_token: &str,
        profile_id: &str,
        file_bytes: Vec<u8>,
        filename: &str,
        mime: &str,
    ) -> Result<String, BackendError> {
        let url = format!("{}/api/profiles/{}/ingest", self.base_url, profile_id);
        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(filename.to_string())
            .mime_str(mime)
            .map_err(|e| BackendError::Network(e.to_string()))?;
        let form = reqwest::multipart::Form::new().part("file", part);

        #[derive(Deserialize)]
        struct StartIngestResponse {
            #[serde(rename = "jobId")]
            job_id: String,
        }

        let resp: StartIngestResponse = self
            .send_authenticated(self.http.post(url).multipart(form), access_token)
            .await?;
        Ok(resp.job_id)
    }

    /// `POST /api/profiles/{id}/ingest` — URL-only ingestion (GitHub repo/doc link),
    /// the JSON-body sibling of `start_ingest`'s multipart upload. The REST route
    /// dispatches on `Content-Type`, not on the presence of a file, so this must send
    /// a plain JSON `{url}` body rather than a multipart form with a `url` field.
    pub async fn start_ingest_url(
        &self,
        access_token: &str,
        profile_id: &str,
        url: &str,
    ) -> Result<String, BackendError> {
        let endpoint = format!("{}/api/profiles/{}/ingest", self.base_url, profile_id);

        #[derive(Serialize)]
        struct UrlIngestBody<'a> {
            url: &'a str,
        }

        #[derive(Deserialize)]
        struct StartIngestResponse {
            #[serde(rename = "jobId")]
            job_id: String,
        }

        let resp: StartIngestResponse = self
            .send_authenticated(self.http.post(endpoint).json(&UrlIngestBody { url }), access_token)
            .await?;
        Ok(resp.job_id)
    }

    /// `GET /api/profiles/{id}/ingest/status` — one active job per profile, no job
    /// id in the path. `BackendError::NoActiveJob` (404) is an expected state, not
    /// a real failure — callers should treat it as "still pending," not surface it.
    pub async fn get_ingest_status(
        &self,
        access_token: &str,
        profile_id: &str,
    ) -> Result<IngestStatusResponse, BackendError> {
        let url = format!("{}/api/profiles/{}/ingest/status", self.base_url, profile_id);
        self.send_authenticated(self.http.get(url), access_token).await
    }

    /// `POST /api/profiles/{id}/ingest/confirm` — despite the name, this isn't a
    /// first-persistence gate for this route (see `start_ingest`'s doc comment):
    /// its real job is applying the user's review edits/rejections on top of data
    /// that's already committed. `review_package` is passed through as an opaque
    /// JSON value — this client never inspects its shape.
    ///
    /// Known latent limitation (not fixed here): the backend's
    /// `ProfileService.commitReviewPackage` returns HTTP 400 if the review package
    /// has zero accepted items. Whatever UI calls this should avoid sending an
    /// all-rejected package.
    pub async fn confirm_ingest(
        &self,
        access_token: &str,
        profile_id: &str,
        review_package: serde_json::Value,
    ) -> Result<(), BackendError> {
        #[derive(Serialize)]
        struct ConfirmBody {
            #[serde(rename = "confirmedItems")]
            confirmed_items: serde_json::Value,
        }

        let url = format!("{}/api/profiles/{}/ingest/confirm", self.base_url, profile_id);
        let _: serde_json::Value = self
            .send_authenticated(
                self.http.post(url).json(&ConfirmBody { confirmed_items: review_package }),
                access_token,
            )
            .await?;
        Ok(())
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

    #[tokio::test]
    async fn list_profiles_success() {
        let base = stub_server(200, r#"[{"id":"p-1","displayName":"Jane Doe"}]"#);
        let client = BackendClient::with_base_url(base);

        let profiles = client.list_profiles("token").await.expect("should succeed");

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, "p-1");
        assert_eq!(profiles[0].display_name, "Jane Doe");
    }

    #[tokio::test]
    async fn list_profiles_401_maps_to_unauthorized() {
        let base = stub_server(401, r#"{"error":"Unauthorized"}"#);
        let client = BackendClient::with_base_url(base);

        let err = client.list_profiles("token").await.expect_err("should fail");

        assert!(matches!(err, BackendError::Unauthorized));
    }

    #[tokio::test]
    async fn list_profiles_network_failure_is_reported() {
        let client = BackendClient::with_base_url("http://127.0.0.1:1");

        let err = client.list_profiles("token").await.expect_err("should fail");

        assert!(matches!(err, BackendError::Network(_)));
    }

    #[tokio::test]
    async fn start_ingest_success() {
        let base = stub_server(202, r#"{"jobId":"job-1","status":"pending"}"#);
        let client = BackendClient::with_base_url(base);

        let job_id = client
            .start_ingest("token", "profile-1", b"fake resume bytes".to_vec(), "resume.pdf", "application/pdf")
            .await
            .expect("should succeed");

        assert_eq!(job_id, "job-1");
    }

    #[tokio::test]
    async fn start_ingest_429_maps_to_ingest_limit_reached() {
        let base = stub_server(429, r#"{"error":"Daily ingestion limit reached (3/day for free tier)."}"#);
        let client = BackendClient::with_base_url(base);

        let err = client
            .start_ingest("token", "profile-1", b"bytes".to_vec(), "resume.pdf", "application/pdf")
            .await
            .expect_err("should fail");

        match err {
            BackendError::IngestLimitReached(msg) => assert_eq!(msg, "Daily ingestion limit reached (3/day for free tier)."),
            other => panic!("expected IngestLimitReached, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn start_ingest_409_maps_to_ingest_in_progress() {
        let base = stub_server(409, r#"{"error":"An ingestion job is already in progress for this profile. Please wait for it to complete."}"#);
        let client = BackendClient::with_base_url(base);

        let err = client
            .start_ingest("token", "profile-1", b"bytes".to_vec(), "resume.pdf", "application/pdf")
            .await
            .expect_err("should fail");

        assert!(matches!(err, BackendError::IngestInProgress(_)));
    }

    #[tokio::test]
    async fn start_ingest_network_failure_is_reported() {
        let client = BackendClient::with_base_url("http://127.0.0.1:1");

        let err = client
            .start_ingest("token", "profile-1", b"bytes".to_vec(), "resume.pdf", "application/pdf")
            .await
            .expect_err("should fail");

        assert!(matches!(err, BackendError::Network(_)));
    }

    #[tokio::test]
    async fn start_ingest_url_success() {
        let base = stub_server(202, r#"{"jobId":"job-2","status":"pending"}"#);
        let client = BackendClient::with_base_url(base);

        let job_id = client
            .start_ingest_url("token", "profile-1", "https://github.com/octocat/hello-world")
            .await
            .expect("should succeed");

        assert_eq!(job_id, "job-2");
    }

    #[tokio::test]
    async fn start_ingest_url_429_maps_to_ingest_limit_reached() {
        let base = stub_server(429, r#"{"error":"Daily ingestion limit reached (3/day for free tier)."}"#);
        let client = BackendClient::with_base_url(base);

        let err = client
            .start_ingest_url("token", "profile-1", "https://github.com/octocat/hello-world")
            .await
            .expect_err("should fail");

        match err {
            BackendError::IngestLimitReached(msg) => assert_eq!(msg, "Daily ingestion limit reached (3/day for free tier)."),
            other => panic!("expected IngestLimitReached, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn start_ingest_url_409_maps_to_ingest_in_progress() {
        let base = stub_server(409, r#"{"error":"An ingestion job is already in progress for this profile. Please wait for it to complete."}"#);
        let client = BackendClient::with_base_url(base);

        let err = client
            .start_ingest_url("token", "profile-1", "https://github.com/octocat/hello-world")
            .await
            .expect_err("should fail");

        assert!(matches!(err, BackendError::IngestInProgress(_)));
    }

    #[tokio::test]
    async fn start_ingest_url_network_failure_is_reported() {
        let client = BackendClient::with_base_url("http://127.0.0.1:1");

        let err = client
            .start_ingest_url("token", "profile-1", "https://github.com/octocat/hello-world")
            .await
            .expect_err("should fail");

        assert!(matches!(err, BackendError::Network(_)));
    }

    #[tokio::test]
    async fn get_ingest_status_success() {
        let base = stub_server(
            200,
            r#"{"jobId":"job-1","status":"awaiting_review","progress":100,"reviewPackage":{"skills":[]},"extractedSkills":0,"errorMessage":null,"statusLabel":"Ready for your review"}"#,
        );
        let client = BackendClient::with_base_url(base);

        let status = client.get_ingest_status("token", "profile-1").await.expect("should succeed");

        assert_eq!(status.job_id, "job-1");
        assert_eq!(status.status, "awaiting_review");
        assert_eq!(status.progress, Some(100));
        assert!(status.review_package.is_some());
    }

    #[tokio::test]
    async fn get_ingest_status_404_maps_to_no_active_job() {
        let base = stub_server(404, r#"{"error":"No ingestion job found"}"#);
        let client = BackendClient::with_base_url(base);

        let err = client.get_ingest_status("token", "profile-1").await.expect_err("should fail");

        assert!(matches!(err, BackendError::NoActiveJob));
    }

    #[tokio::test]
    async fn get_ingest_status_network_failure_is_reported() {
        let client = BackendClient::with_base_url("http://127.0.0.1:1");

        let err = client.get_ingest_status("token", "profile-1").await.expect_err("should fail");

        assert!(matches!(err, BackendError::Network(_)));
    }

    #[tokio::test]
    async fn confirm_ingest_success() {
        let base = stub_server(200, r#"{"committed":true}"#);
        let client = BackendClient::with_base_url(base);

        client
            .confirm_ingest("token", "profile-1", serde_json::json!({"skills": []}))
            .await
            .expect("should succeed");
    }

    #[tokio::test]
    async fn confirm_ingest_400_zero_accepted_items_is_rejected() {
        let base = stub_server(400, r#"{"error":"At least one item must be accepted."}"#);
        let client = BackendClient::with_base_url(base);

        let err = client
            .confirm_ingest("token", "profile-1", serde_json::json!({"skills": []}))
            .await
            .expect_err("should fail");

        match err {
            BackendError::Rejected(msg) => assert_eq!(msg, "At least one item must be accepted."),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn confirm_ingest_network_failure_is_reported() {
        let client = BackendClient::with_base_url("http://127.0.0.1:1");

        let err = client
            .confirm_ingest("token", "profile-1", serde_json::json!({}))
            .await
            .expect_err("should fail");

        assert!(matches!(err, BackendError::Network(_)));
    }
}
