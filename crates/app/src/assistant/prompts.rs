//! System prompt for Manor's local LLM.
//!
//! Phase 1.L of v0.2 Hands. The prompt is now built per-turn so the
//! `context::render(...)` block can be spliced in at the right place. The
//! shape lives in [`build_system_prompt`]; the const skeleton above the
//! function exists so a smoke-test or REPL probe can see the structure
//! without invoking the builder.
//!
//! ## Why a builder, not a const
//!
//! Pre-1.L, [`SYSTEM_PROMPT`] was `&'static str` and the today block was
//! `format!`-spliced into the chat-message content at the call site. With
//! per-room context (1.K) each turn now produces a different markdown
//! block, and the rules block needs to sit between the tool list and the
//! context — not appended after the whole prompt. The cleanest fix is a
//! builder that owns the placeholder discipline.
//!
//! ## What's locked in for v0.2
//!
//! - The clarify rule is embedded in Phase 1 even though the `clarify`
//!   tool itself doesn't land until Phase 7. Spec §B: "the prompt is
//!   already trained-shaped" — by Phase 7 the model has been seeing the
//!   rule for the whole v0.2 line.
//! - Currency-as-pence and list-as-array rules: same logic. Tools that
//!   *use* arrays land in Phase 2; tools that *need* pence (ledger) land
//!   in Phase 4. Both rules sit in the prompt now so model behaviour is
//!   stable across the rollout.
//! - The tool list enumerates only what `tools::all_tools()` currently
//!   exposes (just `add_task` in Phase 1). Each Phase 2+ tool flips its
//!   `// TODO` marker into a one-line description as it lands.

/// Stable identity preamble — the part of the prompt that never changes
/// turn-to-turn. Exposed as `pub` for tests and tooling that want to
/// snapshot the static skeleton without driving the builder.
pub const PROMPT_PREAMBLE: &str = concat!(
    "You are Manor, a calm desktop assistant for one person's household. ",
    "You can propose changes — adding tasks, chores, events, transactions, ",
    "budgets, recipes, meal plans, assets, maintenance — but you do not ",
    "apply them. The user reviews and approves each proposal.\n",
    "\n",
    "Be warm, concise, and practical. Never speak as Nell or any other ",
    "persona. Never claim you have done something — say \"I'd like to ",
    "add...\" or \"shall I propose...?\" instead. The proposal banner ",
    "shows the user what you suggested.",
);

/// One-line description per *currently-wired* tool. Phase 2+ flips each
/// commented-out line below into a real entry as the corresponding tool
/// arm lands in `tools::all_tools()` and `commands::send_message`.
pub const PROMPT_TOOLS: &str = concat!(
    "Tools:\n",
    "- add_task — add a single one-off to-do for today or a chosen date.\n",
    // TODO Phase 2: add_chore — recurring household chore with rotation.
    // TODO Phase 2: add_time_block — focus / time block on a calendar day.
    // TODO Phase 2: add_recurring_block — repeating time block.
    // TODO Phase 3: complete_task — mark a known task done.
    // TODO Phase 3: add_event — create a CalDAV event (batched if many).
    // TODO Phase 4: add_transaction — record a money movement (in pence).
    // TODO Phase 4: set_budget — set a monthly budget for a category.
    // TODO Phase 4: add_recurring_payment — bills / subscriptions.
    // TODO Phase 4: add_contract — long-form supplier contract.
    // TODO Phase 5: add_recipe_quick — capture a recipe by name + steps.
    // TODO Phase 5: plan_meal — slot a recipe into a date.
    // TODO Phase 5: add_to_shopping_list — add ingredients to the list.
    // TODO Phase 6: add_asset — register a household asset.
    // TODO Phase 6: log_maintenance — record completed maintenance.
    // TODO Phase 6: add_maintenance_schedule — recurring service interval.
    // TODO Phase 7: clarify — ask the user a yes/no or one-of-N question.
);

