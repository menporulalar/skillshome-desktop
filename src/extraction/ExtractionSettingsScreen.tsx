import { useEffect, useState } from "react";
import { useExtractionSettings, type ByokProvider, type ExtractionSource } from "./useExtractionSettings";
import { LocalModelDisclaimerBanner } from "./LocalModelDisclaimerBanner";

// Client-side mirror of src-tauri/src/extraction/check.rs's `is_loopback_host` — this
// is UI feedback only. The real gate (Requirement 10.2) is enforced server-side in
// `save_local_model_config`, which rejects a non-loopback endpoint without opt-in
// regardless of what this check shows.
function isLoopbackHost(host: string): boolean {
  return host === "127.0.0.1" || host === "localhost" || host === "::1";
}

function hostOf(endpoint: string): string | null {
  try {
    return new URL(endpoint).hostname;
  } catch {
    return null;
  }
}

interface Props {
  onBack: () => void;
}

export function ExtractionSettingsScreen({ onBack }: Props) {
  const { settings, testResult, busy, saveLocalModel, saveByok, testLocalModel, testByok, activate, deleteLocalModel, deleteByok } =
    useExtractionSettings();

  const [selected, setSelected] = useState<ExtractionSource>("server_fallback");

  const [endpoint, setEndpoint] = useState("http://127.0.0.1:11434");
  const [localModelName, setLocalModelName] = useState("");
  const [nonLoopbackOptIn, setNonLoopbackOptIn] = useState(false);

  const [provider, setProvider] = useState<ByokProvider>("openai");
  const [byokModelName, setByokModelName] = useState("");
  const [apiKey, setApiKey] = useState("");

  useEffect(() => {
    if (!settings) return;
    setSelected(settings.active_source);
    if (settings.local_model) {
      setEndpoint(settings.local_model.endpoint);
      setLocalModelName(settings.local_model.model);
      setNonLoopbackOptIn(settings.local_model.non_loopback_opt_in);
    }
    if (settings.byok_frontier) {
      setProvider(settings.byok_frontier.provider);
      setByokModelName(settings.byok_frontier.model);
    }
  }, [settings]);

  const endpointHost = hostOf(endpoint);
  const needsOptIn = endpointHost !== null && !isLoopbackHost(endpointHost) && !nonLoopbackOptIn;

  // A saved BYOK config implies a key in the OS keychain (save_byok_config writes
  // both together), so a blank key field doesn't mean "no key" — the backend treats
  // blank as "keep/use the stored key".
  const hasStoredByokKey = Boolean(settings?.byok_frontier);

  const testAndSaveLocalModel = async () => {
    try {
      await saveLocalModel(endpoint, localModelName, nonLoopbackOptIn);
    } catch {
      return; // the hook already surfaced the save error
    }
    await testLocalModel(endpoint, localModelName);
  };

  const testAndSaveByok = async () => {
    try {
      await saveByok(provider, byokModelName, apiKey);
    } catch {
      return;
    }
    await testByok(provider, apiKey, byokModelName);
  };

  // "Activate this source" must persist the form first — activating only a
  // previously-saved config silently discards whatever the user just typed
  // (the original "my key and settings weren't persisted" bug).
  const saveAndActivateLocalModel = async () => {
    try {
      await saveLocalModel(endpoint, localModelName, nonLoopbackOptIn);
      await activate("local_model");
    } catch {
      // save/activate errors already land in testResult
    }
  };

  const saveAndActivateByok = async () => {
    try {
      await saveByok(provider, byokModelName, apiKey);
      await activate("byok_frontier");
    } catch {
      // save/activate errors already land in testResult
    }
  };

  return (
    <main className="container">
      <h1>Extraction Settings</h1>
      <p>Choose how SkillsHome Desktop extracts skills and experience from your résumé.</p>

      {settings?.active_source === "local_model" && <LocalModelDisclaimerBanner />}

      <div className="row" style={{ gap: "1em", marginBottom: "1.5em" }}>
        <button
          type="button"
          onClick={() => setSelected("local_model")}
          style={{ fontWeight: selected === "local_model" ? 700 : 400 }}
        >
          Local Model{settings?.active_source === "local_model" ? " (active)" : ""}
        </button>
        <button
          type="button"
          onClick={() => setSelected("byok_frontier")}
          style={{ fontWeight: selected === "byok_frontier" ? 700 : 400 }}
        >
          Bring Your Own Key{settings?.active_source === "byok_frontier" ? " (active)" : ""}
        </button>
        <button
          type="button"
          onClick={() => setSelected("server_fallback")}
          style={{ fontSize: "0.9em", opacity: 0.8 }}
        >
          Use SkillsHome's servers{settings?.active_source === "server_fallback" ? " (active)" : ""}
        </button>
      </div>

      {selected === "local_model" && (
        <div style={{ textAlign: "left", maxWidth: 420, margin: "0 auto" }}>
          <label>
            Endpoint
            <input
              value={endpoint}
              onChange={(e) => setEndpoint(e.currentTarget.value)}
              placeholder="http://127.0.0.1:11434"
              style={{ width: "100%" }}
            />
          </label>
          <label>
            Model name
            <input
              value={localModelName}
              onChange={(e) => setLocalModelName(e.currentTarget.value)}
              placeholder="llama3.2:3b"
              style={{ width: "100%" }}
            />
          </label>

          {endpointHost !== null && !isLoopbackHost(endpointHost) && (
            <div style={{ color: "#a15c00", margin: "0.5em 0" }}>
              <p>
                <strong>"{endpointHost}" is not a loopback address.</strong> Connecting to a
                non-local endpoint sends your résumé text there directly.
              </p>
              <label>
                <input
                  type="checkbox"
                  checked={nonLoopbackOptIn}
                  onChange={(e) => setNonLoopbackOptIn(e.currentTarget.checked)}
                />
                {" "}I understand the risk and want to use this endpoint anyway
              </label>
            </div>
          )}

          <div className="row" style={{ gap: "0.5em", marginTop: "1em" }}>
            <button type="button" onClick={testAndSaveLocalModel} disabled={busy || needsOptIn}>
              {busy ? "Testing…" : "Test Connection"}
            </button>
            <button type="button" onClick={saveAndActivateLocalModel} disabled={busy || needsOptIn}>
              Activate this source
            </button>
            {settings?.local_model && (
              <button type="button" onClick={deleteLocalModel} disabled={busy}>
                Remove
              </button>
            )}
          </div>
        </div>
      )}

      {selected === "byok_frontier" && (
        <div style={{ textAlign: "left", maxWidth: 420, margin: "0 auto" }}>
          <label>
            Provider
            <select value={provider} onChange={(e) => setProvider(e.currentTarget.value as ByokProvider)} style={{ width: "100%" }}>
              <option value="openai">OpenAI</option>
              <option value="anthropic">Anthropic</option>
              <option value="openrouter">OpenRouter</option>
            </select>
          </label>
          <label>
            Model name
            <input
              value={byokModelName}
              onChange={(e) => setByokModelName(e.currentTarget.value)}
              placeholder="gpt-4o-mini"
              style={{ width: "100%" }}
            />
          </label>
          <label>
            API key
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.currentTarget.value)}
              style={{ width: "100%" }}
            />
          </label>
          <p style={{ fontSize: "0.85em", opacity: 0.8 }}>
            Stored only in your OS keychain — never sent to SkillsHome's servers.
          </p>
          {hasStoredByokKey && !apiKey && (
            <p style={{ fontSize: "0.85em", opacity: 0.8 }}>
              A key is already saved in your keychain — leave this field blank to keep using it.
            </p>
          )}

          <div className="row" style={{ gap: "0.5em", marginTop: "1em" }}>
            <button type="button" onClick={testAndSaveByok} disabled={busy || (!apiKey && !hasStoredByokKey)}>
              {busy ? "Testing…" : "Test Connection"}
            </button>
            <button type="button" onClick={saveAndActivateByok} disabled={busy || (!apiKey && !hasStoredByokKey)}>
              Activate this source
            </button>
            {settings?.byok_frontier && (
              <button type="button" onClick={deleteByok} disabled={busy}>
                Remove
              </button>
            )}
          </div>
        </div>
      )}

      {selected === "server_fallback" && (
        <div style={{ textAlign: "left", maxWidth: 420, margin: "0 auto" }}>
          <p>No local pipeline runs — extraction happens on SkillsHome's servers, same as the web app.</p>
          <button type="button" onClick={() => activate("server_fallback")} disabled={busy}>
            Activate this source
          </button>
        </div>
      )}

      {testResult && (
        <p style={{ color: testResult.ok ? "green" : "red", marginTop: "1em" }}>{testResult.message}</p>
      )}

      <button type="button" onClick={onBack} style={{ marginTop: "2em" }}>
        Back
      </button>
    </main>
  );
}
