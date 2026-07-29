---
description: Type-check the frontend and sidecar
---

Type-check both TypeScript surfaces in this repo.

Steps:
1. Run `npx tsc --noEmit` in the project root — checks `src/` (frontend).
2. Run `npx tsc --noEmit` in `sidecar/` — checks `sidecar/src/`.
3. Report errors grouped by file with path:line. If both pass with zero errors, confirm clean.

Note: there is no `tsconfig` covering both together — they're separate npm packages
(`skillshome-desktop` frontend vs `skillshome-desktop-sidecar`), run them separately.
