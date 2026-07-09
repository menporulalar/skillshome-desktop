import { useState } from "react";
import type { ExtractionSource } from "../extraction/useExtractionSettings";
import type { useServerFallbackIngest } from "./useServerFallbackIngest";
import type { useLocalExtraction } from "./useLocalExtraction";

interface ReviewItem {
  id: string;
  type: string;
  status: "pending" | "accepted" | "rejected" | "edited";
  data: Record<string, unknown>;
}

interface ReviewPackageShape {
  skills: ReviewItem[];
  projects: ReviewItem[];
  experience: ReviewItem[];
  education?: ReviewItem[];
  certificates?: ReviewItem[];
  accolades?: ReviewItem[];
  [key: string]: unknown;
}

interface Props {
  activeSource: ExtractionSource;
  profileId: string;
  reviewPackage: unknown;
  serverFallback: ReturnType<typeof useServerFallbackIngest>;
  localExtraction: ReturnType<typeof useLocalExtraction>;
  onConfirmed: () => void;
}

const SECTIONS: Array<keyof ReviewPackageShape> = ["skills", "experience", "projects", "education", "certificates", "accolades"];

function itemLabel(item: ReviewItem): string {
  const d = item.data;
  return (d.name as string) ?? (d.title as string) ?? (d.role as string) ?? item.id;
}

export function ReviewConfirmScreen({ activeSource, profileId, reviewPackage, serverFallback, localExtraction, onConfirmed }: Props) {
  const [pkg, setPkg] = useState<ReviewPackageShape>(reviewPackage as ReviewPackageShape);

  const toggle = (section: keyof ReviewPackageShape, id: string) => {
    setPkg((prev) => {
      const items = (prev[section] as ReviewItem[] | undefined) ?? [];
      return {
        ...prev,
        [section]: items.map((item) =>
          item.id === id ? { ...item, status: item.status === "accepted" ? "rejected" : "accepted" } : item,
        ),
      };
    });
  };

  const allItems = SECTIONS.flatMap((section) => (pkg[section] as ReviewItem[] | undefined) ?? []);
  const acceptedCount = allItems.filter((item) => item.status === "accepted").length;
  const busy = activeSource === "server_fallback" ? serverFallback.busy : localExtraction.busy;
  const errorMessage = activeSource === "server_fallback" ? serverFallback.errorMessage : localExtraction.errorMessage;

  const handleConfirm = async () => {
    try {
      if (activeSource === "server_fallback") {
        await serverFallback.confirm(profileId, pkg);
      } else {
        await localExtraction.confirm(profileId, pkg);
      }
      onConfirmed();
    } catch {
      // errorMessage state on the relevant hook already reflects this.
    }
  };

  return (
    <main className="container">
      <h1>Review Extraction</h1>
      <p>Accept the items you want to keep, then confirm.</p>

      <div style={{ textAlign: "left", maxWidth: 480, margin: "0 auto" }}>
        {SECTIONS.map((section) => {
          const items = (pkg[section] as ReviewItem[] | undefined) ?? [];
          if (items.length === 0) return null;
          return (
            <div key={section as string} style={{ marginBottom: "1em" }}>
              <h3 style={{ textTransform: "capitalize" }}>{section as string}</h3>
              {items.map((item) => (
                <label key={item.id} style={{ display: "block", marginBottom: "0.25em" }}>
                  <input
                    type="checkbox"
                    checked={item.status === "accepted"}
                    onChange={() => toggle(section, item.id)}
                  />
                  {" "}{itemLabel(item)}
                  {item.status === "rejected" && <span style={{ opacity: 0.6 }}> (rejected)</span>}
                </label>
              ))}
            </div>
          );
        })}
      </div>

      {errorMessage && <p style={{ color: "red" }}>{errorMessage}</p>}

      <button type="button" disabled={busy || acceptedCount === 0} onClick={handleConfirm}>
        {busy ? "Confirming…" : `Confirm (${acceptedCount} accepted)`}
      </button>
    </main>
  );
}
