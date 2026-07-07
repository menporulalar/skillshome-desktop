/**
 * resolveExtractionConfig.ts — Module 4 task 4.9: reads the Extraction_Source
 * settings the desktop app's Rust side writes and turns them into an LLMCallConfig
 * for @menporulalar/agents-core's callLLMForExtraction().
 *
 * Reads settingsRoot/extraction_settings.json directly off disk — no Tauri/Rust IPC
 * involved, since it's a plain JSON file the Rust side already produces. Mirrors
 * `ExtractionSettings`/`LocalModelConfig`/`ByokFrontierConfig`/`ByokProvider` in
 * src-tauri/src/extraction/settings.rs field-for-field; `ByokProvider`'s serde
 * renames already serialize as lowercase "openai"/"anthropic"/"openrouter", matching
 * llmCaller.ts's expected provider strings directly — no translation needed.
 *
 * Deliberately does NOT spawn/receive anything from Rust — that's Tauri sidecar
 * process wiring (tasks 4.9-4.11's cross-repo counterpart, out of scope here per the
 * approved plan). `BYOK_API_KEY` as a plain env var here stands in for how a
 * Rust-spawned child process would pass it later: a child-process env var, never over
 * a network, satisfying Requirement 10.1 even in this pre-spawning form.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { homedir, platform } from 'node:os';
import type { LLMCallConfig } from '@menporulalar/agents-core';

type ExtractionSource = 'server_fallback' | 'local_model' | 'byok_frontier';
type ByokProvider = 'openai' | 'anthropic' | 'openrouter';

interface ExtractionSettings {
  active_source: ExtractionSource;
  local_model: { endpoint: string; model: string } | null;
  byok_frontier: { provider: ByokProvider; model: string } | null;
}

/** Not a stored setting on either side — matches the sidecar proof script's
 *  pre-existing hardcoded value. */
const MAX_TOKENS = 4096;

/** Mirrors Tauri's own `app.path().app_data_dir()` resolution (`dirs::data_dir()`
 *  joined with the app's identifier, `com.skillshome.desktop`) — reads the exact
 *  file the real desktop app writes, not a copy. Exposed for tests, which point it
 *  at an injected temp directory instead of the real OS path. */
export function appDataDir(): string {
  const appId = 'com.skillshome.desktop';
  switch (platform()) {
    case 'darwin':
      return join(homedir(), 'Library', 'Application Support', appId);
    case 'win32':
      return join(process.env.APPDATA ?? join(homedir(), 'AppData', 'Roaming'), appId);
    default:
      return join(process.env.XDG_DATA_HOME ?? join(homedir(), '.local', 'share'), appId);
  }
}

export function resolveExtractionConfig(root: string = appDataDir()): LLMCallConfig {
  const settingsPath = join(root, 'extraction_settings.json');
  const settings = JSON.parse(readFileSync(settingsPath, 'utf-8')) as ExtractionSettings;

  if (settings.active_source === 'local_model') {
    if (!settings.local_model) {
      throw new Error('active_source is local_model but no local_model config is saved');
    }
    return {
      provider: 'ollama', // Local_Model always speaks the OpenAI-compatible shape llmCaller.ts's ollama branch already knows
      model: settings.local_model.model,
      maxTokens: MAX_TOKENS,
      baseURL: settings.local_model.endpoint,
    };
  }

  if (settings.active_source === 'byok_frontier') {
    if (!settings.byok_frontier) {
      throw new Error('active_source is byok_frontier but no byok_frontier config is saved');
    }
    const apiKey = process.env.BYOK_API_KEY;
    if (!apiKey) {
      throw new Error('BYOK_API_KEY env var is required when active_source is byok_frontier');
    }
    return {
      provider: settings.byok_frontier.provider,
      model: settings.byok_frontier.model,
      maxTokens: MAX_TOKENS,
      apiKey,
    };
  }

  throw new Error('active_source is server_fallback — no local pipeline should run for this source');
}
