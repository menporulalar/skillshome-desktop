// The signed-in landing screen (task 4.12) — extracted from SigninScreen.tsx's
// former signed-in branch, which now only handles the signed-out view.
interface Props {
  onStartExtraction: () => void;
  onOpenSettings: () => void;
  signOut: () => void;
}

export function HomeScreen({ onStartExtraction, onOpenSettings, signOut }: Props) {
  return (
    <main className="container">
      <h1>Signed in</h1>
      <p>SkillsHome Desktop is connected.</p>
      <div className="row" style={{ gap: "0.5em" }}>
        <button type="button" onClick={onStartExtraction}>
          Start Extraction
        </button>
        <button type="button" onClick={onOpenSettings}>
          Extraction Settings
        </button>
        <button type="button" onClick={signOut}>
          Sign out
        </button>
      </div>
    </main>
  );
}
