// Task 4.14, Requirement 6.5: "surface Staging_Tool and Confirm_Tool failures with
// the same user-facing error categories as the web app... never a raw stack trace."
// The web app itself has no formal error-category system (its own ingestion error
// handling is just `err.message` with a generic fallback — grepped, confirmed) — so
// this matches that same *principle*, not a taxonomy that doesn't exist elsewhere.
// The real job: clean up the specific raw strings the desktop app's own transports
// (MCP via the sidecar) can produce that the web app never had to deal with.
//
// Mirrors sidecar/src/mcpClient.ts's parseMcpToolErrorReason regex/shape —
// duplicated, not imported (different runtime: this is the Tauri webview, that's
// the Node sidecar). If the server adds a new `reason` value, both copies need a
// matching update.
const MCP_ERROR_PREFIX = /^MCP error -?\d+: /;

export function mapDesktopError(raw: string): string {
  const stripped = raw.replace(MCP_ERROR_PREFIX, '');

  // Structured budget/concurrency errors: show the server's own clean inner message
  // directly — already worded consistently with the REST path's equivalent
  // messages, so no new copy needs inventing here.
  try {
    const parsed: unknown = JSON.parse(stripped);
    if (parsed !== null && typeof parsed === 'object' && 'reason' in parsed && 'message' in parsed) {
      return String((parsed as { message: unknown }).message);
    }
  } catch {
    // Not JSON — fall through to the other checks below.
  }

  // Zod validation dumps and anything else that still looks like raw MCP-prefixed
  // technical detail — never show this verbatim.
  if (raw.startsWith('MCP error')) {
    return 'Something went wrong while processing your request. Please try again.';
  }

  // Defensive: a genuine multi-line stack trace should never reach the UI via
  // Tauri's IPC (invoke() rejects with a plain string, not an Error object), but
  // guard anyway rather than assume that invariant holds forever.
  if (raw.includes('\n    at ')) {
    return 'Something went wrong. Please try again.';
  }

  // Already clean, human-authored text (network-unreachable messages, REST
  // BackendError messages, "not signed in", "Unsupported file type…", etc.) —
  // pass through unchanged.
  return raw;
}
