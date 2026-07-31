//! ai-interview-agent Module 4 — the desktop half of Mock_Interview.
//!
//! Session creation and turn-answering both round-trip to the real backend:
//! `start_mock_interview` calls the same `POST /api/interview/start` REST
//! route every other client uses (there is no MCP tool for session creation —
//! Requirement 9's tool surface is exactly `interview.turn.propose`/
//! `interview.session.read`), and `submit_interview_turn` spawns the sidecar
//! for `interview.turn.propose` over MCP (Requirement 6.1: grading is always
//! server-side, regardless of Interview_Source).
//!
//! Interview_Source only ever changes *question text* (Requirement 5.2): when
//! Local_Model/BYOK_Frontier is active, this module spawns an extra sidecar
//! call to regenerate a conversational question's text locally, discarding
//! the server's own version. Bank-question slots are never regenerated.

use crate::auth::state::SigninState;
use crate::extraction::settings::{ExtractionSettingsState, ExtractionSource};
use crate::ingest::sidecar;
use serde::{Deserialize, Serialize};

fn require_access_token(state: &tauri::State<'_, SigninState>) -> Result<String, String> {
    state.access_token().ok_or_else(|| "not signed in".to_string())
}

// ── REST (session creation — no MCP tool for this exists) ────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoleTemplateSummary {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientQuestion {
    pub text: String,
    #[serde(rename = "trackName")]
    pub track_name: String,
    pub kind: String,
    pub options: Option<Vec<String>>,
    #[serde(rename = "perTurnSeconds")]
    pub per_turn_seconds: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StartInterviewResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "roleName")]
    pub role_name: String,
    pub question: ClientQuestion,
    #[serde(rename = "plannedTurns")]
    pub planned_turns: i64,
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

#[tauri::command]
pub async fn list_role_templates(signin: tauri::State<'_, SigninState>) -> Result<Vec<RoleTemplateSummary>, String> {
    let token = require_access_token(&signin)?;
    let base = crate::auth::backend_client::default_backend_url();
    let resp = reqwest::Client::new()
        .get(format!("{base}/api/role-templates"))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(backend_error(resp).await);
    }
    resp.json::<Vec<RoleTemplateSummary>>().await.map_err(|e| e.to_string())
}

/// Starts a session over REST, then — only when Interview_Source is
/// Local_Model/BYOK_Frontier and the opening slot is conversational — spawns
/// the sidecar to regenerate just the opening question's text locally,
/// splicing it into the response before it ever reaches the UI. The server's
/// own opening question is discarded in that case, never shown.
#[tauri::command]
pub async fn start_mock_interview(
    app: tauri::AppHandle,
    signin: tauri::State<'_, SigninState>,
    settings_state: tauri::State<'_, ExtractionSettingsState>,
    update_guard: tauri::State<'_, crate::update::UpdateGuard>,
    profile_id: String,
    role_template_id: String,
) -> Result<StartInterviewResponse, String> {
    // #28 R3: creates a session server-side and may spawn the sidecar for local
    // question generation. A restart between the POST and the response strands a
    // session the UI never learns the id of.
    let _op = update_guard.begin_op();
    let token = require_access_token(&signin)?;
    let base = crate::auth::backend_client::default_backend_url();
    let resp = reqwest::Client::new()
        .post(format!("{base}/api/interview/start"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "profileId": profile_id, "roleTemplateId": role_template_id }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(backend_error(resp).await);
    }
    let mut started = resp.json::<StartInterviewResponse>().await.map_err(|e| e.to_string())?;

    let settings = settings_state.get();
    if settings.active_interview_source != ExtractionSource::ServerFallback && started.question.kind == "conversational" {
        // Best-effort: if local generation fails (e.g. Ollama not running right
        // now), fall back to the server's own opening question rather than
        // failing session creation outright — the candidate can still start.
        let difficulty = 3; // ClientQuestion doesn't carry difficulty; a reasonable mid default for an opening question.
        if let Ok(result) = sidecar::run_sidecar_command(
            &app,
            "interview-opening",
            &[&started.question.track_name, &difficulty.to_string()],
            &token,
            base,
            None,
            &[],
        )
        .await
        {
            if let Some(text) = result.get("text").and_then(|v| v.as_str()) {
                started.question.text = text.to_string();
            }
        }
    }

    Ok(started)
}

// ── Sidecar (turn submission — always over MCP, per Requirement 9.2) ─────────
// Returned as raw serde_json::Value (mirrors projectsync::run_project_sync's
// precedent) — the sidecar's __SIDECAR_RESULT__ shape is already exactly what
// the frontend needs, no Rust-side re-typing required.

#[tauri::command]
pub async fn submit_interview_turn(
    app: tauri::AppHandle,
    signin: tauri::State<'_, SigninState>,
    update_guard: tauri::State<'_, crate::update::UpdateGuard>,
    session_id: String,
    answer_text: String,
) -> Result<serde_json::Value, String> {
    // #28 R3: a full sidecar spawn + MCP round-trip that grades and records the
    // turn server-side. Restarting mid-flight loses the answer the candidate
    // already submitted.
    let _op = update_guard.begin_op();
    let token = require_access_token(&signin)?;
    let base = crate::auth::backend_client::default_backend_url();
    sidecar::run_sidecar_command(&app, "interview-turn", &[&session_id, &answer_text], &token, base, None, &[]).await
}
