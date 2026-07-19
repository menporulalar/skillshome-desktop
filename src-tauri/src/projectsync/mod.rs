//! #25 living-profile-project-skill-sync — Module 3's desktop half: the
//! Local_Project_Grant store and the weekly/on-open scan plumbing.
//!
//! PRIVACY INVARIANT (design.md §1): the REAL folder path lives ONLY in this
//! local grants file. The server knows a connected local project by its
//! candidate-chosen display label and generated id — the path never crosses
//! the network. `connect_local_project` sends label+consent over REST, then
//! records the path locally in the same command.
//!
//! Persistence mirrors `extraction::settings` (plain JSON under the app data
//! dir, hand-wired, Mutex-guarded); REST stays in Rust like every other
//! backend call in this app (`auth::backend_client` precedent) — the React
//! layer only ever `invoke`s.

pub mod grants;

use crate::auth::state::SigninState;
use crate::ingest::sidecar;
use grants::{GrantsState, LocalProjectGrant};
use serde::{Deserialize, Serialize};

fn require_access_token(state: &tauri::State<'_, SigninState>) -> Result<String, String> {
    state
        .access_token()
        .ok_or_else(|| "not signed in — no access token available".to_string())
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ── Backend REST (skillshome-app Module 1 API) ───────────────────────────────

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ConnectedProjectRow {
    pub id: String,
    #[serde(rename = "sourceType")]
    pub source_type: String,
    #[serde(rename = "localPathLabel")]
    pub local_path_label: Option<String>,
    #[serde(rename = "githubRepoFullName")]
    pub github_repo_full_name: Option<String>,
    #[serde(rename = "agentConfigScanEnabled")]
    pub agent_config_scan_enabled: bool,
    pub status: String,
    #[serde(rename = "lastSignalHash")]
    pub last_signal_hash: Option<String>,
    #[serde(rename = "lastSyncedAt")]
    pub last_synced_at: Option<String>,
}

#[derive(Deserialize)]
struct ListResponse {
    projects: Vec<ConnectedProjectRow>,
}

#[derive(Deserialize)]
struct CreateResponse {
    project: ConnectedProjectRow,
}

async fn backend_error(resp: reqwest::Response) -> String {
    #[derive(Deserialize)]
    struct ErrorBody {
        error: String,
    }
    let status = resp.status();
    match resp.json::<ErrorBody>().await {
        Ok(body) => body.error,
        Err(_) => format!("backend returned {status}"),
    }
}

async fn rest_list_projects(token: &str, profile_id: &str) -> Result<Vec<ConnectedProjectRow>, String> {
    let base = crate::auth::backend_client::default_backend_url();
    let resp = reqwest::Client::new()
        .get(format!("{base}/api/profiles/{profile_id}/connected-projects"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(backend_error(resp).await);
    }
    Ok(resp.json::<ListResponse>().await.map_err(|e| e.to_string())?.projects)
}

// ── Command payloads ─────────────────────────────────────────────────────────

/// One row for the Connected-Projects screen: the server record joined with
/// this machine's grant (real path + local scan time), when one exists.
#[derive(Serialize)]
pub struct ConnectedProjectView {
    #[serde(flatten)]
    pub row: ConnectedProjectRow,
    pub local_path: Option<String>,
    pub last_scan_at_ms: Option<i64>,
    pub stale_on_open: bool,
}

fn join_views(rows: Vec<ConnectedProjectRow>, grants: &[LocalProjectGrant], now: i64) -> Vec<ConnectedProjectView> {
    rows.into_iter()
        .map(|row| {
            let grant = grants.iter().find(|g| g.connected_project_id == row.id);
            ConnectedProjectView {
                stale_on_open: grant
                    .map(|g| grants::is_stale(g, now, grants::ON_OPEN_STALE_MS))
                    .unwrap_or(false),
                local_path: grant.map(|g| g.path.clone()),
                last_scan_at_ms: grant.and_then(|g| g.last_scan_at_ms),
                row,
            }
        })
        .collect()
}

// ── Tauri commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn pick_project_folder() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new().pick_folder().await;
    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn list_connected_projects(
    signin: tauri::State<'_, SigninState>,
    grants_state: tauri::State<'_, GrantsState>,
    profile_id: String,
) -> Result<Vec<ConnectedProjectView>, String> {
    let token = require_access_token(&signin)?;
    let rows = rest_list_projects(&token, &profile_id).await?;
    let grants = grants_state.list()?;
    Ok(join_views(rows, &grants, now_ms()))
}

/// Connect a local folder: creates the server-side ConnectedProject (display
/// label + explicit consent, NEVER the path) then records the real path in
/// the local grant store.
#[tauri::command]
pub async fn connect_local_project(
    signin: tauri::State<'_, SigninState>,
    grants_state: tauri::State<'_, GrantsState>,
    profile_id: String,
    folder_path: String,
    label: String,
    consent_confirmed: bool,
    agent_config_scan_enabled: bool,
) -> Result<ConnectedProjectRow, String> {
    if !consent_confirmed {
        // Requirement 1.3 — defence in depth behind the UI checkbox.
        return Err("per-project consent must be affirmed before connecting".to_string());
    }
    if !std::path::Path::new(&folder_path).is_dir() {
        return Err(format!("not a directory: {folder_path}"));
    }
    let token = require_access_token(&signin)?;
    let base = crate::auth::backend_client::default_backend_url();
    let resp = reqwest::Client::new()
        .post(format!("{base}/api/profiles/{profile_id}/connected-projects"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "sourceType": "local",
            "localPathLabel": label,
            "consentConfirmed": true,
            "agentConfigScanEnabled": agent_config_scan_enabled,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(backend_error(resp).await);
    }
    let row = resp.json::<CreateResponse>().await.map_err(|e| e.to_string())?.project;

    grants_state.upsert(LocalProjectGrant {
        connected_project_id: row.id.clone(),
        path: folder_path,
        label,
        last_scan_at_ms: None,
    })?;
    Ok(row)
}

/// Server-side removal (immediate scan stop + audit-retained consent record,
/// Requirement 1.4) plus dropping the local grant so this machine forgets the
/// path too.
#[tauri::command]
pub async fn remove_connected_project(
    signin: tauri::State<'_, SigninState>,
    grants_state: tauri::State<'_, GrantsState>,
    profile_id: String,
    connected_project_id: String,
) -> Result<serde_json::Value, String> {
    let token = require_access_token(&signin)?;
    let base = crate::auth::backend_client::default_backend_url();
    let resp = reqwest::Client::new()
        .delete(format!(
            "{base}/api/profiles/{profile_id}/connected-projects/{connected_project_id}"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(backend_error(resp).await);
    }
    let result = resp.json::<serde_json::Value>().await.map_err(|e| e.to_string())?;
    grants_state.remove(&connected_project_id)?;
    Ok(result)
}

/// One scan pass for one grant: spawns the `project-sync` sidecar script
/// (shared @menporulalar/agents-core scanLocal → dedup vs the server-recorded
/// lastSignalHash → stage over MCP), then stamps last_scan_at on success.
#[tauri::command]
pub async fn run_project_sync(
    signin: tauri::State<'_, SigninState>,
    grants_state: tauri::State<'_, GrantsState>,
    connected_project_id: String,
    profile_id: String,
    last_signal_hash: Option<String>,
    agent_config_scan_enabled: bool,
) -> Result<serde_json::Value, String> {
    let token = require_access_token(&signin)?;
    let grant = grants_state
        .get(&connected_project_id)?
        .ok_or_else(|| format!("no local grant recorded for project {connected_project_id}"))?;

    let hash_arg = last_signal_hash.unwrap_or_else(|| "-".to_string());
    let agent_flag = if agent_config_scan_enabled { "true" } else { "false" };

    let result = sidecar::run_sidecar_command(
        "project-sync",
        &[&grant.path, &profile_id, &connected_project_id, &hash_arg, agent_flag],
        &token,
        crate::auth::backend_client::default_backend_url(),
        None,
        &[],
    )
    .await?;

    grants_state.touch(&connected_project_id, now_ms())?;
    Ok(result)
}

/// The scheduler's only query: grants due under the given threshold
/// ("on_open" = 24h, "weekly" = 7d) — spec task 3.8's cadence, decided by the
/// unit-tested `is_stale`, not UI arithmetic.
#[tauri::command]
pub fn list_stale_project_grants(
    grants_state: tauri::State<'_, GrantsState>,
    threshold: String,
) -> Result<Vec<LocalProjectGrant>, String> {
    let threshold_ms = match threshold.as_str() {
        "weekly" => grants::WEEKLY_STALE_MS,
        _ => grants::ON_OPEN_STALE_MS,
    };
    let now = now_ms();
    Ok(grants_state
        .list()?
        .into_iter()
        .filter(|g| grants::is_stale(g, now, threshold_ms))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str) -> ConnectedProjectRow {
        ConnectedProjectRow {
            id: id.into(),
            source_type: "local".into(),
            local_path_label: Some("x".into()),
            github_repo_full_name: None,
            agent_config_scan_enabled: false,
            status: "active".into(),
            last_signal_hash: None,
            last_synced_at: None,
        }
    }

    #[test]
    fn join_views_marks_staleness_and_paths() {
        let grants = vec![LocalProjectGrant {
            connected_project_id: "a".into(),
            path: "/real/path".into(),
            label: "x".into(),
            last_scan_at_ms: Some(0),
        }];
        let views = join_views(vec![row("a"), row("b")], &grants, grants::ON_OPEN_STALE_MS + 1);
        assert_eq!(views[0].local_path.as_deref(), Some("/real/path"));
        assert!(views[0].stale_on_open);
        // A row with no grant on this machine (e.g. a GitHub source) is never locally stale.
        assert_eq!(views[1].local_path, None);
        assert!(!views[1].stale_on_open);
    }
}
