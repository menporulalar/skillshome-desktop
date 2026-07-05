/**
 * run-local-extraction.ts — Module 4 task 4.3c minimal proof.
 *
 * Proves two things end to end, with no Tauri/Rust involvement at all:
 *  1. @menporulalar/agents-core installs from GitHub Packages into this
 *     separate repo and its agent classes run correctly outside skillshome-app.
 *  2. The resulting shape (skills/experience/projects/credentials arrays)
 *     matches what ReviewPackageAssemblerAgent expects as candidate input on
 *     the server side (Requirement 3.6).
 *
 * The real Extraction_Source-driven config (Local_Model / BYOK_Frontier
 * settings UI, connectivity self-check) is tasks 4.4-4.9, not this one — the
 * llmConfig below is a hardcoded stand-in pointed at a local Ollama.
 * Likewise, invoking this from the Rust shell via Tauri's sidecar mechanism
 * is tasks 4.9-4.11 — this only proves the Node-side logic works when run
 * directly with node/ts-node.
 *
 * Usage: npm run extract:sample -- <path-to-a-resume-file>
 */
import { readFileSync } from 'node:fs';
import { extname } from 'node:path';
import {
  TextExtractorAgent,
  SkillExtractorAgent,
  ExperienceParserAgent,
  ProjectParserAgent,
  CredentialsParserAgent,
  type LLMCallConfig,
  type IngestionInputType,
} from '@menporulalar/agents-core';

const EXTENSION_TO_INPUT_TYPE: Record<string, IngestionInputType> = {
  '.pdf': 'resume_pdf',
  '.docx': 'resume_docx',
  '.txt': 'resume_txt',
  '.md': 'resume_md',
  '.json': 'linkedin_json',
};

async function main() {
  const filePath = process.argv[2];
  if (!filePath) {
    console.error('Usage: npm run extract:sample -- <path-to-a-resume-file>');
    process.exit(1);
  }

  const inputType = EXTENSION_TO_INPUT_TYPE[extname(filePath).toLowerCase()];
  if (!inputType) {
    console.error(`Unrecognized file extension for "${filePath}" — expected one of: ${Object.keys(EXTENSION_TO_INPUT_TYPE).join(', ')}`);
    process.exit(1);
  }

  const fileBytes = readFileSync(filePath);

  // Hardcoded stand-in for the real Extraction_Source-driven resolution
  // (tasks 4.4-4.9) — points at a local Ollama, same default the docker-compose
  // dev stack uses.
  const llmConfig: LLMCallConfig = {
    provider: 'ollama',
    model: process.env.OLLAMA_MODEL ?? 'llama3.2:3b',
    maxTokens: 4096,
  };

  const textExtractor = new TextExtractorAgent();
  const { rawText } = await textExtractor.run({ inputType, fileBytes });

  const [skillResult, experienceResult, projectResult, credentialsResult] = await Promise.all([
    new SkillExtractorAgent().run({ rawText, profileId: 'local-sample', llmConfig }),
    new ExperienceParserAgent().run({ rawText, llmConfig }),
    new ProjectParserAgent().run({ rawText, llmConfig }),
    new CredentialsParserAgent().run({ rawText, llmConfig }),
  ]);

  console.log(JSON.stringify({
    skills: skillResult.skills,
    experience: experienceResult.experience,
    projects: projectResult.projects,
    education: credentialsResult.education,
    certificates: credentialsResult.certificates,
    accolades: credentialsResult.accolades,
    summary: credentialsResult.summary,
  }, null, 2));
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
