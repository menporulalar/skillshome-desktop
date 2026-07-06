//! Persisted, non-sensitive Extraction_Source settings — a plain JSON file under the
//! app's data directory, not `tauri-plugin-store` (this project hand-wires small pieces
//! of persistence rather than reaching for a heavier plugin — see `auth/token_store.rs`).
//!
//! The BYOK API key itself is never stored here — it lives only in the OS keychain via
//! `auth::token_store`'s `save_byok_api_key`/`load_byok_api_key`.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionSource {
    #[default]
    ServerFallback,
    LocalModel,
    ByokFrontier,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct LocalModelConfig {
    pub endpoint: String,
    pub model: String,
    /// Requirement 10.2 — explicit opt-in required for a non-loopback endpoint.
    pub non_loopback_opt_in: bool,
    /// Display-only ("last check: passed/failed") — never trusted as an activation
    /// gate. `activate_extraction_source` always re-runs the real check itself.
    pub connectivity_check_passed: bool,
}

// Explicit renames, not `rename_all = "snake_case"` — serde's default conversion
// turns `OpenAI` into the awkward `"open_a_i"` (each capital gets its own preceding
// underscore, including within the acronym). Explicit names keep the wire format
// (and the persisted JSON file) readable.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum ByokProvider {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openrouter")]
    OpenRouter,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ByokFrontierConfig {
    pub provider: ByokProvider,
    pub model: String,
    pub connectivity_check_passed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ExtractionSettings {
    pub active_source: ExtractionSource,
    pub local_model: Option<LocalModelConfig>,
    pub byok_frontier: Option<ByokFrontierConfig>,
}

const SETTINGS_FILE_NAME: &str = "extraction_settings.json";

pub fn settings_path(root: &Path) -> std::path::PathBuf {
    root.join(SETTINGS_FILE_NAME)
}

/// Missing file → `ExtractionSettings::default()`, which is `ServerFallback` with no
/// configured sources — satisfies Requirement 3.8 (default to Server_Fallback when
/// neither Local_Model nor BYOK_Frontier is configured) with no special-casing.
pub fn load(root: &Path) -> Result<ExtractionSettings, String> {
    let path = settings_path(root);
    if !path.exists() {
        return Ok(ExtractionSettings::default());
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

pub fn save(root: &Path, settings: &ExtractionSettings) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let path = settings_path(root);
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

/// Tauri-managed state — loaded once at startup (mirrors `SigninState`'s existing
/// precedent) and written through to disk on every mutating command, so
/// `get_extraction_settings` never needs to touch disk on the read path.
#[derive(Default)]
pub struct ExtractionSettingsState(pub Mutex<ExtractionSettings>);

impl ExtractionSettingsState {
    pub fn get(&self) -> ExtractionSettings {
        self.0.lock().unwrap().clone()
    }

    pub fn set(&self, settings: ExtractionSettings) {
        *self.0.lock().unwrap() = settings;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("skillshome-desktop-test-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn missing_file_loads_as_default() {
        let root = temp_root("missing-file");
        assert_eq!(load(&root).unwrap(), ExtractionSettings::default());
    }

    /// Locks in the explicit wire-format names — regression guard against
    /// accidentally reintroducing `#[serde(rename_all = "snake_case")]`, which turns
    /// `OpenAI` into the unreadable `"open_a_i"` (each capital in the acronym gets
    /// its own preceding underscore).
    #[test]
    fn byok_provider_serializes_to_explicit_readable_names() {
        assert_eq!(serde_json::to_string(&ByokProvider::OpenAI).unwrap(), "\"openai\"");
        assert_eq!(serde_json::to_string(&ByokProvider::Anthropic).unwrap(), "\"anthropic\"");
        assert_eq!(serde_json::to_string(&ByokProvider::OpenRouter).unwrap(), "\"openrouter\"");
    }

    #[test]
    fn save_load_round_trip() {
        let root = temp_root("round-trip");
        let settings = ExtractionSettings {
            active_source: ExtractionSource::LocalModel,
            local_model: Some(LocalModelConfig {
                endpoint: "http://127.0.0.1:11434".to_string(),
                model: "llama3.2:3b".to_string(),
                non_loopback_opt_in: false,
                connectivity_check_passed: true,
            }),
            byok_frontier: None,
        };

        save(&root, &settings).unwrap();
        assert_eq!(load(&root).unwrap(), settings);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn byok_frontier_config_round_trips_without_storing_a_key_field() {
        let root = temp_root("byok-round-trip");
        let settings = ExtractionSettings {
            active_source: ExtractionSource::ByokFrontier,
            local_model: None,
            byok_frontier: Some(ByokFrontierConfig {
                provider: ByokProvider::Anthropic,
                model: "claude-3-7-sonnet-latest".to_string(),
                connectivity_check_passed: false,
            }),
        };

        save(&root, &settings).unwrap();
        let raw = std::fs::read_to_string(settings_path(&root)).unwrap();
        assert!(!raw.contains("api_key"), "settings file must never contain an api_key field");
        assert_eq!(load(&root).unwrap(), settings);

        let _ = std::fs::remove_dir_all(&root);
    }
}
