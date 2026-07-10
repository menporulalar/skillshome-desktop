import { useEffect, useRef } from "react";
import type { ExtractionSource } from "../extraction/useExtractionSettings";
import type { useServerFallbackIngest, IngestSource } from "./useServerFallbackIngest";
import type { useLocalExtraction } from "./useLocalExtraction";

// Owns the actual "start the job" side effect for both paths (kicked off once on
// mount) — SourcePickerScreen only captures profileId/source, it doesn't touch
// either hook directly, so this is the one place that decides which path's work
// actually begins.
interface Props {
  activeSource: ExtractionSource;
  profileId: string;
  source: IngestSource;
  serverFallback: ReturnType<typeof useServerFallbackIngest>;
  localExtraction: ReturnType<typeof useLocalExtraction>;
  onReviewReady: (reviewPackage: unknown) => void;
  // Server_Fallback hardcodes autoConfirm server-side (task 4.10's finding) — a
  // job reaching `complete` with no `review_package` attached means the data's
  // already committed, there's nothing left to review. Distinct from
  // onReviewReady, which is for a genuine reviewable package.
  onComplete: () => void;
  onRetry: () => void;
  // Task 4.13, Requirement 3.9: offered only for Local_Model/BYOK_Frontier, only
  // after the retry budget (useLocalExtraction's own outer retry loop) is fully
  // exhausted — retries the *same* profileId/source through Server_Fallback
  // instead of sending the user back to re-pick.
  onRetryViaServerFallback: () => void;
}

export function ExtractionProgressScreen({
  activeSource,
  profileId,
  source,
  serverFallback,
  localExtraction,
  onReviewReady,
  onComplete,
  onRetry,
  onRetryViaServerFallback,
}: Props) {
  const started = useRef(false);

  useEffect(() => {
    if (started.current) return;
    started.current = true;

    if (activeSource === "server_fallback") {
      serverFallback.startIngest(profileId, source);
      return;
    }

    // Local_Model/BYOK_Frontier: file-only (URL input is gated to server_fallback
    // in SourcePickerScreen, since neither path has a local GitHub-scanning
    // agent) — a single blocking spawn (task 4.12, dev-mode only) with no
    // incremental progress signal beyond the retry-attempt counter below.
    // Staging always yields a real review_package here (no server-side
    // auto-confirm on this path).
    if (source.kind !== "file") return;
    localExtraction
      .startAndStage(profileId, source.path)
      .then((result) => onReviewReady(result.reviewPackage))
      .catch(() => {
        // localExtraction.errorMessage already captures this — nothing further
        // to do here, the render below reflects it.
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Server_Fallback: the hook's own polling already reached a terminal status.
  useEffect(() => {
    if (activeSource !== "server_fallback" || !serverFallback.status) return;
    const { status, review_package } = serverFallback.status;
    if (review_package && (status === "awaiting_review" || status === "complete")) {
      onReviewReady(review_package);
    } else if (status === "complete") {
      onComplete();
    }
    // 'failed' falls through to the error branch below via serverFallback.status.error_message.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [serverFallback.status]);

  const jobFailedMessage =
    activeSource === "server_fallback" && serverFallback.status?.status === "failed"
      ? serverFallback.status.error_message ?? "Extraction failed"
      : null;
  const errorMessage =
    (activeSource === "server_fallback" ? serverFallback.errorMessage : localExtraction.errorMessage) ?? jobFailedMessage;

  if (errorMessage) {
    return (
      <main className="container">
        <h1>Extraction failed</h1>
        <p style={{ color: "red" }}>{errorMessage}</p>
        <div className="row" style={{ gap: "0.5em" }}>
          <button type="button" onClick={onRetry}>
            Try Again
          </button>
          {activeSource !== "server_fallback" && (
            <button type="button" onClick={onRetryViaServerFallback}>
              Retry via Server_Fallback
            </button>
          )}
        </div>
      </main>
    );
  }

  return (
    <main className="container">
      <h1>Extracting…</h1>
      {activeSource === "server_fallback" && serverFallback.status ? (
        <p>
          {serverFallback.status.status_label}
          {serverFallback.status.progress != null ? ` (${serverFallback.status.progress}%)` : ""}
        </p>
      ) : activeSource !== "server_fallback" && localExtraction.attempt > 1 ? (
        // Requirement 7.3: a clear offline/retry state — the user sees retries
        // happening (useLocalExtraction's outer retry loop) rather than a silent
        // hang. No visible flash on the common case (first attempt succeeds).
        <p>Retrying (attempt {localExtraction.attempt} of {localExtraction.maxAttempts})…</p>
      ) : (
        <p>Running extraction locally — this can take a minute…</p>
      )}
    </main>
  );
}
