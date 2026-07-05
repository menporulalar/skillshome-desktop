//! Background loop that keeps a signed-in session alive past the 15-minute access-token
//! TTL, and restores a session on app launch if a refresh token is already in the
//! keychain. Runs independently of the interactive sign-in flows (`google.rs` /
//! `github_device.rs`) — the only thing they share is the keychain entry itself, so
//! nothing needs to coordinate directly with this loop.

use crate::auth::backend_client::{BackendClient, BackendError};
use crate::auth::state::{SigninState, SigninStatus};
use crate::auth::token_store;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Runtime};

/// Inside the 15-minute access-token TTL with comfortable margin.
const REFRESH_INTERVAL: Duration = Duration::from_secs(600);
const STATUS_EVENT: &str = "signin://status";

/// Spawns the loop once, from `run()`'s `.setup()` hook. Attempts a refresh immediately
/// on each start (before the first sleep), so this single loop covers both restoring a
/// session on launch and keeping a long-running session alive.
///
/// Generic over `R: Runtime` (rather than the concrete `Wry` runtime `AppHandle` defaults
/// to) so the same function can be driven by `tauri::test::mock_app()`'s `MockRuntime` in
/// unit tests.
pub fn spawn<R: Runtime + 'static>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let client = BackendClient::new();
        loop {
            attempt_refresh(&app, &client).await;
            tokio::time::sleep(REFRESH_INTERVAL).await;
        }
    });
}

/// Takes `client` as a parameter (rather than constructing its own `BackendClient`) so
/// tests can pass in one built with `BackendClient::with_base_url` pointed at a stub.
async fn attempt_refresh<R: Runtime>(app: &AppHandle<R>, client: &BackendClient) {
    let Ok(Some(refresh_token)) = token_store::load_refresh_token() else {
        return; // Nothing in the keychain — never signed in, or already signed out.
    };

    let state = app.state::<SigninState>();
    match client.refresh_access_token(&refresh_token).await {
        Ok(token) => {
            // Rotation: the backend issues a new refresh token on every successful call.
            let _ = token_store::save_refresh_token(&token.refresh_token);
            state.set_access_token(Some(token.access_token));
            state.set_status(SigninStatus::Success);
            let _ = app.emit(STATUS_EVENT, state.status());
        }
        Err(BackendError::Unauthorized) => {
            // Token consumed/expired/invalid — clear the stale keychain entry, matching
            // the web cookie-based route's own behavior of clearing a stale cookie.
            let _ = token_store::delete_refresh_token();
            state.set_access_token(None);
            state.set_status(SigninStatus::Idle);
            let _ = app.emit(STATUS_EVENT, state.status());
        }
        Err(_) => {
            // Transient (network/server error, or account pending approval) — leave the
            // keychain token and in-memory state alone, retry on the next tick.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spawns a one-shot HTTP stub on an ephemeral loopback port that always replies
    /// with `(status, body)` — mirrors `backend_client.rs`'s own test helper.
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

    fn mock_app_with_state() -> tauri::App<tauri::test::MockRuntime> {
        let app = tauri::test::mock_app();
        app.handle().manage(SigninState::default());
        app
    }

    /// `attempt_refresh` with no refresh token in the keychain must be a pure no-op:
    /// it must not touch `SigninState` at all (still `Idle`, no access token).
    #[tokio::test]
    async fn no_keychain_token_is_a_no_op() {
        let _guard = token_store::KEYCHAIN_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        token_store::delete_refresh_token().expect("pre-test cleanup should not fail");

        let app = mock_app_with_state();
        let client = BackendClient::with_base_url("http://127.0.0.1:1"); // never reached

        attempt_refresh(&app.handle().clone(), &client).await;

        let state = app.handle().state::<SigninState>();
        assert_eq!(state.status(), SigninStatus::Idle);
        assert_eq!(state.access_token(), None);
    }

    /// A successful refresh rotates the keychain token and marks the session `Success`.
    #[tokio::test]
    async fn successful_refresh_updates_state_and_rotates_keychain_token() {
        let _guard = token_store::KEYCHAIN_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        token_store::save_refresh_token("old-refresh-token").expect("seed keychain");

        let app = mock_app_with_state();
        let base = stub_server(200, r#"{"accessToken":"new-at","refreshToken":"new-rt"}"#);
        let client = BackendClient::with_base_url(base);

        attempt_refresh(&app.handle().clone(), &client).await;

        let state = app.handle().state::<SigninState>();
        assert_eq!(state.status(), SigninStatus::Success);
        assert_eq!(state.access_token(), Some("new-at".to_string()));
        assert_eq!(token_store::load_refresh_token().unwrap(), Some("new-rt".to_string()));

        token_store::delete_refresh_token().expect("post-test cleanup should not fail");
    }

    /// An `Unauthorized` response (invalid/expired/already-rotated token) clears the
    /// stale keychain entry and resets the session to `Idle` — never leaves it in
    /// `Success` with a token that the backend has already rejected.
    #[tokio::test]
    async fn unauthorized_clears_keychain_and_resets_to_idle() {
        let _guard = token_store::KEYCHAIN_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        token_store::save_refresh_token("stale-refresh-token").expect("seed keychain");

        let app = mock_app_with_state();
        let base = stub_server(401, r#"{"error":"Unauthorized"}"#);
        let client = BackendClient::with_base_url(base);

        attempt_refresh(&app.handle().clone(), &client).await;

        let state = app.handle().state::<SigninState>();
        assert_eq!(state.status(), SigninStatus::Idle);
        assert_eq!(state.access_token(), None);
        assert_eq!(token_store::load_refresh_token().unwrap(), None);
    }

    /// A transient failure (network/server error) must not clobber the keychain token
    /// or the in-memory state — the next tick should simply retry.
    #[tokio::test]
    async fn transient_error_leaves_keychain_and_state_untouched() {
        let _guard = token_store::KEYCHAIN_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        token_store::save_refresh_token("still-good-refresh-token").expect("seed keychain");

        let app = mock_app_with_state();
        app.handle().state::<SigninState>().set_status(SigninStatus::Success);
        app.handle().state::<SigninState>().set_access_token(Some("still-good-access-token".to_string()));

        let base = stub_server(500, r#"{"error":"boom"}"#);
        let client = BackendClient::with_base_url(base);

        attempt_refresh(&app.handle().clone(), &client).await;

        let state = app.handle().state::<SigninState>();
        assert_eq!(state.status(), SigninStatus::Success);
        assert_eq!(state.access_token(), Some("still-good-access-token".to_string()));
        assert_eq!(
            token_store::load_refresh_token().unwrap(),
            Some("still-good-refresh-token".to_string())
        );

        token_store::delete_refresh_token().expect("post-test cleanup should not fail");
    }
}
