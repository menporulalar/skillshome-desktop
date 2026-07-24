/**
 * resolveInterviewLoopConfig.ts — ai-interview-agent Module 4 task 4.3: reads the
 * Interview_Source settings the desktop app's Rust side writes
 * (`ExtractionSettings.active_interview_source`, Requirement 5.1) and turns them
 * into an LLMCallConfig for @menporulalar/agents-core's callLLMForExtraction() —
 * the exact same shared caller resolveExtractionConfig.ts already uses, not a
 * second provider-calling implementation. Interview_Source is a SEPARATE active
 * source from Extraction_Source, but reuses the identical local_model/byok_frontier
 * config + BYOK_API_KEY env var convention (Requirement 5.1's "reuse... verbatim").
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { LLMCallConfig } from '@menporulalar/agents-core';
import { appDataDir } from './resolveExtractionConfig';

export type InterviewSource = 'server_fallback' | 'local_model' | 'byok_frontier';
type ByokProvider = 'openai' | 'anthropic' | 'openrouter';

interface ExtractionSettings {
  active_interview_source: InterviewSource;
  local_model: { endpoint: string; model: string } | null;
  byok_frontier: { provider: ByokProvider; model: string } | null;
}

const MAX_TOKENS = 300; // a single interview question, not a full extraction payload

export function resolveActiveInterviewSource(root: string = appDataDir()): InterviewSource {
  const settingsPath = join(root, 'extraction_settings.json');
  const settings = JSON.parse(readFileSync(settingsPath, 'utf-8')) as ExtractionSettings;
  return settings.active_interview_source ?? 'server_fallback';
}

export function resolveInterviewLoopConfig(root: string = appDataDir()): LLMCallConfig {
  const settingsPath = join(root, 'extraction_settings.json');
  const settings = JSON.parse(readFileSync(settingsPath, 'utf-8')) as ExtractionSettings;

  if (settings.active_interview_source === 'local_model') {
    if (!settings.local_model) {
      throw new Error('active_interview_source is local_model but no local_model config is saved');
    }
    return {
      provider: 'ollama',
      model: settings.local_model.model,
      maxTokens: MAX_TOKENS,
      baseURL: settings.local_model.endpoint,
    };
  }

  if (settings.active_interview_source === 'byok_frontier') {
    if (!settings.byok_frontier) {
      throw new Error('active_interview_source is byok_frontier but no byok_frontier config is saved');
    }
    const apiKey = process.env.BYOK_API_KEY;
    if (!apiKey) {
      throw new Error('BYOK_API_KEY env var is required when active_interview_source is byok_frontier');
    }
    return {
      provider: settings.byok_frontier.provider,
      model: settings.byok_frontier.model,
      maxTokens: MAX_TOKENS,
      apiKey,
    };
  }

  throw new Error('active_interview_source is server_fallback — no local question generation should run for this source');
}
