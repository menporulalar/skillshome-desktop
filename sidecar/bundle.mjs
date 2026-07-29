// Bundles the sidecar's stable entry points (task 4.12 script names — sidecar.rs
// spawns these exact names, do not rename without updating the Rust caller) into
// single-file CommonJS output for release builds, where there's no npm/ts-node/
// node_modules checkout to run against (only a bundled Node binary + these files).
import { build } from "esbuild";

const entries = {
  stage: "src/run-local-extraction-and-stage.ts",
  confirm: "src/confirm-staged-ingestion.ts",
  "project-sync": "src/run-project-sync.ts",
  "interview-opening": "src/run-interview-opening-question.ts",
  "interview-turn": "src/run-interview-turn.ts",
};

for (const [name, entry] of Object.entries(entries)) {
  await build({
    entryPoints: [entry],
    outfile: `dist/${name}.cjs`,
    bundle: true,
    platform: "node",
    format: "cjs",
    target: "node20",
    // agents-core's TextExtractorAgent treats this as an optional runtime dep
    // (try/catch around the require, throws a clear error if missing) — not
    // installed today even in dev mode, so leave it unresolved rather than
    // have esbuild fail the build trying to statically bundle it.
    external: ["unzipper"],
  });
}
