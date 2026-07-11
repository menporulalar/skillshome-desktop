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

export function useLocalExtraction() {
  const [busy, setBusy] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const startAndStage = useCallback(async (profileId: string, filePath: string): Promise<LocalStageResult> => {
    setBusy(true);
    setErrorMessage(null);
    try {
      return await invoke<LocalStageResult>("start_local_extraction_and_stage", { profileId, filePath });
    } catch (err) {
      setErrorMessage(String(err));
      throw err;
    } finally {
      setBusy(false);
    }
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

  return { busy, errorMessage, startAndStage, confirm };
}
