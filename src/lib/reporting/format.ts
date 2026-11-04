// Translated labels for report ids; the stored ids and the Markdown stay English.
import { t } from "$lib/i18n/index.svelte";
import type { ReportStatus, ReportView } from "$lib/ipc";

export const CATEGORIES = [
  "closed_unexpectedly",
  "feature_not_working",
  "notifications",
  "audio_video",
  "files",
  "visual",
  "other",
] as const;
export const FREQUENCIES = ["always", "sometimes", "once", "unknown"] as const;

export function categoryLabel(id: string): string {
  const labels: Record<string, string> = {
    closed_unexpectedly: t("FastZap closed unexpectedly"),
    feature_not_working: t("Something is not working"),
    notifications: t("Notification problem"),
    audio_video: t("Audio or video problem"),
    files: t("File problem"),
    visual: t("Visual problem"),
    other: t("Other"),
  };
  return labels[id] ?? labels.other;
}

export function frequencyLabel(id: string): string {
  const labels: Record<string, string> = {
    always: t("Always"),
    sometimes: t("Sometimes"),
    once: t("It happened once"),
    unknown: t("I don't know"),
  };
  return labels[id] ?? labels.unknown;
}

export function statusLabel(status: ReportStatus | null): string {
  switch (status) {
    case "pending_review":
      return t("Waiting for review");
    case "copied":
      return t("Copied");
    case "opened_on_github":
      return t("Opened on GitHub");
    default:
      return t("Saved locally");
  }
}

export function isAutomatic(view: ReportView): boolean {
  return view.document.report_type !== "manual_problem";
}

export function kindLabel(view: ReportView): string {
  return isAutomatic(view) ? t("Unexpected closing") : t("Problem report");
}

const text = (v: unknown) => (typeof v === "string" ? v : "");

/** Plain review text, in the interface language. */
export function previewText(view: ReportView): string {
  const doc = view.document;
  const lines = [t("Check all the information below before continuing to GitHub."), "", t("Your report")];
  const category = text(doc.problem_category);
  lines.push(`${t("Problem type")}: ${category ? categoryLabel(category) : t("Unexpected closing")}`);
  if (text(doc.user_description)) lines.push("", `${t("Description")}:`, text(doc.user_description));
  if (text(doc.expected_behavior)) lines.push("", `${t("Expected result")}:`, text(doc.expected_behavior));
  if (text(doc.frequency)) lines.push(`${t("Frequency")}: ${frequencyLabel(text(doc.frequency))}`);
  const info = doc.system_information as Record<string, unknown> | undefined;
  if (info) {
    lines.push("", t("Application information"));
    for (const key of ["fastzap_version", "package_type", "operating_system", "desktop_environment", "session_type", "architecture", "webview_version"]) {
      if (text(info[key])) lines.push(`${key.replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase())}: ${text(info[key])}`);
    }
  }
  const error = doc.error_information as Record<string, unknown> | undefined;
  if (error) {
    lines.push("", t("Error information"), `${t("Error type")}: ${text(error.type)}`);
    if (text(error.message)) lines.push(`${t("Error details")}: ${text(error.message)}`);
  }
  if (text(doc.sanitized_logs)) lines.push("", t("Sanitized logs are included in the technical information."));
  lines.push(
    "",
    t("Report destination"),
    t("Official FastZap repository on GitHub: quantforge-br/fastzap"),
    t("FastZap will copy the report to the clipboard and open GitHub. Paste it into the issue description before publishing. A GitHub account is required."),
    "",
    t("Privacy"),
    t("Never sent: messages, contacts, phone numbers, cookies, WhatsApp session data, conversation content, passwords, or authentication tokens."),
  );
  return lines.join("\n");
}
