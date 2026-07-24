/**
 * src/__tests__/resolveInterviewLoopConfig.test.ts — the three Interview_Source
 * branches, mirroring resolveExtractionConfig.test.ts's style exactly (same
 * injected-temp-directory pattern) since this is the same shape of resolver
 * reading a different settings field (active_interview_source).
 *
 * Run with: npm test (from sidecar/)
 */
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { resolveInterviewLoopConfig, resolveActiveInterviewSource } from '../resolveInterviewLoopConfig';

let failures = 0;
function check(name: string, cond: boolean) {
  if (cond) console.log(`  ✓ ${name}`);
  else { console.error(`  ✗ ${name}`); failures++; }
}

function tempRoot(name: string): string {
  const root = join(tmpdir(), `skillshome-desktop-sidecar-test-${name}`);
  rmSync(root, { recursive: true, force: true });
  mkdirSync(root, { recursive: true });
  return root;
}

function writeSettings(root: string, settings: unknown) {
  writeFileSync(join(root, 'extraction_settings.json'), JSON.stringify(settings));
}

function run() {
  console.log('active_interview_source: local_model');
  {
    const root = tempRoot('interview-local-model');
    writeSettings(root, {
      active_interview_source: 'local_model',
      local_model: { endpoint: 'http://127.0.0.1:11434', model: 'llama3.2:3b' },
      byok_frontier: null,
    });
    const config = resolveInterviewLoopConfig(root);
    check('maps to the ollama provider', config.provider === 'ollama');
    check('uses the endpoint as baseURL', config.baseURL === 'http://127.0.0.1:11434');
    check('uses the configured model', config.model === 'llama3.2:3b');
    check('apiKey is not set', config.apiKey === undefined);
  }

  console.log('\nactive_interview_source: byok_frontier, BYOK_API_KEY set');
  {
    const root = tempRoot('interview-byok-with-key');
    writeSettings(root, {
      active_interview_source: 'byok_frontier',
      local_model: null,
      byok_frontier: { provider: 'anthropic', model: 'claude-3-7-sonnet-latest' },
    });
    process.env.BYOK_API_KEY = 'sk-ant-test-key';
    const config = resolveInterviewLoopConfig(root);
    check('uses the configured provider', config.provider === 'anthropic');
    check('uses the configured model', config.model === 'claude-3-7-sonnet-latest');
    check('uses BYOK_API_KEY as apiKey', config.apiKey === 'sk-ant-test-key');
    delete process.env.BYOK_API_KEY;
  }

  console.log('\nactive_interview_source: byok_frontier, BYOK_API_KEY NOT set');
  {
    const root = tempRoot('interview-byok-without-key');
    writeSettings(root, {
      active_interview_source: 'byok_frontier',
      local_model: null,
      byok_frontier: { provider: 'openai', model: 'gpt-4o-mini' },
    });
    delete process.env.BYOK_API_KEY;
    let threw = false;
    try {
      resolveInterviewLoopConfig(root);
    } catch (e) {
      threw = e instanceof Error && e.message.includes('BYOK_API_KEY env var is required');
    }
    check('throws a clear error instead of silently proceeding', threw);
  }

  console.log('\nactive_interview_source: server_fallback');
  {
    const root = tempRoot('interview-server-fallback');
    writeSettings(root, { active_interview_source: 'server_fallback', local_model: null, byok_frontier: null });
    let threw = false;
    try {
      resolveInterviewLoopConfig(root);
    } catch (e) {
      threw = e instanceof Error && e.message.includes('no local question generation should run');
    }
    check('throws — this source should never reach local question generation', threw);
  }

  console.log('\nresolveActiveInterviewSource');
  {
    const localRoot = tempRoot('interview-active-source-local');
    writeSettings(localRoot, {
      active_interview_source: 'local_model',
      local_model: { endpoint: 'http://127.0.0.1:11434', model: 'llama3.2:3b' },
      byok_frontier: null,
    });
    check('reads local_model', resolveActiveInterviewSource(localRoot) === 'local_model');

    // Field genuinely absent from the settings file (a pre-existing settings
    // file written before this field existed) must default to server_fallback,
    // not throw — mirrors the Rust side's #[serde(default)] guarantee.
    const missingFieldRoot = tempRoot('interview-active-source-missing-field');
    writeSettings(missingFieldRoot, { active_source: 'local_model', local_model: null, byok_frontier: null });
    check(
      'defaults to server_fallback when the field is absent from an old settings file',
      resolveActiveInterviewSource(missingFieldRoot) === 'server_fallback',
    );
  }

  console.log(failures === 0 ? '\nAll checks passed.' : `\n${failures} check(s) failed.`);
  process.exit(failures === 0 ? 0 : 1);
}

run();
