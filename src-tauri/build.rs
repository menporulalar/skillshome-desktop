use std::fs;
use std::path::Path;

/// Reads `src-tauri/.env` (gitignored, dev-machine-local) and re-exposes each
/// `KEY=VALUE` line as a `cargo:rustc-env` build script instruction, so
/// `option_env!("GOOGLE_DESKTOP_CLIENT_ID")` in `auth/google.rs` resolves to a real
/// value without anyone having to `export` it by hand in every terminal session.
/// Deliberately hand-rolled (no `dotenvy` dependency) — this is one KEY=VALUE line,
/// not worth a crate.
fn load_dotenv_into_rustc_env() {
    let env_path = Path::new(".env");
    println!("cargo:rerun-if-changed=.env");

    let Ok(contents) = fs::read_to_string(env_path) else {
        return; // no .env file — fine, option_env! just falls back to its default
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        println!("cargo:rustc-env={}={}", key.trim(), value.trim());
    }
}

/// `tauri.conf.json`'s `bundle.resources` (release sidecar packaging) points at
/// `resources/sidecar/node` + `resources/sidecar/dist/*.cjs` — `tauri_build::build()`
/// validates those paths exist on *every* `cargo build`/`cargo check`, not just
/// real `tauri build` packaging. CI populates them for real before building (see
/// release.yml), but a fresh dev checkout has neither — write empty placeholders
/// if missing so `cargo tauri dev` keeps working with zero setup (dev mode's own
/// sidecar spawn path never reads these; see `ingest/sidecar.rs`).
fn ensure_sidecar_resource_placeholders() {
    let dist_dir = Path::new("resources/sidecar/dist");
    fs::create_dir_all(dist_dir).expect("failed to create resources/sidecar/dist placeholder dir");

    let node_placeholder = Path::new("resources/sidecar/node");
    if !node_placeholder.exists() {
        fs::write(node_placeholder, "").expect("failed to write resources/sidecar/node placeholder");
    }

    for script in ["stage", "confirm", "project-sync", "interview-opening", "interview-turn"] {
        let path = dist_dir.join(format!("{script}.cjs"));
        if !path.exists() {
            fs::write(&path, "").expect("failed to write sidecar dist placeholder");
        }
    }
}

fn main() {
    load_dotenv_into_rustc_env();
    ensure_sidecar_resource_placeholders();
    tauri_build::build()
}