/// Behavioural rules. These survive every Phase. The clarify rule
/// references a tool that doesn't exist yet — that's intentional (see
/// module docs).
pub const PROMPT_RULES: &str = concat!(
    "Rules:\n",
    "1. When the user's intent is ambiguous, ask the user to clarify ",
    "instead of guessing. Examples of ambiguity: task vs chore, which ",
    "category, which asset, which calendar account. (In a future ",
    "version this will be a `clarify` tool; for now, ask in plain text.)\n",
    "2. Never claim you have done something. Use \"I'd like to add...\" ",
    "or \"shall I propose...?\" — the user must approve.\n",
    "3. For lists of similar actions (e.g. multiple chores, multiple ",
    "transactions), call the tool once with an array of items.\n",
    "4. Currency amounts are integer pence. £40.00 → 4000. Never emit ",
    "decimals or currency symbols inside tool args.\n",
    "5. If a referenced category, asset, or recipe does not exist in ",
    "the context provided to you, ask the user about it before ",
    "proposing — do not invent ids.",
);

/// Marker the builder splices the dynamic context block in at. Exposed
/// for tests so the contract is greppable.
pub const CONTEXT_PLACEHOLDER: &str = "Context for this turn:";

/// Build the full system prompt for one turn.
///
/// Layout:
/// ```text
/// {PROMPT_PREAMBLE}
///
/// {PROMPT_TOOLS}
///
/// {PROMPT_RULES}
///
/// Context for this turn:
/// {context_block}
/// ```
///
/// `context_block` comes from [`super::context::render`]; pass the empty
/// string to omit the section entirely (the placeholder line still
/// appears so the model's expectation is stable).
pub fn build_system_prompt(context_block: &str) -> String {
    let body = if context_block.is_empty() {
        String::new()
    } else if context_block.ends_with('\n') {
        context_block.to_string()
    } else {
        format!("{context_block}\n")
    };
    format!("{PROMPT_PREAMBLE}\n\n{PROMPT_TOOLS}\n{PROMPT_RULES}\n\n{CONTEXT_PLACEHOLDER}\n{body}")
}

/// Backwards-compat alias — the bare static body without any context.
/// Retained for the few call sites (and tests) that want to snapshot the
/// prompt skeleton without driving the builder.
pub fn system_prompt_skeleton() -> String {
    build_system_prompt("")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_system_prompt_is_non_empty() {
        let p = build_system_prompt("");
        assert!(!p.is_empty());
    }

    #[test]
    fn build_system_prompt_contains_preamble() {
        let p = build_system_prompt("");
        assert!(p.contains("You are Manor"));
        assert!(p.contains("propose changes"));
    }

    #[test]
    fn build_system_prompt_lists_currently_wired_tools() {
        let p = build_system_prompt("");
        assert!(p.contains("add_task"));
        // Phase 1 has not yet shipped these — they must NOT appear as
        // active entries (only as TODO comments inside the const, which
        // do not survive into the runtime string).
        assert!(!p.contains("\n- add_chore"), "got: {p}");
        assert!(!p.contains("\n- clarify"), "got: {p}");
    }

    #[test]
    fn build_system_prompt_includes_all_five_rules() {
        let p = build_system_prompt("");
        assert!(p.contains("ask the user to clarify"), "rule 1: {p}");
        assert!(p.contains("Never claim"), "rule 2: {p}");
        assert!(p.contains("array of items"), "rule 3: {p}");
        assert!(p.contains("integer pence"), "rule 4: {p}");
        assert!(p.contains("ask the user about it"), "rule 5: {p}");
    }

    #[test]
    fn build_system_prompt_interpolates_context_at_placeholder() {
        let ctx = "## Today — Saturday, 10 May\nNothing scheduled.";
        let p = build_system_prompt(ctx);
        // The placeholder line and the context block are adjacent.
        let needle = format!("{CONTEXT_PLACEHOLDER}\n{ctx}");
        assert!(p.contains(&needle), "got: {p}");
    }

    #[test]
    fn build_system_prompt_empty_context_keeps_placeholder_line() {
        let p = build_system_prompt("");
        assert!(p.contains(CONTEXT_PLACEHOLDER));
    }

    #[test]
    fn build_system_prompt_normalises_trailing_newline() {
        // With or without a trailing newline on the context, the prompt
        // should not introduce a doubled blank line.
        let with = build_system_prompt("BLOCK\n");
        let without = build_system_prompt("BLOCK");
        assert_eq!(with, without);
    }

    #[test]
    fn skeleton_alias_matches_empty_context_build() {
        assert_eq!(system_prompt_skeleton(), build_system_prompt(""));
    }
}
