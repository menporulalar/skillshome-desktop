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
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

#[derive(serde::Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub body: Option<String>,
}

/// R5 — emitted on `DOWNLOAD_PROGRESS_EVENT` for every chunk `download_and_install`
/// reports. `total` is `None` when the server response has no `Content-Length`, which
/// the frontend renders as an indeterminate bar rather than a percentage.
#[derive(Clone, serde::Serialize)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

const DOWNLOAD_PROGRESS_EVENT: &str = "updater://download-progress";

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
    /// Long-running Rust commands (sidecar extraction runs, project scans, interview
    /// turns) hold an `OpGuard` for their whole duration. Counted, not boolean, so
    /// concurrent operations can't clear each other's protection on completion.
    active_ops: AtomicUsize,
    /// Screen-level state only the webview knows: an extraction flow in progress,
    /// an unsaved review-and-confirm screen (both named explicitly in the PRD), or
    /// an open mock interview session.
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
                "{ops} background operation{} still running (extraction, project scan, or interview turn)",
                if ops == 1 { " is" } else { "s are" }
            ));
        }
        if self.0.ui_busy.load(Ordering::SeqCst) {
            return Some("an extraction, review, or mock interview is open and unfinished".to_string());
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

    interpret_check(updater.check().await, current_version)
}

