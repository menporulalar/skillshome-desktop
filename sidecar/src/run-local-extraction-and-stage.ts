/**
 * run-local-extraction-and-stage.ts — Module 4 task 4.11.
 *
 * Proves the MCP_Client path end to end: run the same local extraction agents
 * run-local-extraction.ts already proves work (task 4.3c/4.9), then sync the
 * result to the server over the real MCP transport (skillshome-app's
 * `app/api/mcp/route.ts`) via `profile.ingest.stage`, instead of a raw file
 * upload — this is the Local_Model/BYOK_Frontier counterpart to task 4.10's
 * Server_Fallback REST path.
 *
 * Defaults to stage-and-print-only. `profile.ingest.confirm` is NOT called
 * unless `--confirm` is passed explicitly — auto-accepting and confirming by
 * default would mutate a real profile on whatever server this points at, which
 * is more than this wiring-proof script should risk doing silently.
 *
 * No Tauri/Rust process-spawning wiring here — that's task 4.12's job. The
 * access token is read from an env var as a stand-in for how a Rust-spawned
 * child process would pass it later (same justification already used for
 * task 4.9's BYOK_API_KEY): a child-process env var, never over a network.
 *
 * Usage: npm run extract:sample:stage -- <path-to-a-resume-file> <profile-id> [--confirm]
 *        SKILLSHOME_ACCESS_TOKEN=<token> must be set.
 *        SKILLSHOME_BACKEND_URL defaults to http://localhost:3000.
 *        (set BYOK_API_KEY=<key> first if Extraction_Source is byok_frontier)
 */
import { runLocalExtraction } from './run-local-extraction';
import { resolveActiveExtractionSource } from './resolveExtractionConfig';
import { connectMcpClient, stageIngestion, confirmIngestion } from './mcpClient';

// Local, minimal shape — not exported from @menporulalar/agents-core (that package
// only owns extraction-agent output types, not the server's review/confirm
// workflow types). Only the fields this script's force-accept helper touches.
interface ReviewPackageItem {
  status: string;
  [key: string]: unknown;
}

function acceptAll(items: ReviewPackageItem[]): ReviewPackageItem[] {
  return items.map((item) => ({ ...item, status: 'accepted' }));
}

async function main() {
  const filePath = process.argv[2];
  const profileId = process.argv[3];
  const shouldConfirm = process.argv.includes('--confirm');

  if (!filePath || !profileId) {
    console.error('Usage: npm run extract:sample:stage -- <path-to-a-resume-file> <profile-id> [--confirm]');
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

  console.log('Running local extraction...');
  const { inputType, ...extractionResult } = await runLocalExtraction(filePath);

  console.log(`Connecting to MCP server at ${backendUrl}...`);
  const client = await connectMcpClient(backendUrl, accessToken as string);

  console.log('Staging extraction result via profile.ingest.stage...');
  const { jobId, reviewPackage } = await stageIngestion(client, {
    profileId,
    inputType,
    extractionSource,
    ...extractionResult,
  });

  console.log(`Staged as job ${jobId}. Review package:`);
  console.log(JSON.stringify(reviewPackage, null, 2));

  if (!shouldConfirm) {
    console.log('\nStage-only run (pass --confirm to also commit this to the profile).');
    return;
  }

  console.log('\n--confirm passed — accepting all items and confirming...');
  const rp = reviewPackage as {
    jobId: string;
    profileId: string;
    skills: ReviewPackageItem[];
    projects: ReviewPackageItem[];
    experience: ReviewPackageItem[];
    education?: ReviewPackageItem[];
    certificates?: ReviewPackageItem[];
    accolades?: ReviewPackageItem[];
  };
  const confirmedItems = {
    ...rp,
    skills: acceptAll(rp.skills),
    projects: acceptAll(rp.projects),
    experience: acceptAll(rp.experience),
    education: acceptAll(rp.education ?? []),
    certificates: acceptAll(rp.certificates ?? []),
    accolades: acceptAll(rp.accolades ?? []),
  };

  const result = await confirmIngestion(client, profileId, confirmedItems);
  console.log('Confirm result:', JSON.stringify(result, null, 2));
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
