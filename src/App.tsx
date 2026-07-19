import { useState } from "react";
import { useSignin } from "./auth/useSignin";
import { SigninScreen } from "./auth/SigninScreen";
import { HomeScreen } from "./auth/HomeScreen";
import { ExtractionSettingsScreen } from "./extraction/ExtractionSettingsScreen";
import { useExtractionSettings } from "./extraction/useExtractionSettings";
import { useServerFallbackIngest, type IngestSource } from "./ingest/useServerFallbackIngest";
import { useLocalExtraction } from "./ingest/useLocalExtraction";
import { SourcePickerScreen } from "./ingest/SourcePickerScreen";
import { ExtractionProgressScreen } from "./ingest/ExtractionProgressScreen";
import { ReviewConfirmScreen } from "./ingest/ReviewConfirmScreen";
import { ConnectedProjectsScreen } from "./project-sync/ConnectedProjectsScreen";
import { ProjectSyncScheduler } from "./project-sync/useProjectSync";
import "./App.css";

// "signin" isn't a stored state — it's purely a function of isSignedIn below, so
// it can't go stale (e.g. after sign-out) the way a stored screen value could.
type Screen = "home" | "settings" | "picker" | "progress" | "review" | "projects";

// Carries just enough across the picker → progress → review hand-off; the actual
// polling/async state lives in the two hooks below (lifted here so it survives
// screen transitions — a hook called fresh inside each screen would lose its
// in-flight polling interval the moment that screen unmounts).
interface ExtractionFlow {
  profileId: string;
  source: IngestSource;
  reviewPackage?: unknown;
}

function App() {
  const signin = useSignin();
  const extractionSettings = useExtractionSettings();
  const serverFallback = useServerFallbackIngest();
  const localExtraction = useLocalExtraction();

  const [screen, setScreen] = useState<Screen>("home");
  const [flow, setFlow] = useState<ExtractionFlow | null>(null);
  // Task 4.13, Requirement 3.9: set when the user clicks "Retry via
  // Server_Fallback" after the Local_Model/BYOK_Frontier retry budget is
  // exhausted — overrides the *effective* source for this one flow only,
  // without touching the persisted Extraction_Source setting. Reset whenever a
  // fresh flow starts from the picker so it doesn't leak into an unrelated
  // extraction.
  const [forceServerFallback, setForceServerFallback] = useState(false);

  const isSignedIn = signin.accessToken !== null && signin.status.state === "Success";

  if (!isSignedIn) {
    return (
      <SigninScreen
        status={signin.status}
        signInWithGoogle={signin.signInWithGoogle}
        signInWithGithub={signin.signInWithGithub}
      />
    );
  }

  if (screen === "settings") {
    return <ExtractionSettingsScreen onBack={() => setScreen("home")} />;
  }

  if (screen === "projects") {
    return (
      <>
        <ProjectSyncScheduler />
        <ConnectedProjectsScreen onBack={() => setScreen("home")} />
      </>
    );
  }

  if (screen === "picker") {
    return (
      <SourcePickerScreen
        pickFile={serverFallback.pickFile}
        listProfiles={serverFallback.listProfiles}
        activeSource={extractionSettings.settings?.active_source ?? "server_fallback"}
        onBack={() => setScreen("home")}
        onStart={(profileId, source) => {
          setForceServerFallback(false);
          setFlow({ profileId, source });
          setScreen("progress");
        }}
        onReviewReady={(profileId, reviewPackage) => {
          setForceServerFallback(false);
          setFlow({ profileId, source: { kind: "file", path: "" }, reviewPackage });
          setScreen("review");
        }}
      />
    );
  }

  const effectiveSource = forceServerFallback ? "server_fallback" : (extractionSettings.settings?.active_source ?? "server_fallback");

  if (screen === "progress" && flow) {
    return (
      <ExtractionProgressScreen
        // Forces a genuine remount when the fallback offer is taken — resets the
        // component's internal "started" ref so it actually kicks off a fresh
        // attempt through the new path, rather than being a no-op on an
        // already-mounted instance.
        key={forceServerFallback ? "fallback" : "primary"}
        activeSource={effectiveSource}
        profileId={flow.profileId}
        source={flow.source}
        serverFallback={serverFallback}
        localExtraction={localExtraction}
        onReviewReady={(reviewPackage) => {
          setFlow({ ...flow, reviewPackage });
          setScreen("review");
        }}
        onComplete={() => {
          setFlow(null);
          setScreen("home");
        }}
        onRetry={() => setScreen("picker")}
        onRetryViaServerFallback={() => setForceServerFallback(true)}
      />
    );
  }

  if (screen === "review" && flow?.reviewPackage) {
    return (
      <ReviewConfirmScreen
        activeSource={effectiveSource}
        profileId={flow.profileId}
        reviewPackage={flow.reviewPackage}
        serverFallback={serverFallback}
        localExtraction={localExtraction}
        onConfirmed={() => {
          setFlow(null);
          setScreen("home");
        }}
      />
    );
  }

  return (
    <>
      {/* #25 task 3.8: on-open + weekly local scans run whenever the app is
          open and signed in, independent of which screen is showing. */}
      <ProjectSyncScheduler />
      <HomeScreen
        onStartExtraction={() => setScreen("picker")}
        onOpenSettings={() => setScreen("settings")}
        onOpenProjects={() => setScreen("projects")}
        signOut={signin.signOut}
      />
    </>
  );
}

export default App;
