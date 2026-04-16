export type SlashCommand =
  | { type: "task"; title: string }
  | { type: "spent"; amountPence: number; description: string }
  | { type: "unknown"; raw: string };

/**
 * Parse a submitted message for slash-command syntax.
 *
 * /task <title>          — add a task
 * /spent <amount> <desc> — add a manual expense (e.g. /spent £12.50 coffee)
 */
export function parseSlash(input: string): SlashCommand | null {
  if (!input.startsWith("/")) return null;

  // /task <title>
  const taskMatch = input.match(/^\/task\s+(.+?)\s*$/);
  if (taskMatch) {
    const title = taskMatch[1].trim();
    if (!title) return null;
    return { type: "task", title };
  }
  if (input === "/task" || /^\/task\s*$/.test(input)) {
    return null;
  }

  // /spent <amount> <description>
  // Accepts: /spent 12.50 coffee, /spent £12.50 coffee, /spent $8 lunch
  const spentMatch = input.match(/^\/spent\s+[£$€]?(\d+(?:\.\d{1,2})?)\s+(.+?)\s*$/);
  if (spentMatch) {
    const pence = Math.round(parseFloat(spentMatch[1]) * 100);
    const description = spentMatch[2].trim();
    if (pence === 0 || !description) return null;
    return { type: "spent", amountPence: -pence, description }; // negative = expense
  }
  if (/^\/spent\s*$/.test(input)) return null;

  return { type: "unknown", raw: input };
}
