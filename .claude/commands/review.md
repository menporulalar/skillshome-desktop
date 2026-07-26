---
description: Run the desktop reviewer panel on a change/area → consolidate via review-coordinator for human approval
---

Run the SkillsHome Desktop review loop. Reviewers are **read-only** — they suggest; the human
approves; the main session implements.

Ask the user (if not already clear from context): "What should I review — a specific area, the
staged/unstaged diff, or the whole change?"

### Phase 1: Pick the reviewers

| If the change touches… | Run reviewer |
|---|---|
| `src-tauri/src/**`, `src-tauri/capabilities/*` (IPC, keyring, update guard) | `tauri-rust-reviewer` |
| `sidecar/src/**` (MCP client, local extraction, Rust↔sidecar contract) | `sidecar-mcp-reviewer` |
| version bumps, `tauri.conf.json` updater config, `.github/workflows/release.yml` | `release-updater-reviewer` |
| `src/**` (React/Vite frontend) general TS quality | `typescript-reviewer` (generic — not Tauri-aware, use for plain type-safety only) |

Default scope when unspecified: review the working diff
(`git -C /Users/janakiraman/Documents/Product/skillshome-desktop diff` + untracked) and pick
reviewers by the files it touches.

### Phase 2: Run reviewers

Launch each selected reviewer via the Task tool, pointing it at the specific files/area. Each
returns findings with severity, `file:line`, and a suggested fix.

### Phase 3: Consolidate

Hand all reviewer reports to the **`review-coordinator`** agent. It produces one prioritized,
de-duplicated table with a blank Decision column.

### Phase 4: Present for approval — STOP here

Show the consolidated table. **Do not implement anything.** Ask the user to mark each item
APPROVE / REJECT / DEFER. Only after approval does the main session implement the fix.
