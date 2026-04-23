/**
 * Per-preset CSV export instructions rendered in CsvImportDrawer's
 * side column. This is the single source of truth for the help copy —
 * bank redesigns are handled by editing this file, not by touching
 * the drawer component.
 *
 * Keys match the preset ids used by CsvImportDrawer's PRESETS array.
 */

export interface PresetHelp {
  /** Heading shown at the top of the side column. */
  title: string;
  /** Numbered steps. Each should be short enough to wrap to 2 lines
   *  maximum at the side column's 240px width for comfortable scanning. */
  steps: string[];
}

export const HELP_BY_PRESET: Record<string, PresetHelp> = {
  monzo: {
    title: "How to export from Monzo",
    steps: [
      "Open the Monzo app on your phone.",
      "Tap Profile (bottom-right).",
      "Tap Statements.",
      "Pick the month you want.",
      "Tap Download and choose CSV.",
    ],
  },
  starling: {
    title: "How to export from Starling",
    steps: [
      "Open the Starling app on your phone.",
      "Tap Spaces or your main account.",
      "Tap the settings icon, then Account.",
      "Choose Statements and set a date range.",
      "Tap Export as CSV.",
    ],
  },
  barclays: {
    title: "How to export from Barclays",
    steps: [
      "Sign in to Barclays online banking.",
      "Open the account you want to export.",
      "Click Statements & documents.",
      "Click Export and pick a date range.",
      "Choose CSV as the format.",
    ],
  },
  hsbc: {
    title: "How to export from HSBC",
    steps: [
      "Sign in to HSBC online banking.",
      "Open the account you want to export.",
      "Click View statements.",
      "Click Download.",
      "Pick CSV and a date range.",
    ],
  },
  natwest: {
    title: "How to export from Natwest",
    steps: [
      "Sign in to Natwest online banking.",
      "Open the account you want to export.",
      "Click Statements.",
      "Pick a date range, click Download.",
      "Choose CSV.",
    ],
  },
  generic: {
    title: "CSV columns Manor expects",
    steps: [
      "Required: Date column (YYYY-MM-DD or DD/MM/YYYY).",
      "Required: Amount column in £ (negative = debit).",
      "Required: Description column.",
      "Optional: Merchant or payee column.",
      "Separate Debit/Credit columns work too — Manor combines them.",
      "Duplicates are detected and skipped, so re-importing is safe.",
    ],
  },
};
