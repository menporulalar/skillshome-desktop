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
  const { info, guard, applying, applyMessage, applyError, applyUpdate } = useUpdateCheck();

  if (!info?.available) return null;

  const blocked = guard.busy;

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
