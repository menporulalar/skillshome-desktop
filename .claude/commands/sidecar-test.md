---
description: Run the sidecar test suite (ts-node based, no Jest)
---

Run the Node sidecar's tests.

Steps:
1. Run `npm test` in `sidecar/`.
2. This chains four ts-node invocations directly (see `sidecar/package.json`'s `test` script) —
   `resolveExtractionConfig.test.ts`, `resolveInterviewLoopConfig.test.ts`, `mcpClient.test.ts`,
   `runProjectSync.test.ts`. There is no Jest/Vitest runner; a failure in one script still lets
   npm move to try the next only if you re-run individually — the chained `&&` command stops at
   the first failure. Re-run the specific `ts-node ... src/__tests__/<name>.test.ts` command to
   isolate a failure.
3. Report pass/fail per file, not just the overall exit code.
