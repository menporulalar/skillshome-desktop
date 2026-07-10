import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ExtractionSource } from "../extraction/useExtractionSettings";
import type { ProfileSummary, IngestStatusResponse, IngestSource } from "./useServerFallbackIngest";
import { mapDesktopError } from "../errors/mapDesktopError";

// "Source picker" here means WHAT data to extract from (file / LinkedIn export /
// GitHub URL) — a different axis from Extraction_Source (WHERE extraction runs,
// chosen on the settings screen, tasks 4.4-4.8).
//
// File-based input (résumé PDF/DOCX/TXT/MD, LinkedIn export JSON) covers both
// Extraction_Source paths. URL-based input (GitHub repo/doc link) is
// Server_Fallback-only per Requirement 3.1/3.2 — Local_Model/BYOK_Frontier have
// no local GitHub-scanning agent in this spec's scope, so the toggle below only
// appears when `activeSource === "server_fallback"`.
interface Props {
  pickFile: () => Promise<string | null>;
  listProfiles: () => Promise<ProfileSummary[]>;
  activeSource: ExtractionSource;
  onBack: () => void;
  onStart: (profileId: string, source: IngestSource) => void;
  onReviewReady: (profileId: string, reviewPackage: unknown) => void;
}

type BlockState = { kind: "checking" } | { kind: "clear" } | { kind: "blocked"; label: string };
type InputMode = "file" | "url";

export function SourcePickerScreen({ pickFile, listProfiles, activeSource, onBack, onStart, onReviewReady }: Props) {
  const [profiles, setProfiles] = useState<ProfileSummary[] | null>(null);
  const [profileId, setProfileId] = useState<string | null>(null);
  const [filePath, setFilePath] = useState<string | null>(null);
  const [inputMode, setInputMode] = useState<InputMode>("file");
  const [url, setUrl] = useState("");
  const [block, setBlock] = useState<BlockState>({ kind: "checking" });
  const [loadError, setLoadError] = useState<string | null>(null);

  const urlInputAllowed = activeSource === "server_fallback";

  useEffect(() => {
    listProfiles()
      .then((result) => {
        setProfiles(result);
        // Free tier is capped at 1 profile — auto-select when there's exactly one,
        // matching the same assumption task 4.10's REST wiring already relies on.
        if (result.length === 1) setProfileId(result[0].id);
      })
      .catch((err) => setLoadError(String(err)));
  }, [listProfiles]);

  // Requirement 6.4: block starting a second extraction while one is already
  // pending. `profile.ingest.stage` (MCP) and `POST .../ingest` (REST) both write
  // to the same IngestionJob table, so this single REST-status check is a valid
  // cross-path source of truth regardless of which path started the pending job.
  useEffect(() => {
    if (!profileId) return;
    setBlock({ kind: "checking" });
    invoke<IngestStatusResponse | null>("get_server_fallback_ingest_status", { profileId })
      .then((status) => {
        if (!status) {
          setBlock({ kind: "clear" });
        } else if (status.status === "pending" || status.status === "processing") {
          setBlock({ kind: "blocked", label: status.status_label });
        } else if (status.status === "awaiting_review" && status.review_package) {
          onReviewReady(profileId, status.review_package);
        } else {
          setBlock({ kind: "clear" });
        }
      })
      .catch((err) => setLoadError(String(err)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profileId]);

  const handlePickFile = async () => {
    const picked = await pickFile();
    if (picked) setFilePath(picked);
  };

  const canStart =
    Boolean(profileId) &&
    block.kind === "clear" &&
    (inputMode === "url" ? url.trim().length > 0 : Boolean(filePath));

  const handleStart = () => {
    if (!profileId) return;
    const source: IngestSource =
      inputMode === "url" ? { kind: "url", url: url.trim() } : { kind: "file", path: filePath as string };
    onStart(profileId, source);
  };

  return (
    <main className="container">
      <h1>Start Extraction</h1>

      {loadError && <p style={{ color: "red" }}>{mapDesktopError(loadError)}</p>}

      {profiles && profiles.length === 0 && (
        <p>No profiles yet — create one on the SkillsHome web app first.</p>
      )}

      {profiles && profiles.length > 1 && (
        <div style={{ textAlign: "left", maxWidth: 420, margin: "0 auto" }}>
          <label>
            Profile
            <select value={profileId ?? ""} onChange={(e) => setProfileId(e.currentTarget.value)} style={{ width: "100%" }}>
              <option value="" disabled>Choose a profile</option>
              {profiles.map((p) => (
                <option key={p.id} value={p.id}>{p.display_name}</option>
              ))}
            </select>
          </label>
        </div>
      )}

      {profileId && block.kind === "blocked" && (
        <p style={{ color: "#a15c00" }}>
          An extraction is already in progress for this profile ({block.label}). Wait for it to finish before starting another.
        </p>
      )}

      {profileId && block.kind === "clear" && (
        <div style={{ textAlign: "left", maxWidth: 420, margin: "0 auto" }}>
          {urlInputAllowed && (
            <div className="row" style={{ gap: "1em", marginBottom: "0.75em" }}>
              <label>
                <input
                  type="radio"
                  checked={inputMode === "file"}
                  onChange={() => setInputMode("file")}
                />
                {" "}File
              </label>
              <label>
                <input
                  type="radio"
                  checked={inputMode === "url"}
                  onChange={() => setInputMode("url")}
                />
                {" "}GitHub / doc URL
              </label>
            </div>
          )}

          {inputMode === "file" ? (
            <>
              <div className="row" style={{ gap: "0.5em", marginBottom: "0.5em" }}>
                <button type="button" onClick={handlePickFile}>
                  {filePath ? "Change file" : "Choose résumé / LinkedIn export"}
                </button>
              </div>
              {filePath && <p style={{ fontSize: "0.85em", opacity: 0.8 }}>{filePath}</p>}
            </>
          ) : (
            <label>
              GitHub repo or doc URL
              <input
                type="url"
                value={url}
                onChange={(e) => setUrl(e.currentTarget.value)}
                placeholder="https://github.com/owner/repo"
                style={{ width: "100%" }}
              />
            </label>
          )}

          <div className="row" style={{ marginTop: "1em" }}>
            <button type="button" disabled={!canStart} onClick={handleStart}>
              Start Extraction
            </button>
          </div>
        </div>
      )}

      <button type="button" onClick={onBack} style={{ marginTop: "2em" }}>
        Back
      </button>
    </main>
  );
}
