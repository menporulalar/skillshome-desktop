/**
 * src/__tests__/mcpClient.test.ts — task 4.11: unit coverage for the one pure,
 * genuinely-testable piece of mcpClient.ts (parseMcpToolErrorReason). Deliberately
 * NOT mocking the full MCP SDK transport/handshake here — low-value and brittle
 * (would mostly test the mock, not the real integration); real coverage for
 * connectMcpClient/stageIngestion/confirmIngestion comes from live verification
 * against the real server instead (see the approved plan's Verification section).
 *
 * Run with: npm test (from sidecar/)
 */
import { parseMcpToolErrorReason } from '../mcpClient';

let failures = 0;
function check(name: string, cond: boolean) {
  if (cond) console.log(`  ✓ ${name}`);
  else { console.error(`  ✗ ${name}`); failures++; }
}

function run() {
  console.log('parseMcpToolErrorReason');

  const budgetText = `MCP error -32013: ${JSON.stringify({ reason: 'budget_exceeded', message: 'Daily ingestion limit reached.' })}`;
  const budgetParsed = parseMcpToolErrorReason(budgetText);
  check('parses a budget_exceeded error, stripping the MCP error prefix', budgetParsed?.reason === 'budget_exceeded');
  check('preserves the original message', budgetParsed?.message === 'Daily ingestion limit reached.');

  const concurrentText = `MCP error -32014: ${JSON.stringify({ reason: 'concurrent_job', message: 'A job is already in progress.' })}`;
  const concurrentParsed = parseMcpToolErrorReason(concurrentText);
  check('parses a concurrent_job error', concurrentParsed?.reason === 'concurrent_job');

  check(
    'returns null for plain prose (e.g. auth/ownership/rate-limit errors)',
    parseMcpToolErrorReason('MCP error -32011: Forbidden — you do not have access to this profile') === null,
  );

  check('returns null for malformed JSON, never throws', parseMcpToolErrorReason('MCP error -32000: {not valid json') === null);

  check(
    'returns null for valid JSON missing reason/message fields',
    parseMcpToolErrorReason('MCP error -32000: {"unrelated":"field"}') === null,
  );

  console.log(failures === 0 ? '\nAll checks passed.' : `\n${failures} check(s) failed.`);
  process.exit(failures === 0 ? 0 : 1);
}

run();
