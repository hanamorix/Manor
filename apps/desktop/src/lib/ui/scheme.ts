/**
 * Return true only for absolute URLs with http(s) scheme.
 * Relative URLs and anchors are rejected because RepairMarkdown passes hrefs
 * straight to the Tauri shell plugin — anchors don't make sense there.
 */
export function isSafeExternalScheme(href: string | undefined): boolean {
  if (!href) return false;
  try {
    const u = new URL(href);
    return u.protocol === "http:" || u.protocol === "https:";
  } catch {
    return false;
  }
}
