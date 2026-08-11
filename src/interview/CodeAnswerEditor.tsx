import CodeMirror from "@uiw/react-codemirror";
import { python } from "@codemirror/lang-python";
import { javascript } from "@codemirror/lang-javascript";
import { Prec, type Extension } from "@codemirror/state";
import { keymap } from "@codemirror/view";

interface Props {
  value: string;
  onChange: (value: string) => void;
  /**
   * The server's language for a `coding_challenge` question. Undefined when the
   * candidate opened the editor themselves on a prose question, or when the
   * backend predates the field — the editor still works, just unhighlighted.
   */
  language?: string | null;
  /** Fallback signal for the manual-toggle case, where the server sent no language. */
  trackName: string;
  onSubmit: () => void;
}

/**
 * The code-answer half of the Mock_Interview answer field. Shown automatically
 * when the server says `kind === 'coding_challenge'`, and otherwise available
 * behind the screen's manual toggle. Default-exported and imported lazily by
 * MockInterviewScreen so the CodeMirror bundle is only fetched when a code
 * answer is actually being written.
 *
 * Nothing here changes how the answer is graded: it is submitted as the same
 * plain string a textarea would produce, to the same server-side grader
 * (Requirement 6.1). This is an input affordance, not a code-evaluation feature.
 */
export default function CodeAnswerEditor({ value, onChange, language, trackName, onSubmit }: Props) {
  return (
    <CodeMirror
      value={value}
      onChange={onChange}
      height="220px"
      theme="dark"
      extensions={[
        ...languageExtensions(language, trackName),
        // Highest precedence so it wins over CodeMirror's own Enter handling,
        // which would otherwise insert a newline and swallow the shortcut.
        Prec.highest(
          keymap.of([
            {
              // "Mod" is Cmd on macOS and Ctrl elsewhere — matches the prose
              // field's shortcut without a platform check here.
              key: "Mod-Enter",
              run: () => {
                onSubmit();
                return true;
              },
            },
          ]),
        ),
      ]}
      basicSetup={{
        // A short interview answer is not a file: line numbers and the fold
        // gutter are noise at this size.
        lineNumbers: false,
        foldGutter: false,
        highlightActiveLine: false,
        autocompletion: false,
      }}
    />
  );
}

/**
 * Prefers the server's `language` — it is authoritative, comes from the same
 * allow-list the question was planned against, and is exact rather than a
 * substring match. The trackName guess remains only for the manual-toggle case,
 * where there is no code question and so no language to be told.
 *
 * An unrecognised value still gets the editor, just without language-aware
 * highlighting — the honest outcome rather than guessing wrong.
 */
function languageExtensions(language: string | null | undefined, trackName: string): Extension[] {
  if (language) {
    const named = language.toLowerCase();
    if (named === "python") return [python()];
    if (named === "javascript" || named === "typescript") {
      return [javascript({ typescript: named === "typescript" })];
    }
    // A known-but-unsupported language (e.g. 'sql') deliberately falls through
    // to no highlighting rather than to the trackName guess, which would be
    // less accurate than what the server just told us.
    return [];
  }

  const track = trackName.toLowerCase();
  if (track.includes("python")) return [python()];
  if (["javascript", "typescript", "react", "node"].some((name) => track.includes(name))) {
    return [javascript({ typescript: track.includes("typescript") })];
  }
  return [];
}
