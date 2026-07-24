import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface RoleTemplateSummary {
  id: string;
  name: string;
}

export interface ClientQuestion {
  text: string;
  trackName: string;
  kind: string;
  options?: string[];
  perTurnSeconds: number;
}

interface StartInterviewResponse {
  sessionId: string;
  roleName: string;
  question: ClientQuestion;
  plannedTurns: number;
}

interface FormativeTrackReport {
  skillName: string;
  status: string;
  turnCount: number;
  averageScore: number | null;
  finalDifficulty: number;
}

export interface FormativeReport {
  formative: true;
  overallScore: number | null;
  turnCount: number;
  flags: { distressFlag: boolean; gamingFlag: boolean };
  perTrack: FormativeTrackReport[];
}

interface TurnSidecarResult {
  ok: boolean;
  error?: string;
  turn?: { score: number | null; feedback?: string; reason?: string };
  status?: "in_progress" | "complete" | "paused_offer";
  question?: ClientQuestion | null;
  report?: FormativeReport | null;
}

type Phase = "picking" | "in_progress" | "complete" | "error";

interface State {
  phase: Phase;
  sessionId: string | null;
  roleName: string | null;
  question: ClientQuestion | null;
  lastFeedback: string | null;
  report: FormativeReport | null;
  error: string | null;
}

const initialState: State = {
  phase: "picking",
  sessionId: null,
  roleName: null,
  question: null,
  lastFeedback: null,
  report: null,
  error: null,
};

/**
 * ai-interview-agent Module 4 — desktop Mock_Interview flow. Session
 * creation and turn submission both call real Tauri commands
 * (src-tauri/src/interview/mod.rs), which round-trip to skillshome-app's
 * REST route (start) and the interview.turn.propose MCP tool (each turn) —
 * grading is always server-side (Requirement 6.1) regardless of which
 * Interview_Source is active; only question *text* generation differs.
 */
export function useMockInterview() {
  const [state, setState] = useState<State>(initialState);
  const [busy, setBusy] = useState(false);

  const listRoleTemplates = useCallback(async (): Promise<RoleTemplateSummary[]> => {
    return invoke<RoleTemplateSummary[]>("list_role_templates");
  }, []);

  const start = useCallback(async (profileId: string, roleTemplateId: string) => {
    setBusy(true);
    setState((s) => ({ ...s, error: null }));
    try {
      const started = await invoke<StartInterviewResponse>("start_mock_interview", {
        profileId,
        roleTemplateId,
      });
      setState({
        phase: "in_progress",
        sessionId: started.sessionId,
        roleName: started.roleName,
        question: started.question,
        lastFeedback: null,
        report: null,
        error: null,
      });
    } catch (err) {
      setState((s) => ({ ...s, phase: "error", error: String(err) }));
    } finally {
      setBusy(false);
    }
  }, []);

  const submitAnswer = useCallback(
    async (answerText: string) => {
      if (!state.sessionId) return;
      setBusy(true);
      try {
        const result = await invoke<TurnSidecarResult>("submit_interview_turn", {
          sessionId: state.sessionId,
          answerText,
        });
        if (!result.ok) {
          setState((s) => ({ ...s, phase: "error", error: result.error ?? "Turn submission failed" }));
          return;
        }
        if (result.status === "complete" || !result.question) {
          setState((s) => ({
            ...s,
            phase: "complete",
            question: null,
            lastFeedback: result.turn?.feedback ?? null,
            report: result.report ?? null,
          }));
        } else {
          setState((s) => ({
            ...s,
            phase: "in_progress",
            question: result.question ?? null,
            lastFeedback: result.turn?.feedback ?? null,
          }));
        }
      } catch (err) {
        setState((s) => ({ ...s, phase: "error", error: String(err) }));
      } finally {
        setBusy(false);
      }
    },
    [state.sessionId],
  );

  const reset = useCallback(() => setState(initialState), []);

  return { ...state, busy, listRoleTemplates, start, submitAnswer, reset };
}
