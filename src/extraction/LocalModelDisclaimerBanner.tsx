// Requirement 10.4 — a persistent quality disclaimer while Local_Model is the active
// Extraction_Source. Its own component (not inlined into ExtractionSettingsScreen)
// since task 4.12's future extraction-progress screen will very likely need the same
// banner wherever Local_Model is active, not just here.
import { LIGHT_THEME } from "../../brand/theme/tokens";
import { withAlpha } from "../../brand/theme/color";

export function LocalModelDisclaimerBanner() {
  return (
    <div
      role="status"
      style={{
        border: `1px solid ${withAlpha(LIGHT_THEME.warning, 0.5)}`,
        background: withAlpha(LIGHT_THEME.warning, 0.1),
        color: LIGHT_THEME.warning,
        borderRadius: 8,
        padding: "0.6em 1em",
        margin: "1em 0",
        textAlign: "left",
      }}
    >
      <strong>Local model active:</strong> small/local models may produce lower-quality
      skill and experience extraction than SkillsHome's managed pipeline.
    </div>
  );
}
