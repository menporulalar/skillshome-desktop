import CodeMirror from "@uiw/react-codemirror";
import { python } from "@codemirror/lang-python";
import { javascript } from "@codemirror/lang-javascript";
import { Prec, type Extension } from "@codemirror/state";
import { keymap } from "@codemirror/view";

interface Props {
  value: string;
  onChange: (value: string) => void;
  trackName: string;
  onSubmit: () => void;
}

/**
 * The code-answer half of the Mock_Interview answer field, behind the screen's
 * "Code answer" toggle. Default-exported and imported lazily by
 * MockInterviewScreen so the CodeMirror bundle is only fetched once someone
 * actually turns the toggle on — prose answers, which are the entire question
 * bank today, never load it.
 *
 * Nothing here changes how the answer is graded: it is submitted as the same
 * plain string a textarea would produce, to the same server-side grader
 * (Requirement 6.1). This is an input affordance, not a code-evaluation feature.
 */
export default function CodeAnswerEditor({ value, onChange, trackName, onSubmit }: Props) {
  return (
    <CodeMirror
      value={value}
      onChange={onChange}
      height="220px"
      theme="dark"
      extensions={[
        ...languageExtensions(trackName),
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
 * The server sends no language with a question — there is no code question kind
 * in the contract at all — so the track name is the only signal available.
 * An unrecognised track still gets the editor, just without language-aware
 * highlighting, which is the honest outcome rather than guessing wrong.
 */
function languageExtensions(trackName: string): Extension[] {
  const track = trackName.toLowerCase();
  if (track.includes("python")) return [python()];
  if (["javascript", "typescript", "react", "node"].some((name) => track.includes(name))) {
    return [javascript({ typescript: track.includes("typescript") })];
  }
  return [];
}
