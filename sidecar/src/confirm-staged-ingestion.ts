/**
 * confirm-staged-ingestion.ts — Module 4 task 4.12.
 *
 * The confirm half of the Local_Model/BYOK_Frontier flow — the counterpart to
 * `run-local-extraction-and-stage.ts`'s stage half. Split into its own script
 * because a real review screen (task 4.12) now sits between staging and
 * confirming: the user reviews/edits the staged `reviewPackage` in the desktop UI,
 * and only THAT edited package — not a blind accept-all — gets confirmed.
 *
 * The edited `ReviewPackage` is read from **stdin**, not a CLI arg — it can be
 * large/deeply nested (arbitrary numbers of skills/experience/projects), and CLI
 * args have practical length/escaping limits stdin doesn't.
 *
 * Invoked by Rust (`src-tauri/src/ingest/sidecar.rs`), which spawns this via
 * `npm run confirm --`, writes the edited package to the child's stdin, and greps
 * stdout for the `__SIDECAR_RESULT__:` marker line — same convention as the stage
 * script, for the same reason (agents-core's own logger also writes to stdout).
 *
 * Usage: npm run confirm -- <profile-id>   (reads ReviewPackage JSON from stdin)
 *        SKILLSHOME_ACCESS_TOKEN=<token> must be set.
 *        SKILLSHOME_BACKEND_URL defaults to http://localhost:3000.
 */
import { connectMcpClient, confirmIngestion } from './mcpClient';

const RESULT_MARKER = '__SIDECAR_RESULT__:';

function printResult(result: Record<string, unknown>) {
  console.log(`${RESULT_MARKER}${JSON.stringify(result)}`);
}

function readStdin(): Promise<string> {
  return new Promise((resolve, reject) => {
    let data = '';
    process.stdin.setEncoding('utf-8');
    process.stdin.on('data', (chunk) => { data += chunk; });
    process.stdin.on('end', () => resolve(data));
    process.stdin.on('error', reject);
  });
}

async function main() {
  const profileId = process.argv[2];
  if (!profileId) {
    console.error('Usage: npm run confirm -- <profile-id>   (reads ReviewPackage JSON from stdin)');
    process.exit(1);
  }

  const backendUrl = process.env.SKILLSHOME_BACKEND_URL ?? 'http://localhost:3000';
  const accessToken = process.env.SKILLSHOME_ACCESS_TOKEN;
  if (!accessToken) {
    console.error('SKILLSHOME_ACCESS_TOKEN env var is required');
    process.exit(1);
  }

  console.error('Reading confirmed ReviewPackage from stdin...');
  const raw = await readStdin();
  const confirmedItems: unknown = JSON.parse(raw);

  console.error(`Connecting to MCP server at ${backendUrl}...`);
  const client = await connectMcpClient(backendUrl, accessToken);

  console.error('Confirming via profile.ingest.confirm...');
  const result = await confirmIngestion(client, profileId, confirmedItems);

  console.error('Confirmed.');
  printResult({ ok: true, ...result });
  // Reading stdin (readStdin() above) leaves process.stdin in flowing mode, which
  // keeps Node's event loop alive indefinitely even after 'end' fires — a well-known
  // Node quirk, not something that resolves on its own. Explicit exit guarantees
  // termination regardless of that (or any other lingering open handle, e.g. the MCP
  // client's HTTP connection) rather than relying on the event loop draining naturally.
  process.exit(0);
}

main().catch((err) => {
  console.error(err);
  printResult({ ok: false, error: err instanceof Error ? err.message : String(err) });
  process.exit(1);
});
