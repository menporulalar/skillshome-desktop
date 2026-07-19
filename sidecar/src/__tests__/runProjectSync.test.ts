/**
 * runProjectSync.test.ts — the pure pieces of the project-sync sidecar script,
 * plus a real end-to-end scanLocal run against a tmpdir fixture (mirroring the
 * out-of-scope-symlink fixture the server repo's checkpoint uses), proving the
 * published @menporulalar/agents-core actually delivers the Module 3 core.
 */
import { mkdtempSync, mkdirSync, writeFileSync, symlinkSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { scanLocal, hashSignalSet, findingsToExtractedSkills } from '@menporulalar/agents-core';
import { decideSyncAction } from '../run-project-sync';

let failures = 0;
function check(name: string, cond: boolean) {
  if (cond) console.log(`  ✓ ${name}`);
  else { console.error(`  ✗ ${name}`); failures++; }
}

console.log('decideSyncAction (Requirement 5.3 dedup)');
{
  check('no findings → no_signal', decideSyncAction({ findingCount: 0, signalHash: 'h', lastSignalHash: '-' }) === 'no_signal');
  check('matching hash → unchanged (nothing staged)', decideSyncAction({ findingCount: 3, signalHash: 'h1', lastSignalHash: 'h1' }) === 'unchanged');
  check('different hash → stage', decideSyncAction({ findingCount: 3, signalHash: 'h2', lastSignalHash: 'h1' }) === 'stage');
  check("no recorded hash ('-') → stage", decideSyncAction({ findingCount: 3, signalHash: 'h2', lastSignalHash: '-' }) === 'stage');
}

(async () => {
  console.log('published agents-core scanLocal against a real fixture');
  const outside = mkdtempSync(join(tmpdir(), 'pss-d-outside-'));
  const root = mkdtempSync(join(tmpdir(), 'pss-d-root-'));
  try {
    writeFileSync(join(outside, 'package.json'), JSON.stringify({ dependencies: { 'leaked-lib': '1' } }));
    writeFileSync(join(root, 'package.json'), JSON.stringify({ dependencies: { react: '18', typescript: '5' } }));
    writeFileSync(join(root, 'CLAUDE.md'), 'agent instructions SENTINEL_CONTENT');
    mkdirSync(join(root, 'sub'));
    symlinkSync(join(outside, 'package.json'), join(root, 'sub', 'package.json'));

    const off = await scanLocal(root, { agentConfigScanEnabled: false });
    check('finds manifest skills', off.findings.some((f) => f.skillName === 'react'));
    check('out-of-scope symlink skipped', off.skippedOutOfScope === 1 && !off.findings.some((f) => f.skillName === 'leaked-lib'));
    check('agent-config unread when toggled off', !off.findings.some((f) => f.skillName === 'Claude Code'));
    check('no raw content in results', !JSON.stringify(off).includes('SENTINEL_CONTENT'));

    const on = await scanLocal(root, { agentConfigScanEnabled: true });
    check('agent-config signal when toggled on', on.findings.some((f) => f.skillName === 'Claude Code'));

    const hash1 = hashSignalSet(off.findings);
    const hash2 = hashSignalSet((await scanLocal(root, { agentConfigScanEnabled: false })).findings);
    check('re-scan of an untouched folder hashes identically', hash1 === hash2);

    const skills = findingsToExtractedSkills(on.findings);
    check('mapped skills carry evidence + currency', skills.every((s) => typeof s.evidenceTimestamp === 'string' && typeof s.skillCurrency === 'string'));
    check('mapped skills carry project_sync source sections', skills.every((s) => s.mentions.every((m) => m.section.startsWith('project_sync@'))));
  } finally {
    rmSync(root, { recursive: true, force: true });
    rmSync(outside, { recursive: true, force: true });
  }

  console.log(`\n${failures === 0 ? 'All project-sync sidecar checks passed' : `${failures} check(s) FAILED`}`);
  process.exit(failures === 0 ? 0 : 1);
})();
