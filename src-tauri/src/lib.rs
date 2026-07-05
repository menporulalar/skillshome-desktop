mod auth;

use auth::state::{SigninState, SigninStatus};
use auth::token_store;

#[tauri::command]
async fn start_google_signin(app: tauri::AppHandle, state: tauri::State<'_, SigninState>) -> Result<(), String> {
    auth::google::start_google_signin(app, state).await
}

#[tauri::command]
async fn start_github_device_signin(
    app: tauri::AppHandle,
    state: tauri::State<'_, SigninState>,
) -> Result<(), String> {
    auth::github_device::start_github_device_signin(app, state).await
}

#[tauri::command]
fn get_signin_status(state: tauri::State<'_, SigninState>) -> SigninStatus {
    state.status()
}

#[tauri::command]
fn get_access_token(state: tauri::State<'_, SigninState>) -> Option<String> {
    state.access_token()
}

#[tauri::command]
fn sign_out(state: tauri::State<'_, SigninState>) -> Result<(), String> {
    state.set_access_token(None);
    state.set_status(SigninStatus::Idle);
    token_store::delete_refresh_token()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(SigninState::default())
        .setup(|app| {
            auth::silent_refresh::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_google_signin,
            start_github_device_signin,
            get_signin_status,
            get_access_token,
            sign_out
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
