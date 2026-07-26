#!/usr/bin/env node
/**
 * .claude/hooks/guard.mjs — PreToolUse safety guard for SkillsHome Desktop.
 *
 * Hard-blocks (not just reminders):
 *   1. Write/Edit to the updater signing keypair or real .env files (.env.example is fine).
 *   2. Bash commands that read/print/cat the private signing key contents.
 *
 * Fail-open: any internal error allows the call through (never wedge the session).
 */
import { readFileSync } from 'node:fs';

function deny(reason) {
  process.stdout.write(JSON.stringify({
    hookSpecificOutput: {
      hookEventName: 'PreToolUse',
      permissionDecision: 'deny',
      permissionDecisionReason: reason,
    },
  }));
  process.exit(0);
}

const SECRET_FILE_RE = /(^|\/)(\.tauri-keys(\.pub)?|\.tauri-keys\..*|\.env(\.production|\.local)?)$/;

try {
  const payload = JSON.parse(readFileSync('/dev/stdin', 'utf8') || '{}');
  const tool = payload.tool_name ?? payload.tool ?? '';
  const input = payload.tool_input ?? payload.toolInput ?? {};

  if (tool === 'Bash') {
    const cmd = String(input.command ?? '');
    if (/\b(cat|less|more|head|tail|pbcopy|xxd|strings)\b[\s\S]*\.tauri-keys\b/.test(cmd)) {
      deny('Blocked: this reads the updater signing private key. Never print/copy its contents — the CI secret is set by the human directly. — SkillsHome Desktop guard');
    }
  }

  if (tool === 'Edit' || tool === 'Write' || tool === 'MultiEdit' || tool === 'NotebookEdit') {
    const file = String(input.file_path ?? input.path ?? input.notebook_path ?? '');
    const base = file.split('/').pop() ?? '';
    if (SECRET_FILE_RE.test('/' + base) && base !== '.env.example') {
      deny(`Blocked: ${base} holds real secrets (updater signing key or OAuth env values) and must be edited by hand, never by the agent. — SkillsHome Desktop guard`);
    }
  }
} catch {
  // Fail-open — a guard bug must never block legitimate work.
}
process.exit(0);
