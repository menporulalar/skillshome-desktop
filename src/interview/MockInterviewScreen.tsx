import { Suspense, lazy, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMockInterview, type RoleTemplateSummary } from "./useMockInterview";
import { LIGHT_THEME } from "../../brand/theme/tokens";

// Lazy so the CodeMirror bundle is fetched only when someone turns on the "Code
// answer" toggle. Every question in the bank today is prose, so for almost all
// sessions this never loads at all.
const CodeAnswerEditor = lazy(() => import("./CodeAnswerEditor"));

interface ProfileSummary {
  id: string;
  display_name: string;
}

interface Props {
  onBack: () => void;
}

/**
 * Role templates are unique on `(name, level)`, so the name alone does not
 * identify one — "AI Engineer" exists at several levels and renders as a run of
 * identical-looking rows without the level appended.
 */
function roleTemplateLabel({ name, level }: RoleTemplateSummary): string {
  if (!level) return name;
  return `${name} — ${level.charAt(0).toUpperCase()}${level.slice(1)}`;
}

/**
 * ai-interview-agent Module 4 desktop half — the Mock_Interview taking
 * screen. Note (task 4.4): there is no Employer_Interview-starting flow
 * anywhere in this app — `start_mock_interview` always creates a
 * `interviewType: 'mock'` session server-side — so the Interview_Source
 * picker in ExtractionSettingsScreen can never apply to anything but
 * Mock_Interview by construction, with nothing extra to restrict here.
 */
