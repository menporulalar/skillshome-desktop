/**
 * run-interview-opening-question.ts — ai-interview-agent Module 4 task 4.3's
 * other half: session *creation* (`POST /api/interview/start`) always
 * generates its opening question server-side, same as every other client —
 * there's no MCP tool for "start a session" (Requirement 9's tool surface is
 * exactly the two turn/read tools). When Interview_Source is Local_Model/
 * BYOK_Frontier, this regenerates just the opening question's *text* locally
 * afterward, exactly like run-interview-turn.ts does for follow-ups — the
 * server's own opening question is simply discarded client-side, never shown.
 * Bank-question openings are never regenerated (see run-interview-turn.ts's
 * header for why).
 *
 * Usage: npm run interview-opening -- <trackName> <difficulty>
 */
import { resolveActiveInterviewSource, resolveInterviewLoopConfig } from './resolveInterviewLoopConfig';
import { generateOpeningQuestion } from './generateInterviewQuestion';

const RESULT_MARKER = '__SIDECAR_RESULT__:';

function printResult(result: Record<string, unknown>) {
  console.log(`${RESULT_MARKER}${JSON.stringify(result)}`);
}

async function main() {
  const [trackName, difficultyArg] = process.argv.slice(2);
  if (!trackName || !difficultyArg) {
    console.error('Usage: npm run interview-opening -- <trackName> <difficulty>');
    process.exit(1);
  }
  const difficulty = Number(difficultyArg);

  const activeSource = resolveActiveInterviewSource();
  if (activeSource === 'server_fallback') {
    printResult({ ok: false, error: 'active_interview_source is server_fallback — nothing to regenerate' });
    process.exit(1);
  }

  try {
    const llmConfig = resolveInterviewLoopConfig();
    const text = await generateOpeningQuestion(llmConfig, trackName, difficulty);
    printResult({ ok: true, text });
  } catch (err) {
    printResult({ ok: false, error: err instanceof Error ? err.message : String(err) });
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}
