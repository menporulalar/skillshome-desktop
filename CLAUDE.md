# SkillsHome Desktop — Project Rules & Architecture Reference

Tauri v2 (Rust) + React 19/Vite desktop app. Talks to [skillshome-app](../skillshome-app)
over MCP + REST — never re-implement server-side logic here, call it.

## Stack map

| Layer | Where | Notes |
|---|---|---|
| Frontend | `src/` | React 19, Vite, no test runner configured yet |
| Rust backend | `src-tauri/src/` | modules: `auth/`, `ingest/`, `interview/`, `extraction/`, `projectsync/`, `update.rs` |
| Sidecar | `sidecar/` | Node/ts-node process Rust spawns directly for local extraction + MCP client |
| IPC surface | `src-tauri/capabilities/default.json` | every new Tauri command's permission must be added here |
| Release | `.github/workflows/release.yml` | tag `v*` → builds macOS arm64/x86_64, Windows, Linux |

## Brand colors

`brand/` is a submodule (shared with skillshome-app and skillshome-marketing) and
`brand/theme/tokens.ts` is the only place a brand hex is written. `src/App.css` imports the
generated `brand/theme/tokens.css` for `var(--brand-*)`; TSX imports `LIGHT_THEME` /
`DARK_THEME` from `../../brand/theme/tokens` and `withAlpha` from `../../brand/theme/color`.
Never re-type a hex. The frontend build now fails without the submodule, so `release.yml`
checks out `brand` explicitly; `npm run brand:check` verifies the submodule's generated
outputs still match its tokens.

### App icon (`src-tauri/icons/`)

The whole icon set (dock/taskbar/installer PNGs, `icon.icns`, `icon.ico`, Windows
`Square*Logo`/`StoreLogo`, Android mipmaps, iOS `AppIcon-*`) is generated — never hand-edit
a file in `src-tauri/icons/`. It is NOT covered by `brand:check`, so it has drifted from the
brand before (regenerated 2026-07-27 in the old purple brand, missed by the blue and brass
token bumps).

To regenerate after a brand change, with the `brand` submodule checked out:

```
npm run brand:icon
```

That runs `scripts/generate-app-icon.mjs` (renders the 1024×1024
`src-tauri/icons/icon.png` from `brand/logo/assets/skillshome-icon.svg` via `sharp`) then
`tauri icon` to fan it out to every platform size. Commit the full `src-tauri/icons/` diff.

## Security

**Token storage** — access token lives in-memory only (`SigninState` mutex in Rust). The
refresh token and BYOK API key go in the OS keychain via `src-tauri/src/auth/token_store.rs`
(`keyring`/`keyring-core`, one entry per platform: Keychain / Credential Manager / Secret
Service). Never write either to disk, `localStorage`, or a plain config file.

**Signing key** — `src-tauri/.tauri-keys` (private) and `.tauri-keys.pub` are gitignored;
never read, print, or commit their contents. The CI secret `TAURI_SIGNING_PRIVATE_KEY` is
the verbatim base64 contents of `.tauri-keys` — do not re-encode it (see the long comment in
`release.yml` about the double-encoding bug).

**Update signature verification** happens inside `tauri-plugin-updater` against the `pubkey`
in `tauri.conf.json` — there is no code path in `update.rs` that can skip or weaken it.

**Update guard (`UpdateGuard` in `update.rs`)** — restart-to-apply is blocked while
`active_ops` (extraction/project-scan) is nonzero or `ui_busy` (unconfirmed review screen) is
set, and re-checked immediately before the actual restart. Long-running Rust commands must
hold an `OpGuard` (`guard.begin_op()`) for their full duration — dropping it early defeats the
protection.

## IPC / Tauri commands

Any new `#[tauri::command]` needs its permission added to `src-tauri/capabilities/default.json`
or it will silently fail from the frontend with a capability error, not a compile error.

## Sidecar contract

`stage` and `confirm` in `sidecar/package.json` are the stable script names Rust spawns
directly (task 4.12) — do not rename without updating the Rust caller. `extract:sample*`
scripts are for manual CLI use only. The sidecar talks MCP to `skillshome-app`;
`sidecar/src/mcpClient.ts`'s `parseMcpToolErrorReason` shape is duplicated (not imported) in
`src/errors/mapDesktopError.ts` because they run in different runtimes — a new server `reason`
value needs updating in both places.

## Error handling

All MCP/sidecar errors surfaced to the UI must go through `src/errors/mapDesktopError.ts` —
never show a raw MCP error string or stack trace to the user. Extend the existing
strip/parse/fallback chain there rather than adding ad hoc error handling at call sites.

## Release process

Version must match across three files before tagging: `package.json`, `src-tauri/Cargo.toml`,
`src-tauri/tauri.conf.json` (`tauri.conf.json`'s `version` is also what `update.rs` reads as
`current_version` — the updater compares against this one, not `CARGO_PKG_VERSION`). Releases
publish as **drafts** (`releaseDraft: true`) while the updater rollout is validated — inspect
the attached `latest.json`, then manually promote the release; nothing reaches existing
installs (the updater endpoint is `releases/latest/download/latest.json`) until it's promoted.

OS-level code signing (Apple notarization, Windows cert) is not set up yet — that's separate
from updater signing (which is configured). Don't conflate the two in commits/docs.

## Testing

`cargo test` (from `src-tauri/`) covers Rust unit tests — e.g. `update.rs`'s `UpdateGuard`
tests. `npm test` in `sidecar/` runs the ts-node test files under `sidecar/src/__tests__/`
directly (no Jest/Vitest runner). There is no frontend test runner configured — don't assume
one exists.

## Code quality

Minimal changes, no speculative abstractions, no comments explaining *what* code does (only
non-obvious *why* — this codebase's existing comments are almost all "why", follow that
pattern). Don't add backwards-compatibility shims for unused code — delete it.
