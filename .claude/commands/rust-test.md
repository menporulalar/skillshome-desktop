---
description: Run the Rust test suite (src-tauri)
---

Run Rust unit tests for the Tauri backend.

Steps:
1. Run `cargo test` in `src-tauri/`.
2. Report pass/fail count. On failure, show the failing test name, assertion, and file:line.
3. If tests reference `tauri::test::mock_app()` (e.g. `update.rs`'s `UpdateGuard` tests), note
   these only compile under the `dev-dependencies` `test` feature — a plain `cargo build`
   (non-test) will not pull that feature in, which is expected, not a bug.
