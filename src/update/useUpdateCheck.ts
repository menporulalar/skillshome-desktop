import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Mirrors src-tauri/src/update.rs's `UpdateInfo` (serde default field names).
export interface UpdateInfo {
  available: boolean;
  current_version: string;
  latest_version: string | null;
  body: string | null;
}

// Mirrors `UpdateGuardStatus`.
export interface UpdateGuardStatus {
  busy: boolean;
  reason: string | null;
}

// Mirrors `ApplyOutcome`. A successful restart never returns, so receiving this at
// all means the restart was deliberately skipped.
export interface ApplyOutcome {
  applied_on_next_launch: boolean;
  message: string;
}

// Once a day is plenty for a desktop app the user may leave open for weeks; the
// check is a single HTTP GET of a small manifest.
const RECHECK_INTERVAL_MS = 24 * 3600 * 1000;
// The guard can flip while the banner is on screen (a scheduled project scan starts
// unattended), so the button's enabled state is re-read on a short timer rather
// than only at mount.
const GUARD_POLL_MS = 3000;

/**
 * #28 R3. Checks for an update in the background and exposes the state needed to
 * offer a user-triggered "Update & Restart".
 *
 * The check itself is deliberately unguarded and failure-tolerant: a missing
 * endpoint or offline machine must never surface as an error to someone who was
 * just trying to extract a résumé, so `checkError` is kept separate from the app's
 * error surfaces and the banner simply stays hidden.
 */
export function useUpdateCheck() {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [guard, setGuard] = useState<UpdateGuardStatus>({ busy: false, reason: null });
  const [applying, setApplying] = useState(false);
  const [applyMessage, setApplyMessage] = useState<string | null>(null);
  const [applyError, setApplyError] = useState<string | null>(null);
  const [checkError, setCheckError] = useState<string | null>(null);
  // StrictMode double-effects must not fire two checks on mount.
  const checkedRef = useRef(false);

  const check = useCallback(async () => {
    try {
      setInfo(await invoke<UpdateInfo>("check_for_updates"));
      setCheckError(null);
    } catch (err) {
      // Offline, endpoint unreachable, malformed manifest — all non-fatal.
      setCheckError(String(err));
    }
  }, []);

  const refreshGuard = useCallback(async () => {
    try {
      setGuard(await invoke<UpdateGuardStatus>("update_guard_status"));
    } catch {
      // If we can't tell, assume busy — refusing to restart is the safe default.
      setGuard({ busy: true, reason: "unable to confirm the app is idle" });
    }
  }, []);

  useEffect(() => {
    if (checkedRef.current) return;
    checkedRef.current = true;
    void check();
    const handle = setInterval(() => void check(), RECHECK_INTERVAL_MS);
    return () => clearInterval(handle);
  }, [check]);

  // Only poll the guard while an update is actually on offer — no point burning a
  // command round-trip every few seconds otherwise.
  useEffect(() => {
    if (!info?.available) return;
    void refreshGuard();
    const handle = setInterval(() => void refreshGuard(), GUARD_POLL_MS);
    return () => clearInterval(handle);
  }, [info?.available, refreshGuard]);

  const applyUpdate = useCallback(async () => {
    setApplying(true);
    setApplyError(null);
    setApplyMessage(null);
    try {
      // On success this never returns — the app restarts. A returned value means
      // the backend declined to restart (work started during the download) and the
      // update is staged for next launch instead.
      const outcome = await invoke<ApplyOutcome>("apply_update");
      setApplyMessage(outcome.message);
    } catch (err) {
      setApplyError(String(err));
      void refreshGuard();
    } finally {
      setApplying(false);
    }
  }, [refreshGuard]);

  return { info, guard, applying, applyMessage, applyError, checkError, check, applyUpdate };
}

/**
 * Declares screen-level busy state to the Rust guard for as long as the calling
 * component says so. Rust still enforces its own RAII guards on long-running
 * commands — this only covers what the backend cannot observe, namely "the user is
 * mid-flow" and "there is an unconfirmed review package on screen".
 */
export function useDeclareUpdateBusy(busy: boolean) {
  useEffect(() => {
    void invoke("set_update_ui_busy", { busy }).catch(() => undefined);
    // Clear on unmount so a crash or navigation can't strand the flag at true and
    // block updates forever.
    return () => {
      void invoke("set_update_ui_busy", { busy: false }).catch(() => undefined);
    };
  }, [busy]);
}
