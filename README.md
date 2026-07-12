# SkillsHome Desktop

The candidate-facing desktop companion to [SkillsHome](https://www.skillshome.me) — a Tauri
(Rust + React) app that runs your profile extraction locally and syncs to your SkillsHome
account over MCP, instead of uploading your resume/files to a server.

## What it does

- Sign in with your existing SkillsHome account (Google or GitHub).
- Pick an `Extraction_Source`:
  - **Local_Model** — point it at a local Ollama (or other OpenAI-compatible) endpoint on your
    own machine. Nothing about your resume/profile content leaves your device.
  - **BYOK_Frontier** — use your own API key against a frontier model provider. The key is
    stored only in your OS's native credential store (Keychain / Credential Manager / Secret
    Service) — it's never sent to SkillsHome's servers.
  - **Server_Fallback** — if you'd rather not run anything locally, this routes through the
    same server-side extraction pipeline the web app uses.
- Extraction runs on your machine, you review the results, then confirm — only the confirmed,
  reviewed data syncs to your SkillsHome profile.

## Download

Grab the latest release for your platform from the
[Releases page](https://github.com/menporulalar/skillshome-desktop/releases) — macOS (Apple
Silicon and Intel), Windows, and Linux (`.deb` / AppImage) builds are published there.

### A note on the security warning you'll see

**These builds are currently unsigned.** Proper code signing (Apple notarization + a Windows
code-signing certificate) is a planned fast-follow, not yet set up. Your OS will warn you on
first launch — this is expected, not a sign of a bad download:

- **macOS**: Gatekeeper says the app "cannot be opened because it is from an unidentified
  developer." Right-click (or Control-click) the app → **Open** → **Open** again in the
  confirmation dialog. (You only need to do this once.)
- **Windows**: SmartScreen shows "Windows protected your PC." Click **More info** → **Run
  anyway**.

## Development

```bash
npm install
npm run tauri dev     # starts the Vite dev server + a Tauri window
```

Build a local installer for your current platform:

```bash
npm run tauri:build
```

Requires the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS (Rust
toolchain, plus platform-specific system dependencies — Xcode command-line tools on macOS,
WebView2 on Windows, `libwebkit2gtk`/`libappindicator3` on Linux).

## Releasing

Releases are built by `.github/workflows/release.yml` (macOS arm64 + x86_64, Windows, Linux) and
published to GitHub Releases automatically whenever a `v*` tag is pushed:

```bash
git tag v0.1.0
git push origin v0.1.0
```

## Related

- Main app / API: [skillshome-app](https://github.com/menporulalar/skillshome-app) (private) —
  this desktop app talks to it over MCP + REST.
- Sign-in and extraction-source plumbing are documented in that repo's
  `docs/architecture/desktop-mcp-development-conventions.md`.
