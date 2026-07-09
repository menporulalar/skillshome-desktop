import type { SigninStatus } from "./useSignin";

// Signed-out view only (task 4.12) — App.tsx owns useSignin() as a single source
// of truth shared across screens and renders HomeScreen.tsx once signed in,
// instead of this component branching on signed-in/signed-out itself.
interface Props {
  status: SigninStatus;
  signInWithGoogle: () => void;
  signInWithGithub: () => void;
}

export function SigninScreen({ status, signInWithGoogle, signInWithGithub }: Props) {
  return (
    <main className="container">
      <h1>Sign in to SkillsHome</h1>

      {status.state === "AwaitingDeviceConfirmation" ? (
        <div className="row">
          <p>
            Enter this code at <strong>{status.verification_uri}</strong>:
          </p>
          <p style={{ fontSize: "1.5rem", letterSpacing: "0.2rem" }}>{status.user_code}</p>
        </div>
      ) : (
        <div className="row">
          <button type="button" onClick={signInWithGoogle} disabled={status.state === "AwaitingBrowser"}>
            {status.state === "AwaitingBrowser" ? "Waiting for browser…" : "Sign in with Google"}
          </button>
          <button type="button" onClick={signInWithGithub}>
            Sign in with GitHub
          </button>
        </div>
      )}

      {status.state === "Error" && <p style={{ color: "red" }}>{status.message}</p>}
    </main>
  );
}
