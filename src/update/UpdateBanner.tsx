import { useUpdateCheck } from "./useUpdateCheck";

/**
 * #28 R3 — the visible "update available" state with a user-triggered apply.
 *
 * Renders nothing until an update actually exists, so it costs no screen space in
 * the common case. The restart button is disabled whenever the Rust guard reports
 * work in flight; the reason is shown rather than leaving a dead control with no
 * explanation. Rust refuses the call independently, so this is presentation, not
 * the safety mechanism.
 */
export function UpdateBanner() {
  const { info, guard, applying, applyMessage, applyError, downloadProgress, applyUpdate } =
    useUpdateCheck();

  if (!info?.available) return null;

  const blocked = guard.busy;
  // `total` is only known once the server sends Content-Length; until then (or if it
  // never does) the bar renders indeterminate rather than claiming a false percentage.
  const progressPercent =
    downloadProgress?.total != null
      ? Math.min(100, Math.round((downloadProgress.downloaded / downloadProgress.total) * 100))
      : null;

  return (
    <section
      aria-label="Application update"
      style={{
        border: "1px solid currentColor",
        borderRadius: "6px",
        padding: "0.75em 1em",
        margin: "0 0 1em",
        textAlign: "left",
      }}
    >
      <strong>Update available</strong>
      <p style={{ margin: "0.35em 0" }}>
        Version {info.latest_version} is ready. You&apos;re on {info.current_version}.
      </p>

      {applyMessage && <p role="status">{applyMessage}</p>}
      {applyError && <p role="alert">{applyError}</p>}

      {applying && (
        <div
          role="progressbar"
          aria-label="Downloading update"
          aria-valuenow={progressPercent ?? undefined}
          aria-valuemin={0}
          aria-valuemax={100}
          style={{
            height: "6px",
            borderRadius: "3px",
            background: "color-mix(in srgb, currentColor 15%, transparent)",
            margin: "0.5em 0",
            overflow: "hidden",
          }}
        >
          <div
            style={{
              height: "100%",
              borderRadius: "3px",
              background: "currentColor",
              width: progressPercent != null ? `${progressPercent}%` : "40%",
              // Indeterminate state (no Content-Length): a sliding segment rather than
              // a static bar, so it doesn't read as "stuck".
              animation: progressPercent == null ? "update-banner-indeterminate 1.2s ease-in-out infinite" : undefined,
            }}
          />
        </div>
      )}
      {applying && (
        <style>{`@keyframes update-banner-indeterminate { 0% { transform: translateX(-100%); } 100% { transform: translateX(250%); } }`}</style>
      )}

      {blocked && (
        <p role="status" style={{ margin: "0.35em 0", opacity: 0.85 }}>
          Can&apos;t restart yet — {guard.reason}.
        </p>
      )}

      <button
        type="button"
        onClick={() => void applyUpdate()}
        disabled={blocked || applying}
        // Without this, a disabled button gives no reason on hover for anyone who
        // missed the line above.
        title={blocked ? `Can't restart yet — ${guard.reason}` : "Download, install, and restart"}
      >
        {applying ? "Updating…" : "Update & Restart"}
      </button>
    </section>
  );
}
