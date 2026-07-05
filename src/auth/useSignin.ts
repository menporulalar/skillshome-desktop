import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// Mirrors src-tauri/src/auth/state.rs's `SigninStatus` (`#[serde(tag = "state")]`).
export type SigninStatus =
  | { state: "Idle" }
  | { state: "AwaitingBrowser" }
  | { state: "AwaitingDeviceConfirmation"; user_code: string; verification_uri: string }
  | { state: "Success" }
  | { state: "Error"; message: string };

export function useSignin() {
  const [status, setStatus] = useState<SigninStatus>({ state: "Idle" });
  const [accessToken, setAccessToken] = useState<string | null>(null);

  useEffect(() => {
    invoke<SigninStatus>("get_signin_status").then(setStatus).catch(() => {});
    invoke<string | null>("get_access_token").then(setAccessToken).catch(() => {});

    const unlisten = listen<SigninStatus>("signin://status", (event) => {
      setStatus(event.payload);
      if (event.payload.state === "Success") {
        invoke<string | null>("get_access_token").then(setAccessToken).catch(() => {});
      }
    });

    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const signInWithGoogle = useCallback(async () => {
    setStatus({ state: "AwaitingBrowser" });
    try {
      await invoke("start_google_signin");
    } catch (err) {
      setStatus({ state: "Error", message: String(err) });
    }
  }, []);

  const signInWithGithub = useCallback(async () => {
    try {
      await invoke("start_github_device_signin");
    } catch (err) {
      setStatus({ state: "Error", message: String(err) });
    }
  }, []);

  const signOut = useCallback(async () => {
    await invoke("sign_out");
    setStatus({ state: "Idle" });
    setAccessToken(null);
  }, []);

  return { status, accessToken, signInWithGoogle, signInWithGithub, signOut };
}
