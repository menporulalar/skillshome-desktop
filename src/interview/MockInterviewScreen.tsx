import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMockInterview, type RoleTemplateSummary } from "./useMockInterview";

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

  const handleRestart = () => {
    reset();
    setAnswer("");
  };

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
          {roleName && <p style={{ color: "#6e8b92", fontSize: "0.85em" }}>{roleName} — {question.trackName}</p>}
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
              <textarea
                value={answer}
                onChange={(e) => setAnswer(e.currentTarget.value)}
                rows={6}
                style={{ width: "100%" }}
                placeholder="Type your answer…"
              />
              <button type="button" onClick={handleSubmit} disabled={busy || !answer.trim()} style={{ marginTop: "0.5em" }}>
                {busy ? "Submitting…" : "Submit answer"}
              </button>
            </>
          )}
        </div>
      )}

      {phase === "complete" && report && (
        <div style={{ textAlign: "left", maxWidth: 480, margin: "0 auto" }}>
          <p style={{ fontSize: "1.5em", fontWeight: 700 }}>
            {report.overallScore != null ? `${report.overallScore}%` : "Not scored"}
          </p>
          <p style={{ color: "#6e8b92" }}>{report.turnCount} turn(s) answered</p>
          {(report.flags.distressFlag || report.flags.gamingFlag) && (
            <p style={{ color: "#a15c00" }}>
              {report.flags.distressFlag && "A pause was offered during this session. "}
              {report.flags.gamingFlag && "Some answers were flagged for review."}
            </p>
          )}
          <div style={{ display: "grid", gap: "0.5em", marginTop: "1em" }}>
            {report.perTrack.map((t) => (
              <div key={t.skillName} className="row" style={{ justifyContent: "space-between" }}>
                <span>{t.skillName}</span>
                <span style={{ color: "#6e8b92" }}>
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
          <p style={{ color: "#b3403f" }}>{error}</p>
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
