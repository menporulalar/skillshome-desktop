//! GitHub sign-in via OAuth Device Flow (RFC 8628) — no callback URL and no local
//! listener at all, which is why this was chosen over a second loopback listener: the
//! existing GitHub OAuth App only supports one callback URL (already used by web
//! login), so device flow avoids needing a second, conflicting registration.
//!
//! Requires a separate GitHub OAuth App with "Device Flow" enabled in its settings —
//! the existing classic OAuth App used by web login does not have this enabled.

use crate::auth::backend_client::BackendClient;
use crate::auth::state::{SigninState, SigninStatus};
use crate::auth::token_store;
use serde::Deserialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const DEVICE_CODE_ENDPOINT: &str = "https://github.com/login/device/code";
const TOKEN_ENDPOINT: &str = "https://github.com/login/oauth/access_token";
const GITHUB_SCOPES: &str = "read:user user:email";
const STATUS_EVENT: &str = "signin://status";

fn github_client_id() -> &'static str {
    option_env!("GITHUB_DEVICE_CLIENT_ID").unwrap_or("")
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[allow(dead_code)]
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct TokenPollResponse {
    access_token: Option<String>,
    error: Option<String>,
}

pub async fn start_github_device_signin(
    app: AppHandle,
    state: tauri::State<'_, SigninState>,
) -> Result<(), String> {
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
    if github_client_id().is_empty() {
        return Err("GITHUB_DEVICE_CLIENT_ID is not configured for this build".to_string());
    }

    let http = reqwest::Client::new();

    let device: DeviceCodeResponse = http
        .post(DEVICE_CODE_ENDPOINT)
        .header("Accept", "application/json")
        .form(&[("client_id", github_client_id()), ("scope", GITHUB_SCOPES)])
        .send()
        .await
        .map_err(|e| format!("failed to request a GitHub device code: {e}"))?
        .json()
        .await
        .map_err(|e| format!("unexpected response requesting a GitHub device code: {e}"))?;

    let awaiting = SigninStatus::AwaitingDeviceConfirmation {
        user_code: device.user_code.clone(),
        verification_uri: device.verification_uri.clone(),
    };
    let _ = app.emit(STATUS_EVENT, awaiting);

    tauri_plugin_opener::open_url(&device.verification_uri, None::<&str>)
        .map_err(|e| format!("failed to open system browser: {e}"))?;

    let access_token = poll_for_token(&http, &device.device_code, device.interval).await?;

    BackendClient::new()
        .exchange_github_token(&access_token)
        .await
        .map_err(|e| e.to_string())
}

async fn poll_for_token(http: &reqwest::Client, device_code: &str, interval: u64) -> Result<String, String> {
    let mut interval = Duration::from_secs(interval.max(1));

    loop {
        tokio::time::sleep(interval).await;

        let resp: TokenPollResponse = http
            .post(TOKEN_ENDPOINT)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", github_client_id()),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|e| format!("failed to poll for a GitHub access token: {e}"))?
            .json()
            .await
            .map_err(|e| format!("unexpected response polling for a GitHub access token: {e}"))?;

        if let Some(token) = resp.access_token {
            return Ok(token);
        }

        match resp.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                interval += Duration::from_secs(5);
                continue;
            }
            Some("expired_token") => return Err("the GitHub device code expired before you confirmed it".to_string()),
            Some("access_denied") => return Err("sign-in was cancelled".to_string()),
            Some(other) => return Err(format!("GitHub device flow error: {other}")),
            None => return Err("GitHub device flow returned neither a token nor an error".to_string()),
        }
    }
}
