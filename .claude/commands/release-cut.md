---
description: Walk through cutting a new release (version bump + tag) — does not push without confirmation
---

Guide the user through cutting a new SkillsHome Desktop release. This touches shared/remote
state (a pushed tag triggers CI to build and draft-publish a GitHub Release) — confirm with the
user before the actual `git push --tags` / `git tag` step, per this project's normal
risky-action rules.

Steps:
1. Ask the user for the new version number (or infer the next patch/minor from the current
   `package.json` version and ask for confirmation).
2. Update the version in all three places — they must match:
   - `package.json` (`version` field)
   - `src-tauri/Cargo.toml` (`[package] version`)
   - `src-tauri/tauri.conf.json` (`version` field — this is the one the updater compares
     against at runtime, so it matters most)
3. Run `type-check` and `rust-test` (see those commands) before tagging — don't tag on a red
   build.
4. Show the user a diff of the three version bumps and ask for confirmation before committing.
5. Only after explicit confirmation: `git tag v<version>` and `git push origin v<version>`.
6. Remind the user: the release publishes as a **draft** — nothing reaches existing installs
   via the updater until they manually inspect `latest.json` and promote the release on GitHub.
