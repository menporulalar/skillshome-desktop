---
description: Start the Tauri dev app (Vite dev server + Rust backend + webview window)
---

Start SkillsHome Desktop in development mode.

Steps:
1. Check for an already-running dev instance: `ps aux | grep -i "skillshome_desktop\|tauri dev" | grep -v grep`. If one is running, ask before starting a second (a second instance can double-bind port 1420).
2. Run `npm run tauri dev` in the project root (backgrounded — this is long-running and opens a native window; don't wait on it synchronously).
3. Watch the first ~20 lines of output for:
   - Rust compile errors (fix before retrying)
   - `Error: address already in use` on port 1420 → another Vite instance is running, kill it first
   - Keyring init failures on Linux (`failed to initialize OS credential store`) → Secret Service isn't available in this environment; expected in some CI/headless sandboxes, not a bug.

Note: this opens a native window, which cannot be screenshotted the way a browser page can — verify behavior via `cargo test` / sidecar tests / reading logs, not by trying to view the window.
