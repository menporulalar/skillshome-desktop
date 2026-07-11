/**
 * run-local-extraction-and-stage.ts — Module 4 tasks 4.11 + 4.12.
 *
 * Runs the local extraction agents (task 4.3c/4.9), then syncs the result to the
 * server over the real MCP transport (skillshome-app's `app/api/mcp/route.ts`) via
 * `profile.ingest.stage`, instead of a raw file upload — the Local_Model/
 * BYOK_Frontier counterpart to task 4.10's Server_Fallback REST path.
 *
 * Stage-only — never calls `profile.ingest.confirm`. That's a fully separate step
 * now (`confirm-staged-ingestion.ts`), since task 4.12 added a real review screen
 * that sits between staging and confirming; a single script that could silently
 * auto-accept-and-commit no longer fits once a user is expected to review first.
 *
 * Invoked two ways:
 *  - Manually, for CLI testing (`npm run stage -- <file> <profile-id>`), reading
 *    plain console output.
 *  - By Rust (task 4.12, `src-tauri/src/ingest/sidecar.rs`), which spawns this via
 *    `npm run stage --` and greps stdout for the `__SIDECAR_RESULT__:` marker line
 *    below — everything else on stdout (including `@menporulalar/agents-core`'s own
 *    logger, which writes info-level JSON lines to stdout) is informational only.
 *    All of THIS script's own progress messages go to stderr for exactly that
 *    reason — only the final marker line belongs on stdout.
 *
 * The access token is read from an env var as a stand-in for how the real
 * Rust-spawned child process passes it (same justification already used for task
 * 4.9's BYOK_API_KEY): a child-process env var, never over a network.
 *
 * Usage: npm run stage -- <path-to-a-resume-file> <profile-id>
 *        SKILLSHOME_ACCESS_TOKEN=<token> must be set.
 *        SKILLSHOME_BACKEND_URL defaults to http://localhost:3000.
 *        (set BYOK_API_KEY=<key> first if Extraction_Source is byok_frontier)
 */
import { runLocalExtraction } from './run-local-extraction';
import { resolveActiveExtractionSource } from './resolveExtractionConfig';
import { connectMcpClient, stageIngestion } from './mcpClient';

const RESULT_MARKER = '__SIDECAR_RESULT__:';

function printResult(result: Record<string, unknown>) {
  console.log(`${RESULT_MARKER}${JSON.stringify(result)}`);
}

async function main() {
  const filePath = process.argv[2];
  const profileId = process.argv[3];

  if (!filePath || !profileId) {
    console.error('Usage: npm run stage -- <path-to-a-resume-file> <profile-id>');
    process.exit(1);
  }

  const backendUrl = process.env.SKILLSHOME_BACKEND_URL ?? 'http://localhost:3000';
  const accessToken = process.env.SKILLSHOME_ACCESS_TOKEN;
  if (!accessToken) {
    console.error('SKILLSHOME_ACCESS_TOKEN env var is required');
    process.exit(1);
  }

  const extractionSource = resolveActiveExtractionSource();
  if (extractionSource === 'server_fallback') {
    console.error('active_source is server_fallback — this script is for Local_Model/BYOK_Frontier only');
    process.exit(1);
  }

  console.error('Running local extraction...');
  const { inputType, ...extractionResult } = await runLocalExtraction(filePath);

  console.error(`Connecting to MCP server at ${backendUrl}...`);
  const client = await connectMcpClient(backendUrl, accessToken);

  console.error('Staging extraction result via profile.ingest.stage...');
  const { jobId, reviewPackage } = await stageIngestion(client, {
    profileId,
    inputType,
    extractionSource,
    ...extractionResult,
  });

  console.error(`Staged as job ${jobId}.`);
  printResult({ ok: true, jobId, reviewPackage });
  // Defensive, matching confirm-staged-ingestion.ts's fix: guarantees termination
  // even if something (the MCP client's HTTP connection, etc.) would otherwise keep
  // Node's event loop alive past the point where the actual work is done.
  process.exit(0);
}

main().catch((err) => {
  console.error(err);
  printResult({ ok: false, error: err instanceof Error ? err.message : String(err) });
  process.exit(1);
});