/// Maps a raw `Updater::check()` result onto `UpdateInfo`. Split out from
/// `check_for_updates` purely so `manifest_tests` below can drive it against an
/// `Updater` built with a mocked endpoint instead of `app`'s real, production one —
/// `Updater`/`Update` have no public constructor, so a real (if pointed-elsewhere)
/// `Updater::check()` call is the only way to get one at all.
fn interpret_check(
    result: tauri_plugin_updater::Result<Option<tauri_plugin_updater::Update>>,
    current_version: String,
) -> Result<UpdateInfo, String> {
    match result {
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

    // R5: cumulative bytes downloaded so far, emitted alongside the (possibly
    // unknown) total per chunk so the frontend can render a determinate or
    // indeterminate bar. `app` is cloned into the closure since `download_and_install`
    // requires `FnMut`, not a borrow tied to this stack frame.
    let mut downloaded: u64 = 0;
    let progress_app = app.clone();
    update
        .download_and_install(
            move |chunk_len, total| {
                downloaded += chunk_len as u64;
                // Best-effort: a missed event just means the bar doesn't tick for one
                // chunk. The download/install itself does not depend on this.
                let _ = progress_app.emit(
                    DOWNLOAD_PROGRESS_EVENT,
                    DownloadProgress {
                        downloaded,
                        total,
                    },
                );
            },
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

/// Closes the Phase 1 DoD gap flagged in `tasks.md`: "update detection against a
/// mocked manifest". Drives the real `tauri-plugin-updater` `Updater::check()` against
/// a local HTTP stub, through `interpret_check` — the same mapping `check_for_updates`
/// uses in production.
///
/// Needs its own fixture config (`tests/fixtures/updater-test/tauri.conf.json`) rather
/// than the app's real `tauri.conf.json`: the plugin only accepts a plaintext `http://`
/// loopback endpoint when `dangerousInsecureTransportProtocol` is set, and the real
/// config rightly does not set it. Nothing about the fixture's pubkey/endpoints values
/// matters — both are overridden per-call below — only the insecure-transport flag.
#[cfg(test)]
mod manifest_tests {
    use super::*;
    use tauri_plugin_updater::UpdaterExt;

    /// Serves `body` once on an ephemeral loopback port. Mirrors the stub-server
    /// pattern already used by `auth::silent_refresh`'s tests.
    fn manifest_server(body: String) -> String {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind ephemeral port");
        let addr = server.server_addr().to_ip().expect("ip address");
        std::thread::spawn(move || {
            if let Ok(request) = server.recv() {
                let header =
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                        .unwrap();
                let response = tiny_http::Response::from_string(body).with_header(header);
                let _ = request.respond(response);
            }
        });
        format!("http://{addr}/latest.json")
    }

    fn mock_updater_app() -> tauri::App<tauri::test::MockRuntime> {
        tauri::test::mock_builder()
            .plugin(
                tauri_plugin_updater::Builder::new()
                    .target("test-target")
                    .build(),
            )
            .build(tauri::generate_context!(
                "tests/fixtures/updater-test/tauri.conf.json"
            ))
            .expect("mock app with updater plugin should build")
    }

    fn manifest_json(version: &str) -> String {
        serde_json::json!({
            "version": version,
            "notes": "test release notes",
            "pub_date": null,
            "platforms": {
                "test-target": { "url": "http://127.0.0.1:1/unused", "signature": "" }
            }
        })
        .to_string()
    }

    /// Builds an `Updater` pointed at `endpoint`, bypassing the fixture config's
    /// (empty) `endpoints`/`pubkey` — matches how `check_for_updates` builds its own
    /// `Updater` except for this one override.
    fn updater_against(app: &tauri::App<tauri::test::MockRuntime>, endpoint: &str) -> tauri_plugin_updater::Updater {
        app.handle()
            .updater_builder()
            .endpoints(vec![endpoint.parse().unwrap()])
            .expect("loopback http endpoint should validate under dangerousInsecureTransportProtocol")
            .build()
            .expect("updater should build")
    }

    #[tokio::test]
    async fn reports_available_when_manifest_version_is_newer() {
        let endpoint = manifest_server(manifest_json("9.9.9"));
        let app = mock_updater_app();
        let updater = updater_against(&app, &endpoint);

        let info = interpret_check(updater.check().await, "0.1.0".to_string())
            .expect("check should succeed against a well-formed manifest");

        assert!(info.available);
        assert_eq!(info.latest_version.as_deref(), Some("9.9.9"));
        assert_eq!(info.current_version, "0.1.0");
        assert_eq!(info.body.as_deref(), Some("test release notes"));
    }

    #[tokio::test]
    async fn reports_unavailable_when_manifest_version_is_not_newer() {
        // Same version as the fixture app's own `current_version` ("0.1.0", set in
        // tests/fixtures/updater-test/tauri.conf.json) — must not be offered as an update.
        let endpoint = manifest_server(manifest_json("0.1.0"));
        let app = mock_updater_app();
        let updater = updater_against(&app, &endpoint);

        let info = interpret_check(updater.check().await, "0.1.0".to_string())
            .expect("check should succeed even when there is nothing newer");

        assert!(!info.available);
        assert_eq!(info.latest_version, None);
    }

    #[tokio::test]
    async fn surfaces_an_error_for_a_malformed_manifest() {
        // Exactly the "malformed manifest" case useUpdateCheck.ts's checkError path
        // exists for — must come back as Err, never panic or silently report no update.
        let endpoint = manifest_server("not json".to_string());
        let app = mock_updater_app();
        let updater = updater_against(&app, &endpoint);

        let result = interpret_check(updater.check().await, "0.1.0".to_string());

        assert!(result.is_err());
    }
}

/// Closes the other half of the Phase 1 DoD gap: "signature verification
/// (valid/invalid/tampered)". `verify_signature` inside tauri-plugin-updater
/// (`updater.rs`) is private, so it can't be called directly — this instead drives the
/// exact same `minisign_verify::{PublicKey, Signature}` calls it makes, against a
/// keypair and payload generated here with the sibling `minisign` crate (same author,
/// interoperable wire format; `minisign-verify` only verifies, it can't sign).
/// A pass here is a direct guarantee about the primitive the plugin depends on, not a
/// reimplementation of it.
#[cfg(test)]
mod signature_tests {
    use base64::Engine;

    struct SignedFixture {
        pubkey_b64: String,
        signature_b64: String,
        payload: Vec<u8>,
    }

    /// Signs `payload` with a freshly generated keypair and returns the same two
    /// base64 blobs `update.rs`'s doc comment says tauri.conf.json's `pubkey` and the
    /// manifest's `signature` field hold: base64 of the whole `.pub`/`.sig` file text,
    /// matching `verify_signature`'s `base64_to_string` -> `PublicKey/Signature::decode`
    /// chain in tauri-plugin-updater.
    fn sign_fixture(payload: &[u8]) -> SignedFixture {
        let keypair =
            minisign::KeyPair::generate_unencrypted_keypair().expect("keypair generation");
        let pubkey_text = keypair.pk.to_box().expect("public key box").to_string();
        let sig_box = minisign::sign(None, &keypair.sk, payload, Some("test"), Some("test"))
            .expect("signing should succeed");

        SignedFixture {
            pubkey_b64: base64::engine::general_purpose::STANDARD.encode(pubkey_text),
            signature_b64: base64::engine::general_purpose::STANDARD.encode(sig_box.to_string()),
            payload: payload.to_vec(),
        }
    }

    /// Decodes the outer base64 layer to the plain multi-line `.pub`/`.sig` file text —
    /// the same step `verify_signature`'s `base64_to_string` performs.
    fn decode_outer(b64: &str) -> String {
        String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .expect("valid base64"),
        )
        .expect("valid utf8")
    }

    fn verify(
        pubkey_b64: &str,
        signature_b64: &str,
        data: &[u8],
    ) -> std::result::Result<(), minisign_verify::Error> {
        let public_key = minisign_verify::PublicKey::decode(&decode_outer(pubkey_b64))
            .expect("valid pubkey encoding");
        let signature = minisign_verify::Signature::decode(&decode_outer(signature_b64))
            .expect("valid signature encoding");
        public_key.verify(data, &signature, true)
    }

    #[test]
    fn valid_signature_verifies() {
        let fixture = sign_fixture(b"skillshome-desktop-update-package-bytes");
        assert!(verify(&fixture.pubkey_b64, &fixture.signature_b64, &fixture.payload).is_ok());
    }

    #[test]
    fn tampered_payload_byte_is_rejected() {
        let fixture = sign_fixture(b"skillshome-desktop-update-package-bytes");
        let mut tampered = fixture.payload.clone();
        tampered[0] ^= 0xFF;

        assert!(
            verify(&fixture.pubkey_b64, &fixture.signature_b64, &tampered).is_err(),
            "a single flipped payload byte must fail verification"
        );
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let fixture = sign_fixture(b"skillshome-desktop-update-package-bytes");
        let sig_text = decode_outer(&fixture.signature_b64);
        let lines: Vec<&str> = sig_text.lines().collect();
        assert_eq!(
            lines.len(),
            4,
            "minisign .sig text is 4 lines: untrusted comment, sig, trusted comment, global sig"
        );

        // Flip a byte inside the signature line's own base64 payload — specifically
        // within the 64-byte Ed25519 signature (bytes 10..74 of the decoded 74-byte
        // blob; the first 10 bytes are the algorithm tag + key id and must stay intact
        // or `Signature::decode` itself would reject the corrupted blob, which would
        // prove nothing about *verification* rejecting it). The re-encoded line stays
        // valid base64/UTF-8, so decode() still succeeds — only the signature is wrong.
        let mut sig_line_bytes = base64::engine::general_purpose::STANDARD
            .decode(lines[1])
            .expect("valid base64 sig line");
        assert_eq!(sig_line_bytes.len(), 74, "2-byte alg + 8-byte keynum + 64-byte sig");
        sig_line_bytes[40] ^= 0xFF;
        let corrupted_line = base64::engine::general_purpose::STANDARD.encode(sig_line_bytes);
        let corrupted_text = format!(
            "{}\n{}\n{}\n{}\n",
            lines[0], corrupted_line, lines[2], lines[3]
        );
        let corrupted_b64 = base64::engine::general_purpose::STANDARD.encode(corrupted_text);

        assert!(
            verify(&fixture.pubkey_b64, &corrupted_b64, &fixture.payload).is_err(),
            "a corrupted signature must fail verification even against the original payload"
        );
    }

    #[test]
    fn wrong_public_key_is_rejected() {
        let fixture = sign_fixture(b"skillshome-desktop-update-package-bytes");
        let other = sign_fixture(b"unrelated-payload-signed-by-a-different-key");

        assert!(
            verify(&other.pubkey_b64, &fixture.signature_b64, &fixture.payload).is_err(),
            "a signature made by a different keypair must be rejected"
        );
    }
}
