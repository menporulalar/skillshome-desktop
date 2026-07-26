// Feature #28 — R1 (updater plugin integration), R2 (signed releases),
// R3 (visible update state + user-controlled apply that never interrupts work).
//
// Signature verification is handled by tauri-plugin-updater itself: it checks the
// manifest/binary against the `plugins.updater.pubkey` embedded in tauri.conf.json.
// There is no code path here that can skip or weaken that check — an unsigned or
// tampered manifest fails inside `check()`/`download_and_install()` and surfaces as
// an `Err` to the caller.
//
// R3's "don't restart out from under an in-progress extraction" rule is enforced
// HERE, in Rust, not only in the UI — same defence-in-depth stance as
// `validate_local_model_endpoint`. The frontend disables the button; this refuses
// the call regardless of what the frontend believes.

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(serde::Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub body: Option<String>,
}

#[derive(serde::Serialize)]
pub struct UpdateGuardStatus {
    pub busy: bool,
    pub reason: Option<String>,
}

/// Outcome of an apply that did NOT restart. A successful restart diverges, so
/// any value returned to the caller means the restart was deliberately skipped.
#[derive(serde::Serialize)]
pub struct ApplyOutcome {
    /// The new version is installed on disk and takes effect on next launch.
    pub applied_on_next_launch: bool,
    pub message: String,
}

#[derive(Default)]
struct GuardInner {
    /// Long-running Rust commands (sidecar extraction runs, project scans) hold an
    /// `OpGuard` for their whole duration. Counted, not boolean, so concurrent
    /// operations can't clear each other's protection on completion.
    active_ops: AtomicUsize,
    /// Screen-level state only the webview knows: an extraction flow in progress,
    /// or an unsaved review-and-confirm screen (both named explicitly in the PRD).
    /// This one IS frontend-declared — no Rust-side signal exists for "the user is
    /// looking at an unconfirmed review package" — so it supplements, never
    /// replaces, `active_ops`.
    ui_busy: AtomicBool,
}

#[derive(Clone, Default)]
pub struct UpdateGuard(Arc<GuardInner>);

/// RAII: protection lasts exactly as long as the operation, including across
/// `.await` points and early `?` returns.
pub struct OpGuard(Arc<GuardInner>);

impl Drop for OpGuard {
    fn drop(&mut self) {
        self.0.active_ops.fetch_sub(1, Ordering::SeqCst);
    }
}

impl UpdateGuard {
    #[must_use = "the guard must be held for the operation's duration; dropping it immediately leaves the operation unprotected"]
    pub fn begin_op(&self) -> OpGuard {
        self.0.active_ops.fetch_add(1, Ordering::SeqCst);
        OpGuard(self.0.clone())
    }

    pub fn set_ui_busy(&self, busy: bool) {
        self.0.ui_busy.store(busy, Ordering::SeqCst);
    }

    pub fn busy_reason(&self) -> Option<String> {
        let ops = self.0.active_ops.load(Ordering::SeqCst);
        if ops > 0 {
            return Some(format!(
                "{ops} background operation{} still running (extraction or project scan)",
                if ops == 1 { " is" } else { "s are" }
            ));
        }
        if self.0.ui_busy.load(Ordering::SeqCst) {
            return Some("an extraction or review is open and unconfirmed".to_string());
        }
        None
    }

    pub fn status(&self) -> UpdateGuardStatus {
        let reason = self.busy_reason();
        UpdateGuardStatus {
            busy: reason.is_some(),
            reason,
        }
    }
}

/// Query the configured endpoint for a newer release. Read-only and deliberately
/// NOT guarded: checking never restarts anything, so it is safe to call on startup
/// or on a timer while an extraction or project sync is running.
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

/// Download, verify, install, and — if nothing is in flight — restart into the new
/// version.
///
/// Performs its own `check()` rather than trusting a version passed in from the
/// webview, so the binary installed is always one the updater just verified against
/// the embedded public key.
///
/// The busy state is checked TWICE: once before downloading, and again immediately
/// before restarting. The second check is the one that matters — a download takes
/// tens of seconds, and an extraction started during it would otherwise be killed by
/// a restart that was authorised when nothing was running. When that happens the
/// update stays installed and takes effect on next launch rather than being lost.
pub async fn apply_update(app: &AppHandle, guard: &UpdateGuard) -> Result<ApplyOutcome, String> {
    if let Some(reason) = guard.busy_reason() {
        return Err(format!(
            "Can't restart to update right now — {reason}. Finish or cancel it, then try again."
        ));
    }

    let updater = app.updater().map_err(|e| e.to_string())?;

    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(ApplyOutcome {
            applied_on_next_launch: false,
            message: "Already up to date.".to_string(),
        });
    };

    update
        .download_and_install(
            // Per-chunk progress. Wired to no-ops for now; surfacing a progress
            // bar is R5 (P1), and needs an event emit rather than a callback here.
            |_chunk_len, _content_len| {},
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    // Re-check: work may have started while the download was in flight.
    if let Some(reason) = guard.busy_reason() {
        return Ok(ApplyOutcome {
            applied_on_next_launch: true,
            message: format!(
                "Update installed, but not restarting — {reason}. It will take effect next time you open the app."
            ),
        });
    }

    // Diverges — nothing after this line runs.
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_guard_is_not_busy() {
        let g = UpdateGuard::default();
        assert!(g.busy_reason().is_none());
        assert!(!g.status().busy);
    }

    #[test]
    fn an_operation_blocks_and_releases_on_drop() {
        let g = UpdateGuard::default();
        {
            let _op = g.begin_op();
            assert!(g.busy_reason().is_some(), "op in flight must block");
        }
        assert!(g.busy_reason().is_none(), "drop must release");
    }

    #[test]
    fn concurrent_operations_do_not_clear_each_other() {
        // The bug a bool would have: two overlapping scans, the first finishes and
        // flips the flag to false while the second is still running.
        let g = UpdateGuard::default();
        let a = g.begin_op();
        let b = g.begin_op();
        drop(a);
        assert!(
            g.busy_reason().is_some(),
            "second operation must still block after the first ends"
        );
        drop(b);
        assert!(g.busy_reason().is_none());
    }

    #[test]
    fn ui_busy_blocks_independently_of_operations() {
        let g = UpdateGuard::default();
        g.set_ui_busy(true);
        assert!(g.busy_reason().is_some());
        g.set_ui_busy(false);
        assert!(g.busy_reason().is_none());
    }

    #[test]
    fn clearing_ui_busy_does_not_release_a_running_operation() {
        let g = UpdateGuard::default();
        let _op = g.begin_op();
        g.set_ui_busy(true);
        g.set_ui_busy(false);
        assert!(
            g.busy_reason().is_some(),
            "a frontend clearing its own flag must not unblock a live operation"
        );
    }

    #[test]
    fn reason_mentions_the_operation_count() {
        let g = UpdateGuard::default();
        let _a = g.begin_op();
        let _b = g.begin_op();
        let reason = g.busy_reason().expect("busy");
        assert!(reason.contains('2'), "reason should be specific: {reason}");
    }

    #[test]
    fn guard_clones_share_one_counter() {
        // State is handed out by clone in some call paths; a clone must not get a
        // private counter, or protection would silently not apply.
        let g = UpdateGuard::default();
        let clone = g.clone();
        let _op = clone.begin_op();
        assert!(g.busy_reason().is_some());
    }
}
