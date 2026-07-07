/**
 * run-local-extraction.ts — Module 4 tasks 4.3c + 4.9.
 *
 * Proves, with no Tauri/Rust process-spawning involved:
 *  1. @menporulalar/agents-core installs from GitHub Packages into this
 *     separate repo and its agent classes run correctly outside skillshome-app.
 *  2. The resulting shape (skills/experience/projects/credentials arrays)
 *     matches what ReviewPackageAssemblerAgent expects as candidate input on
 *     the server side (Requirement 3.6).
 *  3. (4.9) The real Extraction_Source setting — configured and activated via the
 *     desktop app's settings screen (4.4-4.8) — actually drives which provider
 *     this hits, via resolveExtractionConfig() reading the same
 *     extraction_settings.json file the Rust side writes.
 *
 * `runLocalExtraction()` is also reused by run-local-extraction-and-stage.ts
 * (task 4.11), which additionally syncs the result to the server over MCP.
 *
 * Invoking this from the Rust shell via Tauri's sidecar mechanism (rather than
 * running it directly with node/ts-node, as this script still does) is task
 * 4.12's job — deliberately out of scope here.
 *
 * Usage: npm run extract:sample -- <path-to-a-resume-file>
 *        (set BYOK_API_KEY=<key> first if Extraction_Source is byok_frontier)
 */
import { readFileSync } from 'node:fs';
import { extname } from 'node:path';
import {
  TextExtractorAgent,
  SkillExtractorAgent,
  ExperienceParserAgent,
  ProjectParserAgent,
  CredentialsParserAgent,
  type IngestionInputType,
  type ExtractedSkill,
  type ExtractedExperience,
  type ExtractedProject,
  type ExtractedEducation,
  type ExtractedCertificate,
  type ExtractedAccolade,
} from '@menporulalar/agents-core';
import { resolveExtractionConfig } from './resolveExtractionConfig';

const EXTENSION_TO_INPUT_TYPE: Record<string, IngestionInputType> = {
  '.pdf': 'resume_pdf',
  '.docx': 'resume_docx',
  '.txt': 'resume_txt',
  '.md': 'resume_md',
  '.json': 'linkedin_json',
};

export interface LocalExtractionResult {
  inputType: IngestionInputType;
  skills: ExtractedSkill[];
  experience: ExtractedExperience[];
  projects: ExtractedProject[];
  education: ExtractedEducation[];
  certificates: ExtractedCertificate[];
  accolades: ExtractedAccolade[];
  summary: string;
}

export async function runLocalExtraction(filePath: string): Promise<LocalExtractionResult> {
  const inputType = EXTENSION_TO_INPUT_TYPE[extname(filePath).toLowerCase()];
  if (!inputType) {
    throw new Error(
      `Unrecognized file extension for "${filePath}" — expected one of: ${Object.keys(EXTENSION_TO_INPUT_TYPE).join(', ')}`,
    );
  }

  const fileBytes = readFileSync(filePath);
  const llmConfig = resolveExtractionConfig();

  const textExtractor = new TextExtractorAgent();
  const { rawText } = await textExtractor.run({ inputType, fileBytes });

  const [skillResult, experienceResult, projectResult, credentialsResult] = await Promise.all([
    new SkillExtractorAgent().run({ rawText, profileId: 'local-sample', llmConfig }),
    new ExperienceParserAgent().run({ rawText, llmConfig }),
    new ProjectParserAgent().run({ rawText, llmConfig }),
    new CredentialsParserAgent().run({ rawText, llmConfig }),
  ]);

  return {
    inputType,
    skills: skillResult.skills,
    experience: experienceResult.experience,
    projects: projectResult.projects,
    education: credentialsResult.education,
    certificates: credentialsResult.certificates,
    accolades: credentialsResult.accolades,
    summary: credentialsResult.summary,
  };
}

async function main() {
  const filePath = process.argv[2];
  if (!filePath) {
    console.error('Usage: npm run extract:sample -- <path-to-a-resume-file>');
    process.exit(1);
  }

  const { inputType: _inputType, ...result } = await runLocalExtraction(filePath);
  console.log(JSON.stringify(result, null, 2));
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
