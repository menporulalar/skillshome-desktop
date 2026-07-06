import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Mirrors src-tauri/src/extraction/settings.rs's `ExtractionSettings`. ByokProvider
// uses explicit serde renames (not derived snake_case, which would mangle "OpenAI"
// into "open_a_i") — keep these three literals in sync with that file's `#[serde(rename = ...)]`s.
export type ExtractionSource = "server_fallback" | "local_model" | "byok_frontier";
export type ByokProvider = "openai" | "anthropic" | "openrouter";

export interface LocalModelConfig {
  endpoint: string;
  model: string;
  non_loopback_opt_in: boolean;
  connectivity_check_passed: boolean;
}

export interface ByokFrontierConfig {
  provider: ByokProvider;
  model: string;
  connectivity_check_passed: boolean;
}

export interface ExtractionSettings {
  active_source: ExtractionSource;
  local_model: LocalModelConfig | null;
  byok_frontier: ByokFrontierConfig | null;
}

export interface TestResult {
  ok: boolean;
  message: string;
}

export function useExtractionSettings() {
  const [settings, setSettings] = useState<ExtractionSettings | null>(null);
  const [testResult, setTestResult] = useState<TestResult | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    setSettings(await invoke<ExtractionSettings>("get_extraction_settings"));
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const saveLocalModel = useCallback(
    async (endpoint: string, model: string, nonLoopbackOptIn: boolean) => {
      await invoke("save_local_model_config", { endpoint, model, nonLoopbackOptIn });
      setTestResult(null);
      await refresh();
    },
    [refresh],
  );

  const saveByok = useCallback(
    async (provider: ByokProvider, model: string, apiKey: string) => {
      await invoke("save_byok_config", { provider, model, apiKey });
      setTestResult(null);
      await refresh();
    },
    [refresh],
  );

  const testLocalModel = useCallback(async (endpoint: string, model: string) => {
    setBusy(true);
    try {
      await invoke("test_local_model_connection", { endpoint, model });
      setTestResult({ ok: true, message: "Connection succeeded." });
    } catch (err) {
      setTestResult({ ok: false, message: String(err) });
    } finally {
      setBusy(false);
    }
  }, []);

  const testByok = useCallback(async (provider: ByokProvider, apiKey: string, model: string) => {
    setBusy(true);
    try {
      await invoke("test_byok_connection", { provider, apiKey, model });
      setTestResult({ ok: true, message: "Connection succeeded." });
    } catch (err) {
      setTestResult({ ok: false, message: String(err) });
    } finally {
      setBusy(false);
    }
  }, []);

  const activate = useCallback(
    async (source: ExtractionSource) => {
      setBusy(true);
      try {
        await invoke("activate_extraction_source", { source });
        setTestResult(null);
        await refresh();
      } catch (err) {
        setTestResult({ ok: false, message: String(err) });
        throw err;
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  const deleteLocalModel = useCallback(async () => {
    await invoke("delete_local_model_config");
    await refresh();
  }, [refresh]);

  const deleteByok = useCallback(async () => {
    await invoke("delete_byok_config");
    await refresh();
  }, [refresh]);

  return {
    settings,
    testResult,
    busy,
    saveLocalModel,
    saveByok,
    testLocalModel,
    testByok,
    activate,
    deleteLocalModel,
    deleteByok,
  };
}
