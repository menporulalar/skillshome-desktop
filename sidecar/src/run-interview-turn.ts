/**
 * run-interview-turn.ts — ai-interview-agent Module 4 task 4.3: one interview
 * turn from the desktop app. Grading is ALWAYS server-side (Requirement 6.1) —
 * this script only ever calls `interview.turn.propose` for that. The only
 * thing Interview_Source changes is who writes the *next* question's text:
 * when Local_Model/BYOK_Frontier is active, this replaces the server's
 * already-generated next-question text with one generated locally, using the
 * same track/recent-turn context read back via `interview.session.read`.
 * Only `conversational` slots are regenerated. Bank-question slots are not —
 * the question bank is fixed, server-side, deterministic (Requirement 6.2's
 * grader-type split), so there is nothing for a local model to usefully
 * generate there. `coding_challenge` slots are not either, and that one is
 * load-bearing rather than merely pointless: the server authors the code task
 * and grades the answer against its code rubric, so swapping in a locally
 * generated task would have the candidate answer one question and be graded
 * against another.
 *
 * Usage: npm run interview-turn -- <sessionId> <answerText> [proposedScore]
 *        SKILLSHOME_ACCESS_TOKEN/SKILLSHOME_BACKEND_URL must be set (same
 *        contract as every other sidecar script — see run-local-extraction-
 *        and-stage.ts's header for why progress goes to stderr).
 */
import { connectMcpClient, proposeInterviewTurn, readInterviewSession, type ProposedTurnResult } from './mcpClient';
import { resolveActiveInterviewSource, resolveInterviewLoopConfig } from './resolveInterviewLoopConfig';
import { generateFollowUpQuestion } from './generateInterviewQuestion';

const RESULT_MARKER = '__SIDECAR_RESULT__:';

function printResult(result: Record<string, unknown>) {
  console.log(`${RESULT_MARKER}${JSON.stringify(result)}`);
}

export async function runInterviewTurn(
  sessionId: string,
  answerText: string,
  proposedScore: number | undefined,
  backendUrl: string,
  accessToken: string,
): Promise<ProposedTurnResult> {
  const client = await connectMcpClient(backendUrl, accessToken);
  try {
    const result = await proposeInterviewTurn(client, sessionId, answerText, proposedScore);

    if (result.status === 'in_progress' && result.question && result.question.kind === 'conversational') {
      const activeSource = resolveActiveInterviewSource();
      if (activeSource !== 'server_fallback') {
        console.error(`[interview-turn] generating next question locally via ${activeSource}`);
        const session = await readInterviewSession(client, sessionId);
        const track = session.tracks.find((t) => t.name === result.question!.trackName);
        const llmConfig = resolveInterviewLoopConfig();
        const localText = await generateFollowUpQuestion(
          llmConfig,
          result.question.trackName,
          track?.difficulty ?? 3,
          track?.turns ?? [],
          result.turn.score,
        );
        result.question = { ...result.question, text: localText };
      }
    }

    return result;
  } finally {
    await client.close().catch(() => undefined);
  }
}

async function main() {
  const [sessionId, answerText, proposedScoreArg] = process.argv.slice(2);
  if (!sessionId || !answerText) {
    console.error('Usage: npm run interview-turn -- <sessionId> <answerText> [proposedScore]');
    process.exit(1);
  }
  const accessToken = process.env.SKILLSHOME_ACCESS_TOKEN;
  if (!accessToken) {
    printResult({ ok: false, error: 'SKILLSHOME_ACCESS_TOKEN is not set' });
    process.exit(1);
  }
  const backendUrl = process.env.SKILLSHOME_BACKEND_URL ?? 'http://localhost:3000';
  const proposedScore = proposedScoreArg != null && proposedScoreArg !== '-' ? Number(proposedScoreArg) : undefined;

  try {
    const result = await runInterviewTurn(sessionId, answerText, proposedScore, backendUrl, accessToken!);
    printResult({ ok: true, ...result });
  } catch (err) {
    printResult({ ok: false, error: err instanceof Error ? err.message : String(err) });
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}