export function MockInterviewScreen({ onBack }: Props) {
  const {
    phase,
    roleName,
    question,
    lastFeedback,
    report,
    error,
    busy,
    listRoleTemplates,
    start,
    submitAnswer,
    reset,
  } = useMockInterview();

  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  const [profileId, setProfileId] = useState<string | null>(null);
  const [roleTemplates, setRoleTemplates] = useState<RoleTemplateSummary[]>([]);
  const [roleTemplateId, setRoleTemplateId] = useState<string | null>(null);
  const [answer, setAnswer] = useState("");
  // Sticky across turns: someone answering one question in code is likely to
  // answer the next one that way too. Reset only when the whole session is.
  const [codeMode, setCodeMode] = useState(false);

  useEffect(() => {
    void invoke<ProfileSummary[]>("list_my_profiles").then((rows) => {
      setProfiles(rows);
      setProfileId((current) => current ?? rows[0]?.id ?? null);
    });
    void listRoleTemplates().then((rows) => {
      setRoleTemplates(rows);
      setRoleTemplateId((current) => current ?? rows[0]?.id ?? null);
    });
  }, [listRoleTemplates]);

  const handleStart = async () => {
    if (!profileId || !roleTemplateId) return;
    await start(profileId, roleTemplateId);
  };

  const handleSubmit = async () => {
    if (!answer.trim()) return;
    const submitted = answer;
    setAnswer("");
    await submitAnswer(submitted);
  };

  // Deliberately not a Tab handler: Tab must keep moving focus out of a prose
  // field. Code answers get real indentation from CodeMirror instead.
  const handleAnswerKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      // The submit button is disabled while busy; this path has to re-check it.
      if (!busy) void handleSubmit();
    }
  };

  // Shared by both input modes, so the shortcut can't submit an empty or
  // in-flight answer from inside the editor's own keymap.
  const submitFromShortcut = () => {
    if (!busy) void handleSubmit();
  };

  const handleRestart = () => {
    reset();
    setAnswer("");
    setCodeMode(false);
  };

  // The server decides this now: a 'coding_challenge' slot is a question that
  // must be answered in code, and it is graded against the code rubric whatever
  // the candidate types. The manual toggle stays for prose questions, whose
  // answers may still contain a snippet.
  const isCodeQuestion = question?.kind === "coding_challenge";
  const showEditor = isCodeQuestion || codeMode;

  return (
    <main className="container">
      <h1>Mock Interview</h1>

      {phase === "picking" && (
        <div style={{ textAlign: "left", maxWidth: 420, margin: "0 auto" }}>
          <p>Practise against a role's required skills. Every answer is graded server-side.</p>

          {profiles.length > 1 && (
            <label>
              Profile
              <select value={profileId ?? ""} onChange={(e) => setProfileId(e.target.value)} style={{ width: "100%" }}>
                {profiles.map((p) => (
                  <option key={p.id} value={p.id}>{p.display_name}</option>
                ))}
              </select>
            </label>
          )}

          <label>
            Role
            <select value={roleTemplateId ?? ""} onChange={(e) => setRoleTemplateId(e.target.value)} style={{ width: "100%" }}>
              {roleTemplates.map((r) => (
                <option key={r.id} value={r.id}>{roleTemplateLabel(r)}</option>
              ))}
            </select>
          </label>

          <button type="button" onClick={handleStart} disabled={busy || !profileId || !roleTemplateId} style={{ marginTop: "1em" }}>
            {busy ? "Starting…" : "Start interview"}
          </button>
        </div>
      )}

      {phase === "in_progress" && question && (
        <div style={{ textAlign: "left", maxWidth: 480, margin: "0 auto" }}>
          {roleName && <p style={{ color: LIGHT_THEME.ink3, fontSize: "0.85em" }}>{roleName} — {question.trackName}</p>}
          {lastFeedback && <p style={{ fontStyle: "italic" }}>{lastFeedback}</p>}
          <p style={{ fontWeight: 600 }}>{question.text}</p>

          {question.options && question.options.length > 0 ? (
            <div className="row" style={{ flexDirection: "column", gap: "0.5em" }}>
              {question.options.map((opt) => (
                <button key={opt} type="button" onClick={() => void submitAnswer(opt)} disabled={busy} style={{ textAlign: "left" }}>
                  {opt}
                </button>
              ))}
            </div>
          ) : (
            <>
              {isCodeQuestion ? (
                <p style={{ fontSize: "0.8em", color: LIGHT_THEME.ink3, marginBottom: "0.4em" }}>
                  Code answer{question.language ? ` · ${question.language}` : ""}
                </p>
              ) : (
                // Offered only on a prose question, where the candidate may still
                // want to include a snippet. On a code question the server has
                // already said so, and letting them switch it off would just be a
                // worse field for a question that has to be answered in code.
                <label style={{ display: "flex", alignItems: "center", gap: "0.4em", fontSize: "0.8em", color: LIGHT_THEME.ink3, marginBottom: "0.4em" }}>
                  <input type="checkbox" checked={codeMode} onChange={(e) => setCodeMode(e.currentTarget.checked)} />
                  Code answer
                </label>
              )}

              {showEditor ? (
                // The fallback is sized to match the editor so the layout doesn't
                // jump on first toggle while the chunk loads.
                <Suspense fallback={<div style={{ height: 220, color: LIGHT_THEME.ink3, fontSize: "0.8em" }}>Loading editor…</div>}>
                  <CodeAnswerEditor
                    value={answer}
                    onChange={setAnswer}
                    language={question.language}
                    trackName={question.trackName}
                    onSubmit={submitFromShortcut}
                  />
                </Suspense>
              ) : (
                <textarea
                  value={answer}
                  onChange={(e) => setAnswer(e.currentTarget.value)}
                  onKeyDown={handleAnswerKeyDown}
                  rows={8}
                  style={{ width: "100%" }}
                  placeholder="Type your answer…"
                />
              )}

              <div className="row" style={{ justifyContent: "space-between", alignItems: "center", marginTop: "0.5em" }}>
                <button type="button" onClick={handleSubmit} disabled={busy || !answer.trim()}>
                  {busy ? "Submitting…" : "Submit answer"}
                </button>
                <span style={{ color: LIGHT_THEME.ink3, fontSize: "0.75em" }}>⌘/Ctrl + Enter submits</span>
              </div>
            </>
          )}
        </div>
      )}

      {phase === "complete" && report && (
        <div style={{ textAlign: "left", maxWidth: 480, margin: "0 auto" }}>
          <p style={{ fontSize: "1.5em", fontWeight: 700 }}>
            {report.overallScore != null ? `${report.overallScore}%` : "Not scored"}
          </p>
          <p style={{ color: LIGHT_THEME.ink3 }}>{report.turnCount} turn(s) answered</p>
          {(report.flags.distressFlag || report.flags.gamingFlag) && (
            <p style={{ color: LIGHT_THEME.warning }}>
              {report.flags.distressFlag && "A pause was offered during this session. "}
              {report.flags.gamingFlag && "Some answers were flagged for review."}
            </p>
          )}
          <div style={{ display: "grid", gap: "0.5em", marginTop: "1em" }}>
            {report.perTrack.map((t) => (
              <div key={t.skillName} className="row" style={{ justifyContent: "space-between" }}>
                <span>{t.skillName}</span>
                <span style={{ color: LIGHT_THEME.ink3 }}>
                  {t.averageScore != null ? `${Math.round(t.averageScore)}%` : "—"} · {t.turnCount} turn(s)
                </span>
              </div>
            ))}
          </div>
          <button type="button" onClick={handleRestart} style={{ marginTop: "1em" }}>
            Start another
          </button>
        </div>
      )}

      {phase === "error" && (
        <div style={{ textAlign: "left", maxWidth: 480, margin: "0 auto" }}>
          <p style={{ color: LIGHT_THEME.error }}>{error}</p>
          <button type="button" onClick={handleRestart}>
            Try again
          </button>
        </div>
      )}

      <button type="button" onClick={onBack} style={{ marginTop: "2em" }}>
        Back
      </button>
    </main>
  );
}
