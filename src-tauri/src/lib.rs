mod auth;
mod extraction;

use auth::state::{SigninState, SigninStatus};
use auth::token_store;
use extraction::check;
use extraction::settings::{ByokFrontierConfig, ByokProvider, ExtractionSettings, ExtractionSettingsState, ExtractionSource, LocalModelConfig};
use tauri::Manager;

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

// ── Extraction_Source settings (Module 4 tasks 4.4-4.8) ────────────────────────

fn app_data_root(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

/// Requirement 10.2 defense in depth — never trust the frontend's own warning UI
/// alone. Rejects a non-loopback endpoint unless the caller has explicitly opted in.
fn validate_local_model_endpoint(endpoint: &str, non_loopback_opt_in: bool) -> Result<(), String> {
    let parsed = url::Url::parse(endpoint).map_err(|e| format!("Invalid endpoint URL: {e}"))?;
    let host = parsed.host_str().ok_or_else(|| "Endpoint URL has no host".to_string())?;
    if !check::is_loopback_host(host) && !non_loopback_opt_in {
        return Err(format!(
            "\"{host}\" is not a loopback address — pass non_loopback_opt_in=true to confirm you understand the risk"
        ));
    }
    Ok(())
}

#[tauri::command]
fn get_extraction_settings(state: tauri::State<'_, ExtractionSettingsState>) -> ExtractionSettings {
    state.get()
}

#[tauri::command]
fn save_local_model_config(
    app: tauri::AppHandle,
    state: tauri::State<'_, ExtractionSettingsState>,
    endpoint: String,
    model: String,
    non_loopback_opt_in: bool,
) -> Result<(), String> {
    validate_local_model_endpoint(&endpoint, non_loopback_opt_in)?;

    let mut updated = state.get();
    updated.local_model = Some(LocalModelConfig {
        endpoint,
        model,
        non_loopback_opt_in,
        connectivity_check_passed: false, // reset — a stale pass shouldn't survive an edit
    });
    extraction::settings::save(&app_data_root(&app)?, &updated)?;
    state.set(updated);
    Ok(())
}

#[tauri::command]
fn save_byok_config(
    app: tauri::AppHandle,
    state: tauri::State<'_, ExtractionSettingsState>,
    provider: ByokProvider,
    model: String,
    api_key: String,
) -> Result<(), String> {
    token_store::save_byok_api_key(&api_key)?;

    let mut updated = state.get();
    updated.byok_frontier = Some(ByokFrontierConfig {
        provider,
        model,
        connectivity_check_passed: false,
    });
    extraction::settings::save(&app_data_root(&app)?, &updated)?;
    state.set(updated);
    Ok(())
}

#[tauri::command]
async fn test_local_model_connection(endpoint: String, model: String) -> Result<(), String> {
    check::run_local_model_check(&endpoint, &model).await
}

#[tauri::command]
async fn test_byok_connection(provider: ByokProvider, api_key: String, model: String) -> Result<(), String> {
    check::run_byok_check(provider, &api_key, &model).await
}

#[tauri::command]
async fn activate_extraction_source(
    app: tauri::AppHandle,
    state: tauri::State<'_, ExtractionSettingsState>,
    source: ExtractionSource,
) -> Result<(), String> {
    let mut updated = state.get();

    match source {
        ExtractionSource::ServerFallback => {}
        ExtractionSource::LocalModel => {
            let cfg = updated
                .local_model
                .clone()
                .ok_or_else(|| "No Local_Model configuration saved yet".to_string())?;
            // Always re-validate — never trust a prior connectivity_check_passed flag.
            check::run_local_model_check(&cfg.endpoint, &cfg.model).await?;
            updated.local_model = Some(LocalModelConfig { connectivity_check_passed: true, ..cfg });
        }
        ExtractionSource::ByokFrontier => {
            let cfg = updated
                .byok_frontier
                .clone()
                .ok_or_else(|| "No BYOK_Frontier configuration saved yet".to_string())?;
            let api_key = token_store::load_byok_api_key()?
                .ok_or_else(|| "No BYOK_Frontier API key found in the keychain".to_string())?;
            check::run_byok_check(cfg.provider, &api_key, &cfg.model).await?;
            updated.byok_frontier = Some(ByokFrontierConfig { connectivity_check_passed: true, ..cfg });
        }
    }

    updated.active_source = source;
    extraction::settings::save(&app_data_root(&app)?, &updated)?;
    state.set(updated);
    Ok(())
}

#[tauri::command]
fn delete_local_model_config(
    app: tauri::AppHandle,
    state: tauri::State<'_, ExtractionSettingsState>,
) -> Result<(), String> {
    let mut updated = state.get();
    updated.local_model = None;
    if updated.active_source == ExtractionSource::LocalModel {
        updated.active_source = ExtractionSource::ServerFallback;
    }
    extraction::settings::save(&app_data_root(&app)?, &updated)?;
    state.set(updated);
    Ok(())
}

#[tauri::command]
fn delete_byok_config(
    app: tauri::AppHandle,
    state: tauri::State<'_, ExtractionSettingsState>,
) -> Result<(), String> {
    token_store::delete_byok_api_key()?;

    let mut updated = state.get();
    updated.byok_frontier = None;
    if updated.active_source == ExtractionSource::ByokFrontier {
        updated.active_source = ExtractionSource::ServerFallback;
    }
    extraction::settings::save(&app_data_root(&app)?, &updated)?;
    state.set(updated);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(SigninState::default())
        .manage(ExtractionSettingsState::default())
        .setup(|app| {
            auth::silent_refresh::spawn(app.handle().clone());

            let root = app_data_root(&app.handle())?;
            let loaded = extraction::settings::load(&root)?;
            app.state::<ExtractionSettingsState>().set(loaded);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_google_signin,
            start_github_device_signin,
            get_signin_status,
            get_access_token,
            sign_out,
            get_extraction_settings,
            save_local_model_config,
            save_byok_config,
            test_local_model_connection,
            test_byok_connection,
            activate_extraction_source,
            delete_local_model_config,
            delete_byok_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
