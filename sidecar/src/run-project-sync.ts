/**
 * run-project-sync.ts — #25 living-profile-project-skill-sync, Module 3's
 * desktop half: scans ONE granted local folder with the shared
 * Project_Skill_Sync_Agent core (@menporulalar/agents-core ≥0.3.0) and stages
 * the result over the real MCP transport (`profile.ingest.stage`) — the same
 * review-gated pipeline as every other ingestion source. Never confirms:
 * the candidate reviews the package (desktop review screen or web) first.
 *
 * Dedup (Requirement 5.3): the caller passes the server-recorded
 * lastSignalHash (from GET /api/profiles/[id]/connected-projects); when the
 * fresh scan hashes identically, this script stages nothing and reports
 * `unchanged`. The hash itself only ever moves on CONFIRM, server-side.
 *
 * Privacy invariants inherited from the shared core (property-tested there):
 * allow-listed files only, symlink containment, raw content discarded at
 * parse. This script additionally sends ONLY derived findings over the wire —
 * the folder path itself never leaves the machine (the server knows the
 * project by its display label alone).
 *
 * Spawned by Rust (src-tauri/src/projectsync/) via `npm run project-sync --`;
 * stdout carries exactly one `__SIDECAR_RESULT__:{...}` line (same contract as
 * stage/confirm — see run-local-extraction-and-stage.ts's header for why all
 * progress goes to stderr).
 *
 * Usage: npm run project-sync -- <folder-path> <profile-id> <connected-project-id> <last-signal-hash|-> <agent-config-enabled true|false>
 *        SKILLSHOME_ACCESS_TOKEN must be set; SKILLSHOME_BACKEND_URL defaults
 *        to http://localhost:3000.
 */
import {
  scanLocal,
  hashSignalSet,
  findingsToExtractedSkills,
} from '@menporulalar/agents-core';
import { connectMcpClient, stageIngestion } from './mcpClient';

const RESULT_MARKER = '__SIDECAR_RESULT__:';

function printResult(result: Record<string, unknown>) {
  console.log(`${RESULT_MARKER}${JSON.stringify(result)}`);
}

/**
 * Pure dedup/staging decision — exported for the unit suite.
 * `lastSignalHash` uses '-' as the CLI stand-in for "none recorded yet".
 */
export function decideSyncAction(input: {
  findingCount: number;
  signalHash: string;
  lastSignalHash: string;
}): 'no_signal' | 'unchanged' | 'stage' {
  if (input.findingCount === 0) return 'no_signal';
  if (input.lastSignalHash !== '-' && input.lastSignalHash === input.signalHash) return 'unchanged';
  return 'stage';
}

async function main() {
  const [folderPath, profileId, connectedProjectId, lastSignalHash, agentConfigFlag] =
    process.argv.slice(2);

  if (!folderPath || !profileId || !connectedProjectId || !lastSignalHash || !agentConfigFlag) {
    console.error(
      'Usage: npm run project-sync -- <folder-path> <profile-id> <connected-project-id> <last-signal-hash|-> <agent-config-enabled true|false>',
    );
    process.exit(1);
  }
  const accessToken = process.env.SKILLSHOME_ACCESS_TOKEN;
  if (!accessToken) {
    printResult({ ok: false, error: 'SKILLSHOME_ACCESS_TOKEN is not set' });
    process.exit(1);
  }
  const backendUrl = process.env.SKILLSHOME_BACKEND_URL ?? 'http://localhost:3000';

  console.error(`[project-sync] scanning ${folderPath} (agent-config: ${agentConfigFlag})`);
  const scan = await scanLocal(folderPath, {
    agentConfigScanEnabled: agentConfigFlag === 'true',
  });
  console.error(
    `[project-sync] ${scan.findings.length} findings from ${scan.scannedFiles.length} files` +
      (scan.skippedOutOfScope > 0 ? ` (${scan.skippedOutOfScope} out-of-scope symlink(s) skipped)` : ''),
  );

  const signalHash = hashSignalSet(scan.findings);
  const action = decideSyncAction({
    findingCount: scan.findings.length,
    signalHash,
    lastSignalHash,
  });

  if (action === 'no_signal') {
    printResult({ ok: true, outcome: 'no_signal', signalHash });
    process.exit(0);
  }
  if (action === 'unchanged') {
    console.error('[project-sync] signal unchanged since last confirm — nothing to stage');
    printResult({ ok: true, outcome: 'unchanged', signalHash });
    process.exit(0);
  }

  const skills = findingsToExtractedSkills(scan.findings);
  console.error(`[project-sync] staging ${skills.length} skill(s) for review`);

  const client = await connectMcpClient(backendUrl, accessToken);
  try {
    const { jobId } = await stageIngestion(client, {
      profileId,
      inputType: 'project_sync',
      extractionSource: 'project_sync',
      skills,
      experience: [],
      projects: [],
      rawTextPreview: '',
      projectSync: { connectedProjectId, signalHash },
    });
    printResult({ ok: true, outcome: 'staged', jobId, skillCount: skills.length, signalHash });
  } finally {
    await client.close().catch(() => undefined);
  }
}

if (require.main === module) {
  main().catch((err) => {
    printResult({ ok: false, error: err instanceof Error ? err.message : String(err) });
    process.exit(1);
  });
}
