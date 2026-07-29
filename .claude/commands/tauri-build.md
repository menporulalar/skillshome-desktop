---
description: Build a local installer for the current platform
---

Build SkillsHome Desktop for local testing (not a release — no signing keys needed for this).

Steps:
1. Run `npm run tauri:build` in the project root.
2. This runs `tsc && vite build` (frontend) then compiles Rust in release mode and bundles an
   installer for the current OS only. Expect several minutes on first build (cold Rust cache).
3. Report the output artifact path (`src-tauri/target/release/bundle/...`) on success.
4. On failure, distinguish:
   - `tsc` errors → frontend type errors, fix in `src/`
   - `cargo` compile errors → Rust errors in `src-tauri/src/`
   - bundler errors (icon/config) → check `src-tauri/tauri.conf.json`

This does not require `TAURI_SIGNING_PRIVATE_KEY` — that's only consumed by the CI release
workflow (`.github/workflows/release.yml`) for updater-signed artifacts.
