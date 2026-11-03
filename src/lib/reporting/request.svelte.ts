// The report dialog is shown by the main page over every panel; settings
// pages and the startup prompt open it through here.
import type { ReportView } from "$lib/ipc";

export const reportDialog = $state<{ open: boolean; review: ReportView | null }>({
  open: false,
  review: null,
});

export function openReport(review: ReportView | null = null) {
  reportDialog.review = review;
  reportDialog.open = true;
}

export function closeReport() {
  reportDialog.open = false;
  reportDialog.review = null;
}
