//! Local_Project_Grant persistence — `project_grants.json` under the app data
//! directory. The only place the real folder path is ever stored.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LocalProjectGrant {
    pub connected_project_id: String,
    /// Real filesystem path — never leaves this machine.
    pub path: String,
    /// Mirror of the server-side display label, for the list UI when offline.
    pub label: String,
    /// Last local scan (ms epoch) — drives the weekly/on-open staleness check.
    pub last_scan_at_ms: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
struct GrantsFile {
    grants: Vec<LocalProjectGrant>,
}

pub fn grants_path(root: &Path) -> PathBuf {
    root.join("project_grants.json")
}

fn load(root: &Path) -> Result<GrantsFile, String> {
    let path = grants_path(root);
    if !path.exists() {
        return Ok(GrantsFile::default());
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

fn save(root: &Path, file: &GrantsFile) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let json = serde_json::to_vec_pretty(file).map_err(|e| e.to_string())?;
    std::fs::write(grants_path(root), json).map_err(|e| e.to_string())
}

/// Mutex-guarded store handle managed by Tauri (`app.manage`), mirroring
/// `ExtractionSettingsState`'s hand-wired pattern.
pub struct GrantsState {
    root: PathBuf,
    lock: Mutex<()>,
}

impl GrantsState {
    pub fn new(root: PathBuf) -> Self {
        Self { root, lock: Mutex::new(()) }
    }

    pub fn list(&self) -> Result<Vec<LocalProjectGrant>, String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        Ok(load(&self.root)?.grants)
    }

    pub fn get(&self, connected_project_id: &str) -> Result<Option<LocalProjectGrant>, String> {
        Ok(self
            .list()?
            .into_iter()
            .find(|g| g.connected_project_id == connected_project_id))
    }

    pub fn upsert(&self, grant: LocalProjectGrant) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let mut file = load(&self.root)?;
        file.grants.retain(|g| g.connected_project_id != grant.connected_project_id);
        file.grants.push(grant);
        save(&self.root, &file)
    }

    pub fn remove(&self, connected_project_id: &str) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let mut file = load(&self.root)?;
        file.grants.retain(|g| g.connected_project_id != connected_project_id);
        save(&self.root, &file)
    }

    pub fn touch(&self, connected_project_id: &str, at_ms: i64) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let mut file = load(&self.root)?;
        for g in file.grants.iter_mut() {
            if g.connected_project_id == connected_project_id {
                g.last_scan_at_ms = Some(at_ms);
            }
        }
        save(&self.root, &file)
    }
}

/// Staleness decision for the scheduler (weekly + on-open >24h) — pure, so the
/// thresholds are unit-tested here rather than in UI code.
pub const ON_OPEN_STALE_MS: i64 = 24 * 3600 * 1000;
pub const WEEKLY_STALE_MS: i64 = 7 * 24 * 3600 * 1000;

pub fn is_stale(grant: &LocalProjectGrant, now_ms: i64, threshold_ms: i64) -> bool {
    match grant.last_scan_at_ms {
        None => true,
        Some(last) => now_ms.saturating_sub(last) >= threshold_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pss-grants-{}", uuid_ish()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn uuid_ish() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        format!("{}-{:?}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), std::thread::current().id())
    }

    fn grant(id: &str) -> LocalProjectGrant {
        LocalProjectGrant {
            connected_project_id: id.into(),
            path: "/tmp/x".into(),
            label: "x".into(),
            last_scan_at_ms: None,
        }
    }

    #[test]
    fn upsert_list_remove_round_trip() {
        let root = temp_root();
        let state = GrantsState::new(root.clone());
        state.upsert(grant("a")).unwrap();
        state.upsert(grant("b")).unwrap();
        assert_eq!(state.list().unwrap().len(), 2);

        // Upsert replaces, never duplicates.
        state.upsert(grant("a")).unwrap();
        assert_eq!(state.list().unwrap().len(), 2);

        state.remove("a").unwrap();
        assert_eq!(state.list().unwrap().len(), 1);
        assert_eq!(state.get("b").unwrap().unwrap().connected_project_id, "b");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn touch_stamps_last_scan_time() {
        let root = temp_root();
        let state = GrantsState::new(root.clone());
        state.upsert(grant("a")).unwrap();
        state.touch("a", 1234).unwrap();
        assert_eq!(state.get("a").unwrap().unwrap().last_scan_at_ms, Some(1234));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn staleness_thresholds() {
        let mut g = grant("a");
        assert!(is_stale(&g, 0, ON_OPEN_STALE_MS), "never-scanned is always stale");
        g.last_scan_at_ms = Some(1000);
        assert!(!is_stale(&g, 1000 + ON_OPEN_STALE_MS - 1, ON_OPEN_STALE_MS));
        assert!(is_stale(&g, 1000 + ON_OPEN_STALE_MS, ON_OPEN_STALE_MS));
        assert!(is_stale(&g, 1000 + WEEKLY_STALE_MS, WEEKLY_STALE_MS));
    }

    #[test]
    fn grants_file_survives_reload() {
        let root = temp_root();
        {
            let state = GrantsState::new(root.clone());
            state.upsert(grant("persisted")).unwrap();
        }
        let fresh = GrantsState::new(root.clone());
        assert_eq!(fresh.get("persisted").unwrap().unwrap().label, "x");
        std::fs::remove_dir_all(root).ok();
    }
}
