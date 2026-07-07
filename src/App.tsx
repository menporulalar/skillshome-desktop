import { useState } from "react";
import { SigninScreen } from "./auth/SigninScreen";
import { ExtractionSettingsScreen } from "./extraction/ExtractionSettingsScreen";
import "./App.css";

// No router — this is a 2-3 screen app today. Plain local state is enough; revisit
// if task 4.12's remaining screens make this feel cramped.
type Screen = "signin" | "settings";

function App() {
  const [screen, setScreen] = useState<Screen>("signin");

  if (screen === "settings") {
    return <ExtractionSettingsScreen onBack={() => setScreen("signin")} />;
  }

  return <SigninScreen onOpenSettings={() => setScreen("settings")} />;
}

export default App;
