import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Mirrors src-tauri/src/auth/backend_client.rs's `ProfileSummary`/`IngestStatusResponse`.
export interface ProfileSummary {
  id: string;
  display_name: string;
}

export interface IngestStatusResponse {
  job_id: string;
  // Kept as a raw string, not a union — avoids drift if the backend adds a status
  // value this hook doesn't know about yet.
  status: string;
  progress: number | null;
  // Opaque — this hook never inspects ReviewPackage's nested shape, only carries
  // it between the status poll and `confirm`. A real review UI (task 4.12) should
  // define its own typed `ReviewPackage` interface for whatever it renders/edits.
  review_package: unknown | null;
  extracted_skills: number;
  error_message: string | null;
  status_label: string;
}

const POLL_INTERVAL_MS = 2000;
// 'complete' must be included: because `POST .../ingest` hardcodes `autoConfirm:
// true` today, `ProfileService.commitReviewPackage` (called synchronously right
// after the assembler sets `awaiting_review`) flips the job to `complete` in the
// same transaction — a poller effectively never observes `awaiting_review` with a
// populated `reviewPackage` in practice, it goes straight to `complete` with
// `reviewPackage: null`. Kept `awaiting_review` in this list too for
// forward-compatibility, in case a future non-auto-confirm call path is added.
const TERMINAL_STATUSES = ["awaiting_review", "complete", "failed"];

export function useServerFallbackIngest() {
  const [status, setStatus] = useState<IngestStatusResponse | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const pollHandle = useRef<ReturnType<typeof setInterval> | null>(null);

  const stopPolling = useCallback(() => {
    if (pollHandle.current !== null) {
      clearInterval(pollHandle.current);
      pollHandle.current = null;
    }
  }, []);

  useEffect(() => stopPolling, [stopPolling]);

  const pickFile = useCallback(async (): Promise<string | null> => {
    return invoke<string | null>("pick_resume_file");
  }, []);

  const listProfiles = useCallback(async (): Promise<ProfileSummary[]> => {
    // An empty array is a valid result (no profiles yet) — not special-cased here;
    // the UI built on top of this hook (task 4.12) decides the empty-state messaging.
    return invoke<ProfileSummary[]>("list_my_profiles");
  }, []);

  const pollStatus = useCallback(
    async (profileId: string) => {
      try {
        // A `null` result means no job exists yet for this profile — an expected
        // "still pending" state, not an error.
        const result = await invoke<IngestStatusResponse | null>("get_server_fallback_ingest_status", {
          profileId,
        });
        if (result) {
          setStatus(result);
          if (TERMINAL_STATUSES.includes(result.status)) {
            stopPolling();
          }
        }
      } catch (err) {
        setErrorMessage(String(err));
        stopPolling();
      }
    },
    [stopPolling],
  );

  const startIngest = useCallback(
    async (profileId: string, filePath: string) => {
      setBusy(true);
      setErrorMessage(null);
      setStatus(null);
      stopPolling();
      try {
        await invoke("start_server_fallback_ingest", { profileId, filePath });
        await pollStatus(profileId);
        pollHandle.current = setInterval(() => pollStatus(profileId), POLL_INTERVAL_MS);
      } catch (err) {
        setErrorMessage(String(err));
      } finally {
        setBusy(false);
      }
    },
    [pollStatus, stopPolling],
  );

  // Despite the name, this isn't a first-persistence gate — the backend
  // auto-commits accepted items as soon as the job reaches `awaiting_review`
  // (see backend_client.rs's `confirm_ingest` doc comment). This call applies the
  // user's review edits/rejections on top of that already-committed data. Kept
  // mandatory (not skippable) at the hook layer — deciding *when* to call it
  // (immediately with all-accepted, or after edits) belongs to the review UI, not
  // this hook. Known limitation: the backend rejects an all-rejected package with
  // HTTP 400 ("At least one item must be accepted") — callers should avoid that.
  const confirm = useCallback(async (profileId: string, reviewPackage: unknown) => {
    setBusy(true);
    try {
      await invoke("confirm_server_fallback_ingest", { profileId, reviewPackage });
    } catch (err) {
      setErrorMessage(String(err));
      throw err;
    } finally {
      setBusy(false);
    }
  }, []);

  return {
    status,
    errorMessage,
    busy,
    pickFile,
    listProfiles,
    startIngest,
    confirm,
  };
}
