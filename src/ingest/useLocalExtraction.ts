import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// The Local_Model/BYOK_Frontier counterpart to useServerFallbackIngest.ts's REST
// hook — mirrors its shape (busy/errorMessage state, similarly-named actions) so
// screens can treat both paths fairly uniformly, branching only on which hook's
// function to call. No polling here (unlike the REST hook): `start_local_extraction_and_stage`
// is a single blocking Tauri command (dev-mode sidecar spawn, task 4.12) with no
// incremental progress signal, not a pollable server-side job.
export interface LocalStageResult {
  jobId: string;
  reviewPackage: unknown;
}

// Task 4.13 (Requirement 3.5): an outer retry loop around the whole sidecar
// invocation, matching packages/agents-core's BaseAgent.run()'s own backoff
// formula exactly (1000 * 2^(attempt-1), capped at 15s) — "matching the existing
// worker's retry policy" per the requirement's wording. This is deliberately at
// the desktop-app level, not inside agents-core itself: llmCaller.ts classifies
// an unreachable Ollama/MLX endpoint as retriable but an unreachable
// OpenAI/OpenRouter endpoint as fatal (skips BaseAgent's own retry entirely) — an
// outer retry here covers a whole failed attempt uniformly regardless of why it
// failed, without touching agents-core (a shared package also used by the
// server-side worker, out of this desktop-spec module's scope).
const MAX_ATTEMPTS = 3;

function backoffDelayMs(attempt: number): number {
  return Math.min(1000 * 2 ** (attempt - 1), 15_000);
}

export function useLocalExtraction() {
  const [busy, setBusy] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  const startAndStage = useCallback(async (profileId: string, filePath: string): Promise<LocalStageResult> => {
    setBusy(true);
    setErrorMessage(null);
    let lastError: unknown;

    for (let i = 1; i <= MAX_ATTEMPTS; i++) {
      setAttempt(i);
      try {
        const result = await invoke<LocalStageResult>("start_local_extraction_and_stage", { profileId, filePath });
        setBusy(false);
        return result;
      } catch (err) {
        lastError = err;
        if (i < MAX_ATTEMPTS) {
          await new Promise((resolve) => setTimeout(resolve, backoffDelayMs(i)));
        }
      }
    }

    setErrorMessage(String(lastError));
    setBusy(false);
    throw lastError;
  }, []);

  const confirm = useCallback(async (profileId: string, confirmedItems: unknown) => {
    setBusy(true);
    setErrorMessage(null);
    try {
      await invoke("confirm_local_extraction", { profileId, confirmedItems });
    } catch (err) {
      setErrorMessage(String(err));
      throw err;
    } finally {
      setBusy(false);
    }
  }, []);

  return { busy, errorMessage, attempt, maxAttempts: MAX_ATTEMPTS, startAndStage, confirm };
}
