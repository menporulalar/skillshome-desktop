import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useProjectSync, type ConnectedProjectView } from "./useProjectSync";

interface ProfileSummary {
  id: string;
  display_name: string;
}

// #25 Module 3 desktop half — the "Connected projects" list (Requirement 1.4):
// every connected source in one place, each individually removable, plus the
// local-folder connect flow with per-project affirmative consent (Requirement
// 1.3). The real folder path is only ever shown/stored locally; the server
// receives the display label alone.

interface Props {
  onBack: () => void;
}

function describeSource(p: ConnectedProjectView): string {
  if (p.sourceType === "github") return p.githubRepoFullName ?? "GitHub repository";
  return p.localPathLabel ?? "Local folder";
}

function lastScanLabel(p: ConnectedProjectView): string {
  if (p.sourceType !== "local") return p.lastSyncedAt ? `server-synced ${new Date(p.lastSyncedAt).toLocaleDateString()}` : "server-scheduled";
  if (!p.last_scan_at_ms) return "never scanned";
  return `scanned ${new Date(p.last_scan_at_ms).toLocaleString()}`;
}

export function ConnectedProjectsScreen({ onBack }: Props) {
  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  const [profileId, setProfileId] = useState<string | null>(null);
  useEffect(() => {
    void invoke<ProfileSummary[]>("list_my_profiles").then((rows) => {
      setProfiles(rows);
      setProfileId((current) => current ?? rows[0]?.id ?? null);
    });
  }, []);

  const { projects, busy, notice, errorMessage, pickFolder, connect, remove, syncNow } =
    useProjectSync(profileId);

  const [folderPath, setFolderPath] = useState<string | null>(null);
  const [label, setLabel] = useState("");
  const [consent, setConsent] = useState(false);
  const [agentConfig, setAgentConfig] = useState(false);

  const chooseFolder = async () => {
    const picked = await pickFolder();
    if (picked) {
      setFolderPath(picked);
      // Default label = folder name; user-editable, and the only part the server sees.
      const parts = picked.split(/[\\/]/).filter(Boolean);
      setLabel(parts[parts.length - 1] ?? "my-project");
    }
  };

  const submit = async () => {
    if (!folderPath || !consent) return;
    try {
      await connect({
        folderPath,
        label: label.trim() || "my-project",
        consentConfirmed: consent,
        agentConfigScanEnabled: agentConfig,
      });
      setFolderPath(null);
      setLabel("");
      setConsent(false);
      setAgentConfig(false);
    } catch {
      // errorMessage is already surfaced by the hook.
    }
  };

  return (
    <main className="container">
      <h1>Connected projects</h1>
      <p>
        Keep your profile current from the projects you actually work on. Only dependency
        manifests, README, and CI config are ever read — never your source code — and every
        scan result waits for your review before it touches your profile.
      </p>

      {profiles.length > 1 && (
        <div className="row" style={{ gap: "0.5em", alignItems: "center" }}>
          <label htmlFor="project-profile">Profile:</label>
          <select id="project-profile" value={profileId ?? ""} onChange={(e) => setProfileId(e.target.value)}>
            {profiles.map((p) => (
              <option key={p.id} value={p.id}>{p.display_name}</option>
            ))}
          </select>
        </div>
      )}

      {notice && <p style={{ color: "#4a7c52" }}>{notice}</p>}
      {errorMessage && <p style={{ color: "#b3403f" }}>{errorMessage}</p>}

      <section>
        <h2>Your projects</h2>
        {projects.length === 0 && <p>No connected projects yet.</p>}
        {projects.map((p) => (
          <div key={p.id} className="row" style={{ gap: "0.5em", alignItems: "center", marginBottom: "0.5em" }}>
            <span style={{ minWidth: 0, flex: 1 }}>
              <strong>{describeSource(p)}</strong>{" "}
              <small>
                ({p.sourceType}
                {p.agentConfigScanEnabled ? " · agent-config on" : ""} · {p.status} · {lastScanLabel(p)})
              </small>
              {p.sourceType === "local" && p.local_path && (
                <small style={{ display: "block", opacity: 0.7 }}>{p.local_path} (stays on this device)</small>
              )}
              {p.sourceType === "local" && !p.local_path && (
                <small style={{ display: "block", opacity: 0.7 }}>
                  connected from another device — no folder grant on this machine
                </small>
              )}
            </span>
            {p.sourceType === "local" && p.local_path && p.status === "active" && (
              <button type="button" disabled={busy} onClick={() => void syncNow(p)}>
                Sync now
              </button>
            )}
            <button type="button" disabled={busy} onClick={() => void remove(p.id)}>
              Remove
            </button>
          </div>
        ))}
      </section>

      <section>
        <h2>Connect a local folder</h2>
        <div className="row" style={{ gap: "0.5em", alignItems: "center" }}>
          <button type="button" disabled={busy} onClick={() => void chooseFolder()}>
            Choose folder…
          </button>
          {folderPath && <code>{folderPath}</code>}
        </div>

        {folderPath && (
          <>
            <div className="row" style={{ gap: "0.5em", alignItems: "center", marginTop: "0.5em" }}>
              <label htmlFor="project-label">Display name (all the server ever sees):</label>
              <input
                id="project-label"
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                maxLength={120}
              />
            </div>
            <label className="row" style={{ gap: "0.5em", marginTop: "0.5em" }}>
              <input type="checkbox" checked={consent} onChange={(e) => setConsent(e.target.checked)} />
              I confirm I have the right to grant access to this project.
            </label>
            <label className="row" style={{ gap: "0.5em" }}>
              <input
                type="checkbox"
                checked={agentConfig}
                onChange={(e) => setAgentConfig(e.target.checked)}
              />
              Also scan AI-tooling config (CLAUDE.md, AGENTS.md, .cursor/rules, MCP configs) — optional.
            </label>
            <button type="button" disabled={busy || !consent} onClick={() => void submit()} style={{ marginTop: "0.5em" }}>
              Connect project
            </button>
          </>
        )}
      </section>

      <button type="button" onClick={onBack} style={{ marginTop: "1em" }}>
        Back
      </button>
    </main>
  );
}
