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

fn main() {
    load_dotenv_into_rustc_env();
    tauri_build::build()
}
