#!/usr/bin/env node
/**
 * .claude/hooks/session-start.mjs — SessionStart hook.
 *
 * Prints a short briefing so every new/post-compaction session immediately knows:
 *   - whether the three version files (package.json, Cargo.toml, tauri.conf.json) agree
 *   - current git branch
 *
 * Output only — reads nothing sensitive (never touches .tauri-keys/.env), writes nothing.
 * Fail-open (silent on error).
 */
import { readFileSync } from 'node:fs';
import { execSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const out = [];

try {
  const pkgVersion = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8')).version;
  const tauriConfVersion = JSON.parse(
    readFileSync(join(ROOT, 'src-tauri', 'tauri.conf.json'), 'utf8'),
  ).version;
  const cargoToml = readFileSync(join(ROOT, 'src-tauri', 'Cargo.toml'), 'utf8');
  const cargoVersionMatch = cargoToml.match(/^\s*version\s*=\s*"([^"]+)"/m);
  const cargoVersion = cargoVersionMatch ? cargoVersionMatch[1] : null;

  const versions = { 'package.json': pkgVersion, 'Cargo.toml': cargoVersion, 'tauri.conf.json': tauriConfVersion };
  const unique = new Set(Object.values(versions));
  if (unique.size === 1) {
    out.push(`✅ Version in sync across package.json / Cargo.toml / tauri.conf.json: ${pkgVersion}`);
  } else {
    out.push('⚠️  Version MISMATCH across files (fix before tagging a release):');
    for (const [file, v] of Object.entries(versions)) out.push(`  • ${file}: ${v}`);
  }
} catch {
  /* one of the files is missing/unreadable — skip silently */
}

try {
  const branch = execSync('git rev-parse --abbrev-ref HEAD', { cwd: ROOT, encoding: 'utf8' }).trim();
  out.push(`🌿 Branch: ${branch}`);
} catch { /* not a git repo or git unavailable */ }

if (out.length) {
  process.stdout.write('=== SkillsHome Desktop session briefing ===\n' + out.join('\n') + '\n');
}
process.exit(0);
