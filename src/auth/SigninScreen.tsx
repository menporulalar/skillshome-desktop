import { useSignin } from "./useSignin";

export function SigninScreen() {
  const { status, accessToken, signInWithGoogle, signInWithGithub, signOut } = useSignin();

  if (accessToken && status.state === "Success") {
    return (
      <main className="container">
        <h1>Signed in</h1>
        <p>SkillsHome Desktop is connected.</p>
        <button type="button" onClick={signOut}>
          Sign out
        </button>
      </main>
    );
  }

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
