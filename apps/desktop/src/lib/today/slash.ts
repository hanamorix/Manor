export type SlashCommand =
  | { type: "task"; title: string }
  | { type: "unknown"; raw: string };

/**
 * Parse a submitted message for slash-command syntax.
 *
 * Returns:
 *  - null if not a slash command (no leading slash, or `/task` with empty title)
 *  - a typed SlashCommand otherwise
 */
export function parseSlash(input: string): SlashCommand | null {
  if (!input.startsWith("/")) return null;

  const taskMatch = input.match(/^\/task\s+(.+?)\s*$/);
  if (taskMatch) {
    const title = taskMatch[1].trim();
    if (!title) return null;
    return { type: "task", title };
  }
  if (input === "/task" || /^\/task\s*$/.test(input)) {
    return null;
  }

  return { type: "unknown", raw: input };
}
