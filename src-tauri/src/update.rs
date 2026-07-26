// Feature #28 — R1 (updater plugin integration) + R2 (signed releases).
//
// Signature verification is handled by tauri-plugin-updater itself: it checks the
// manifest/binary against the `plugins.updater.pubkey` embedded in tauri.conf.json.
// There is no code path here that can skip or weaken that check — an unsigned or
// tampered manifest fails inside `check()`/`download_and_install()` and surfaces as
// an `Err` to the caller.
//
// R3 (the update-available UI and the "don't restart during an active extraction"
// guard) is NOT implemented here — these are the plumbing commands the frontend will
// call once that screen exists. `apply_update` restarts the app immediately, so the
// frontend must only invoke it from an explicit user action.

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(serde::Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub body: Option<String>,
}

/// Query the configured endpoint for a newer release. Read-only: never downloads
/// or installs, so it is safe to call on startup or on a timer while an extraction
/// or project sync is running.
pub async fn check_for_updates(app: &AppHandle) -> Result<UpdateInfo, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;

    // Read the version the updater itself compares against (tauri.conf.json's
    // `version`), rather than env!("CARGO_PKG_VERSION") — the two are in sync today
    // but only this one is authoritative for the comparison the plugin performs.
    let current_version = app.package_info().version.to_string();

    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateInfo {
            available: true,
            current_version,
            latest_version: Some(update.version.clone()),
            body: update.body.clone(),
        }),
        Ok(None) => Ok(UpdateInfo {
            available: false,
            current_version,
            latest_version: None,
            body: None,
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// Download, verify, install, and restart into the new version.
///
/// This RESTARTS THE APP on success and does not return. It performs its own
/// `check()` rather than trusting a version passed in from the webview, so the
/// binary installed is always one the updater just verified against the embedded
/// public key. Callers must treat this as destructive: the R3 UI is responsible for
/// confirming with the user and for refusing to call it while an extraction or
/// project sync is in flight.
pub async fn apply_update(app: &AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;

    if let Some(update) = updater.check().await.map_err(|e| e.to_string())? {
        update
            .download_and_install(
                // Per-chunk progress. Wired to no-ops for now; surfacing a progress
                // bar is R5 (P1), and needs an event emit rather than a callback here.
                |_chunk_len, _content_len| {},
                || {},
            )
            .await
            .map_err(|e| e.to_string())?;

        // Diverges — nothing after this line runs.
        app.restart();
    }

    // Reached only when no update was available.
    Ok(())
}
