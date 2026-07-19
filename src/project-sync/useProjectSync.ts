import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Mirrors src-tauri/src/projectsync/mod.rs's `ConnectedProjectView` (row fields
// are serde-flattened into the same object).
export interface ConnectedProjectView {
  id: string;
  sourceType: string;
  localPathLabel: string | null;
  githubRepoFullName: string | null;
  agentConfigScanEnabled: boolean;
  status: string;
  lastSignalHash: string | null;
  lastSyncedAt: string | null;
  local_path: string | null;
  last_scan_at_ms: number | null;
  stale_on_open: boolean;
}

export interface SyncOutcome {
  ok: boolean;
  outcome?: "staged" | "unchanged" | "no_signal";
  jobId?: string;
  skillCount?: number;
  error?: string;
}

// Spec task 3.8 cadence: on-open catch-up scan for anything >24h stale, plus a
// periodic check while the app stays running so no grant goes >1 week unscanned.
// The staleness decisions themselves live (unit-tested) in Rust — this hook only
// decides how often to ASK.
const RUNNING_CHECK_INTERVAL_MS = 6 * 3600 * 1000;

export function useProjectSync(profileId: string | null) {
  const [projects, setProjects] = useState<ConnectedProjectView[]>([]);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  // One scheduler pass per mount; StrictMode double-effects must not double-scan.
  const scheduledRef = useRef(false);

  const refresh = useCallback(async () => {
    if (!profileId) return [] as ConnectedProjectView[];
    const rows = await invoke<ConnectedProjectView[]>("list_connected_projects", { profileId });
    setProjects(rows);
    return rows;
  }, [profileId]);

  const syncOne = useCallback(
    async (project: ConnectedProjectView): Promise<SyncOutcome> => {
      const result = await invoke<SyncOutcome>("run_project_sync", {
        connectedProjectId: project.id,
        profileId,
        lastSignalHash: project.lastSignalHash,
        agentConfigScanEnabled: project.agentConfigScanEnabled,
      });
      return result;
    },
    [profileId],
  );

  /** Scan every local project whose grant is stale under `threshold`. */
  const runScheduledScans = useCallback(
    async (threshold: "on_open" | "weekly") => {
      if (!profileId) return;
      const stale = await invoke<Array<{ connected_project_id: string }>>(
        "list_stale_project_grants",
        { threshold },
      );
      if (stale.length === 0) return;
      const rows = await refresh();
      let staged = 0;
      for (const grant of stale) {
        const project = rows.find((p) => p.id === grant.connected_project_id && p.status === "active");
        if (!project) continue;
        try {
          const result = await syncOne(project);
          if (result.outcome === "staged") staged++;
        } catch (err) {
          // One project's failure never blocks the rest (Requirement 1.5 spirit).
          console.error("scheduled project sync failed", project.id, err);
        }
      }
      if (staged > 0) {
        setNotice(
          `${staged} project scan${staged === 1 ? "" : "s"} staged new skills — review them to update your profile.`,
        );
      }
      await refresh();
    },
    [profileId, refresh, syncOne],
  );

  // On-open catch-up + keep-alive weekly check while the app runs.
  useEffect(() => {
    if (!profileId || scheduledRef.current) return;
    scheduledRef.current = true;
    void refresh().catch(() => undefined);
    void runScheduledScans("on_open").catch(() => undefined);
    const handle = setInterval(() => {
      void runScheduledScans("weekly").catch(() => undefined);
    }, RUNNING_CHECK_INTERVAL_MS);
    return () => clearInterval(handle);
  }, [profileId, refresh, runScheduledScans]);

  const pickFolder = useCallback(async (): Promise<string | null> => {
    return (await invoke<string | null>("pick_project_folder")) ?? null;
  }, []);

  const connect = useCallback(
    async (input: { folderPath: string; label: string; consentConfirmed: boolean; agentConfigScanEnabled: boolean }) => {
      setBusy(true);
      setErrorMessage(null);
      try {
        await invoke("connect_local_project", {
          profileId,
          folderPath: input.folderPath,
          label: input.label,
          consentConfirmed: input.consentConfirmed,
          agentConfigScanEnabled: input.agentConfigScanEnabled,
        });
        await refresh();
      } catch (err) {
        setErrorMessage(String(err));
        throw err;
      } finally {
        setBusy(false);
      }
    },
    [profileId, refresh],
  );

  const remove = useCallback(
    async (connectedProjectId: string) => {
      setBusy(true);
      setErrorMessage(null);
      try {
        await invoke("remove_connected_project", { profileId, connectedProjectId });
        await refresh();
      } catch (err) {
        setErrorMessage(String(err));
      } finally {
        setBusy(false);
      }
    },
    [profileId, refresh],
  );

  const syncNow = useCallback(
    async (project: ConnectedProjectView) => {
      setBusy(true);
      setErrorMessage(null);
      setNotice(null);
      try {
        const result = await syncOne(project);
        if (result.outcome === "staged") {
          setNotice(`Staged ${result.skillCount ?? 0} skill(s) for review (job ${result.jobId ?? "?"}).`);
        } else if (result.outcome === "unchanged") {
          setNotice("No changes since the last confirmed scan.");
        } else {
          setNotice("No recognisable project signal found in this folder.");
        }
        await refresh();
      } catch (err) {
        setErrorMessage(String(err));
      } finally {
        setBusy(false);
      }
    },
    [refresh, syncOne],
  );

  return { projects, busy, notice, errorMessage, refresh, pickFolder, connect, remove, syncNow };
}

/**
 * Renders nothing — mounted once at the App root (signed-in tree) so the
 * spec-task-3.8 cadence (on-open catch-up + running-app weekly check) runs
 * even if the Connected-Projects screen is never opened. v1 limitation,
 * deliberate: schedules against the account's FIRST profile (multi-profile
 * users trigger scans for other profiles from the screen's "Sync now").
 */
export function ProjectSyncScheduler() {
  const [profileId, setProfileId] = useState<string | null>(null);
  useEffect(() => {
    void invoke<Array<{ id: string }>>("list_my_profiles")
      .then((rows) => setProfileId(rows[0]?.id ?? null))
      .catch(() => undefined);
  }, []);
  useProjectSync(profileId);
  return null;
}
