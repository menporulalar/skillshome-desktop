//! Google sign-in via the RFC 8252 loopback-redirect pattern: bind a one-shot local
//! HTTP listener, send the user to the system browser for Google's consent screen,
//! and wait for Google to redirect back to us with an authorization code.
//!
//! Requires a Google Cloud "Desktop app" type OAuth client (only that client type
//! accepts the any-port loopback exception) — the existing web login client is a
//! "Web application" type and cannot be reused for this flow.

use crate::auth::backend_client::BackendClient;
use crate::auth::pkce;
use crate::auth::state::{SigninState, SigninStatus};
use crate::auth::token_store;
use tauri::{AppHandle, Emitter};

/// Must match `DESKTOP_OAUTH_GOOGLE_LOOPBACK_PORT` in skillshome-app's
/// `lib/mcp/desktopOAuthConfig.ts` — both sides need to agree on the redirect URI
/// registered with Google.
const GOOGLE_LOOPBACK_PORT: u16 = 53791;
const GOOGLE_AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_SCOPES: &str = "openid email profile";
const STATUS_EVENT: &str = "signin://status";

fn google_client_id() -> &'static str {
    // `option_env!`, not `env!`: a dev build must compile before the Desktop-type
    // Google Cloud client has been created.
    option_env!("GOOGLE_DESKTOP_CLIENT_ID").unwrap_or("")
}

fn redirect_uri() -> String {
    format!("http://127.0.0.1:{GOOGLE_LOOPBACK_PORT}/callback")
}

pub async fn start_google_signin(app: AppHandle, state: tauri::State<'_, SigninState>) -> Result<(), String> {
    let outcome = run(&app).await;
    match &outcome {
        Ok(token) => {
            token_store::save_refresh_token(&token.refresh_token)?;
            state.set_access_token(Some(token.access_token.clone()));
            state.set_status(SigninStatus::Success);
        }
        Err(message) => {
            state.set_status(SigninStatus::Error {
                message: message.clone(),
            });
        }
    }
    let _ = app.emit(STATUS_EVENT, state.status());
    outcome.map(|_| ())
}

async fn run(app: &AppHandle) -> Result<crate::auth::backend_client::TokenResponse, String> {
    if google_client_id().is_empty() {
        return Err("GOOGLE_DESKTOP_CLIENT_ID is not configured for this build".to_string());
    }

    let verifier = pkce::generate_code_verifier();
    let challenge = pkce::generate_code_challenge(&verifier);
    let csrf_state = pkce::generate_state();
    let redirect = redirect_uri();

    let mut auth_url = url::Url::parse(GOOGLE_AUTH_ENDPOINT).map_err(|e| e.to_string())?;
    auth_url
        .query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", google_client_id())
        .append_pair("redirect_uri", &redirect)
        .append_pair("scope", GOOGLE_SCOPES)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &csrf_state);

    let server = tiny_http::Server::http(("127.0.0.1", GOOGLE_LOOPBACK_PORT)).map_err(|e| {
        format!("failed to bind loopback listener on 127.0.0.1:{GOOGLE_LOOPBACK_PORT}: {e}")
    })?;

    let _ = app.emit(STATUS_EVENT, SigninStatus::AwaitingBrowser);
    tauri_plugin_opener::open_url(auth_url.to_string(), None::<&str>)
        .map_err(|e| format!("failed to open system browser: {e}"))?;

    let (code, returned_state) = tokio::task::spawn_blocking(move || accept_callback(server))
        .await
        .map_err(|e| e.to_string())??;

    if returned_state != csrf_state {
        return Err("state mismatch on Google callback — possible CSRF, aborting sign-in".to_string());
    }

    BackendClient::new()
        .exchange_google_code(&code, &verifier, &redirect)
        .await
        .map_err(|e| e.to_string())
}

/// Blocks on exactly one HTTP request (the loopback redirect), then shuts the listener
/// down by returning — `tiny_http::Server` is dropped, releasing the port.
fn accept_callback(server: tiny_http::Server) -> Result<(String, String), String> {
    let request = server.recv().map_err(|e| e.to_string())?;

    let full_url = format!("http://127.0.0.1{}", request.url());
    let parsed = url::Url::parse(&full_url).map_err(|e| e.to_string())?;

    let mut code = None;
    let mut returned_state = None;
    let mut provider_error = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => returned_state = Some(value.into_owned()),
            "error" => provider_error = Some(value.into_owned()),
            _ => {}
        }
    }

    let success = code.is_some() && provider_error.is_none();
    let body = if success {
        "<html><body><h1>Signed in</h1><p>You can close this tab and return to SkillsHome Desktop.</p></body></html>"
    } else {
        "<html><body><h1>Sign-in failed</h1><p>You can close this tab and return to SkillsHome Desktop.</p></body></html>"
    };
    let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
        .expect("static header is valid");
    let _ = request.respond(tiny_http::Response::from_string(body).with_header(header));

    if let Some(err) = provider_error {
        return Err(format!("Google returned an error: {err}"));
    }

    match (code, returned_state) {
        (Some(code), Some(returned_state)) => Ok((code, returned_state)),
        _ => Err("callback was missing the code or state parameter".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    /// Full bind → accept → parse → respond cycle against a real loopback socket
    /// (ephemeral port, not the fixed Google port, so this can run alongside anything
    /// else that might be using 53791 in dev).
    #[test]
    fn accept_callback_parses_code_and_state_and_responds() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind ephemeral port");
        let addr = server.server_addr().to_ip().expect("ip address");

        let requester = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect to loopback server");
            stream
                .write_all(
                    b"GET /callback?code=test-code&state=test-state HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                )
                .expect("write request");
            let mut response = String::new();
            stream.read_to_string(&mut response).expect("read response");
            response
        });

        let (code, returned_state) = accept_callback(server).expect("callback should parse");
        assert_eq!(code, "test-code");
        assert_eq!(returned_state, "test-state");

        let response = requester.join().expect("requester thread should not panic");
        assert!(response.starts_with("HTTP/1.1 200"), "unexpected response: {response}");
        assert!(response.contains("Signed in"));
    }

    #[test]
    fn accept_callback_surfaces_provider_error() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind ephemeral port");
        let addr = server.server_addr().to_ip().expect("ip address");

        let requester = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect to loopback server");
            stream
                .write_all(
                    b"GET /callback?error=access_denied&state=test-state HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                )
                .expect("write request");
            let mut response = String::new();
            stream.read_to_string(&mut response).expect("read response");
        });

        let result = accept_callback(server);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("access_denied"));

        requester.join().expect("requester thread should not panic");
    }
}
