# Verification — Feature #28: Desktop Auto-Update

**Note on this file's history:** an earlier task list (`feature-28-desktop-auto-update/tasks.md`
task 1.7 in skillshome-specs) claimed this doc was created on 2026-07-26. It wasn't — `docs/` has
no git history in this repo before this commit. Written for real 2026-07-27, covering what task
1.7 originally asked for (signing-key setup) plus the rollback/pin procedure (task 3.4).

## Signing key setup

- Keypair generated via `npx @tauri-apps/cli signer generate`, written to `src-tauri/.tauri-keys`
  (private) and `.tauri-keys.pub` (public). Both are gitignored — never commit either.
- Public key is embedded in `src-tauri/tauri.conf.json` under `plugins.updater.pubkey` (base64,
  read by the updater plugin at compile time). Active key id: `A9B2A1B724AF00E0`.
- Private key goes into the `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions secret as the **verbatim
  base64 contents** of `.tauri-keys` — do not re-encode it. `.tauri-keys` is already a base64 blob;
  running it through `base64` again produces a value that fails with the misleading error
  "incorrect updater private key password: Missing encoded key in secret key" (see the long
  comment in `release.yml` above the `TAURI_SIGNING_PRIVATE_KEY` line).
- If the keypair ever needs regenerating (e.g. a private key is exposed, as happened once during
  this feature's development — see `tasks.md` 1.3), rotation is zero-downtime: Tauri's updater
  accepts multiple trusted keys during a transition, so you can generate a new pair, update the
  `TAURI_SIGNING_PRIVATE_KEY` secret, and swap the embedded public key over 1–2 releases without a
  window where old and new clients can't verify anything.

## Rollback / pin procedure (task 3.4)

The updater endpoint is fixed: `releases/latest/download/latest.json` on GitHub Releases. GitHub
resolves "latest" to whichever published (non-draft) release is currently flagged as the latest —
by default the most recently created non-prerelease release, but a release can also be explicitly
marked or unmarked as "the latest release" from the release's **Edit** page without re-tagging or
rebuilding anything.

**Important asymmetry:** Tauri's updater only moves forward. `check()` compares the manifest's
semver against the installed version and reports "no update available" whenever the manifest
version is `<=` the installed one. Repointing `latest.json` at an older tag does **not** downgrade
anyone who has already installed the newer, broken version — it only stops the broken version from
reaching installs that haven't updated yet. Plan rollback around that split:

1. **Stop the bleed (not-yet-updated installs)** — on GitHub, open the bad release and either:
   - un-check "Set as the latest release" and instead mark the last known-good release as latest, or
   - mark the bad release as a pre-release / edit it back to draft.

   Either action makes `releases/latest/download/latest.json` resolve to the good release's assets
   again. Anyone who checks for updates after this point sees no update (if they're on the good
   version) or is offered the good version again (if they're somehow behind it) — they are never
   offered the bad one.

2. **Recover already-updated installs** — since downgrade isn't how the updater works, the fix is a
   **new release with a higher version number** that reverts or patches the regression (e.g. the
   bad release was `0.1.5`; ship `0.1.6` with the revert). Bump `package.json`, `Cargo.toml`, and
   `tauri.conf.json`'s `version` together (all three must match — `tauri.conf.json`'s is what
   `update.rs` reads as `current_version`), tag it, let `release.yml` build and sign it as usual,
   inspect the draft's `latest.json`, then promote. Installs already on the bad version will pick
   this up on their next check like any other update.

3. **There is no in-place "undo the install"** — nothing in this feature downloads or reverts a
   previously-installed binary automatically. If (1) and (2) are both too slow for an active
   incident, the only faster lever is telling affected users to reinstall from a specific prior
   release asset directly (bypassing the updater entirely), which is a manual, user-facing ask —
   not something `update.rs` or the CI workflow does today.

4. **Verify before promoting, every time** — the same "inspect `latest.json` on the draft before
   promoting" step that gates every normal release (see `release.yml`'s `releaseDraft: true`
   comment) applies doubly to a rollback release: confirm the version number, confirm the attached
   `.sig` files are present, and confirm the body text isn't left over from the release you're
   trying to undo.
