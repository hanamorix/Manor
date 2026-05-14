//! Per-room dynamic context composition for the assistant prompt.
//!
//! Phase 1.K of v0.2 Hands. Each user turn calls [`classify`] on the message
//! to pick which room slices to include, then [`render`] composes the
//! markdown blocks. This replaces the old "always emit today" path so
//! ledger/rhythm/hearth/bones questions get the right state instead of a
//! today block plus hallucinated category/asset ids.
//!
//! The slice flags are intentionally additive — multi-room messages produce
//! multiple flags. The `today` slice defaults on when *no* other room
//! matched, so the assistant sees today's state in the absence of a clearer
//! signal (matches v0.1.5 behaviour for unrelated chitchat).
//!
//! ## Phase 1 vs Phase 2+
//!
//! Today's slice is the full body from [`super::today::compose_today_block`]
//! and is the regression-zero contract for v0.1.5 → v0.2 Phase 1 (any test
//! fixture that called `compose_today_context` returns identical output).
//!
//! Other slices are minimal stubs — counts and id-name pairs — by design.
//! Phase 2+ enrich them as their tools land (e.g. `add_chore` arrives with
//! a fuller rhythm slice that lists active chore titles + rotation members).
//!
//! ## Tests
//!
//! Tests live at the bottom of this file. The today regression is
//! delegated to `today.rs` tests, which still call `compose_today_context`
//! and so exercise the round-trip through `render` automatically.

use anyhow::Result;
use chrono::{DateTime, Local};
use rusqlite::Connection;

/// Which per-room slices to include in the rendered context.
///
/// `Copy` so it threads through the call chain without explicit clones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextSlices {
    pub today: bool,
    pub rhythm: bool,
    pub ledger: bool,
    pub hearth: bool,
    pub bones: bool,
}

impl ContextSlices {
    pub const NONE: Self = Self {
        today: false,
        rhythm: false,
        ledger: false,
        hearth: false,
        bones: false,
    };

    pub const ALL: Self = Self {
        today: true,
        rhythm: true,
        ledger: true,
        hearth: true,
        bones: true,
    };

    /// `today: true` and everything else false. The default for short
    /// messages and the v0.1.5 compatibility wrapper.
    pub const fn today_only() -> Self {
        Self {
            today: true,
            ..Self::NONE
        }
    }
}

/// Lowercase keyword classifier — returns slice flags from the user's
/// message. Multi-room messages produce multiple flags.
///
/// `today` defaults on when nothing else matched, so a question like
/// "what's the weather" still gets today context. When a different room
/// matches, the today slice fires only if a today-shaped keyword (today,
/// now, tomorrow, schedule, calendar, event, meeting, task, todo, remind)
/// also appears in the message.
pub fn classify(user_message: &str) -> ContextSlices {
    let s = user_message.to_lowercase();

    let rhythm = matches_any(
        &s,
        &[
            "chore",
            "chores",
            "dishes",
            "laundry",
            "rotation",
            "weekly",
            "alternating",
            "time block",
            "time-block",
            "focus",
        ],
    );
    let ledger = matches_any(
        &s,
        &[
            "spend",
            "spent",
            "paid",
            "payment",
            "transaction",
            "budget",
            "£",
            "$",
            "€",
            "recurring",
            "contract",
            "subscription",
            "bill",
            "gardener",
        ],
    );
    let hearth = matches_any(
        &s,
        &[
            "recipe",
            "meal",
            "dinner",
            "lunch",
            "breakfast",
            "cook",
            "ingredient",
            "shopping list",
            "groceries",
            "plan",
        ],
    );
    let bones = matches_any(
        &s,
        &[
            "asset",
            "boiler",
            "fridge",
            "washing machine",
            "maintain",
            "maintenance",
            "repair",
            "service",
        ],
    );
    let today_explicit = matches_any(
        &s,
        &[
            "today",
            "now",
            "this morning",
            "tonight",
            "tomorrow",
            "schedule",
            "calendar",
            "event",
            "meeting",
            "task",
            "todo",
            "to do",
            "remind",
        ],
    );

    let any_other = rhythm || ledger || hearth || bones;
    let today = today_explicit || !any_other;

    ContextSlices {
        today,
        rhythm,
        ledger,
        hearth,
        bones,
    }
}

fn matches_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

/// Render the slices into a single markdown string. Empty `slices` yields
/// the empty string (caller decides whether to splice it in).
///
/// Slice order is fixed: today → rhythm → ledger → hearth → bones. Each
/// slice is separated from the previous by a single blank line so the
/// result reads as discrete H2 sections.
pub fn render(now: DateTime<Local>, conn: &Connection, slices: ContextSlices) -> Result<String> {
    let mut out = String::new();

    if slices.today {
        out.push_str(&super::today::compose_today_block(now, conn)?);
    }

    if slices.rhythm {
        push_section_separator(&mut out);
        let n = manor_core::assistant::chore::list_all(conn)?.len();
        out.push_str(&format!("## Rhythm\n(active chores: {n})\n"));
    }

    if slices.ledger {
        push_section_separator(&mut out);
        let cats = manor_core::ledger::category::list(conn)?;
        let names: Vec<String> = cats.into_iter().map(|c| c.name).collect();
        out.push_str(&format!("## Ledger\n(categories: {})\n", names.join(", ")));
    }

    if slices.hearth {
        push_section_separator(&mut out);
        let n = manor_core::recipe::dal::list_recipes(
            conn,
            &manor_core::recipe::dal::ListFilter::default(),
        )?
        .len();
        out.push_str(&format!("## Hearth\n(recipes: {n})\n"));
    }

    if slices.bones {
        push_section_separator(&mut out);
        let assets = manor_core::asset::dal::list_assets(
            conn,
            &manor_core::asset::dal::AssetListFilter::default(),
        )?;
        out.push_str("## Bones\n");
        for a in &assets {
            out.push_str(&format!(
                "- {} — {} — {}\n",
                a.id,
                a.name,
                a.category.as_str()
            ));
        }
    }

    Ok(out)
}

fn push_section_separator(out: &mut String) {
    if !out.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone};
    use manor_core::assistant::db;
    use tempfile::tempdir;

    fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = db::init(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn local_dt(date: &str, h: u32, m: u32) -> DateTime<Local> {
        let naive = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap();
        Local.from_local_datetime(&naive).unwrap()
    }

    // ── classify ──────────────────────────────────────────────────────────

    #[test]
    fn classify_empty_defaults_to_today() {
        let s = classify("");
        assert!(s.today);
        assert!(!s.rhythm);
        assert!(!s.ledger);
        assert!(!s.hearth);
        assert!(!s.bones);
    }

    #[test]
    fn classify_what_is_on_today_picks_today() {
        let s = classify("what's on today");
        assert!(s.today);
        assert!(!s.rhythm);
    }

    #[test]
    fn classify_dishes_picks_rhythm_only() {
        let s = classify("add 'Lewis does dishes' alternating with Scarlett");
        assert!(s.rhythm);
        assert!(!s.today, "today should be off when only rhythm matches");
    }

    #[test]
    fn classify_pound_amount_picks_ledger() {
        let s = classify("add £40 to gardener");
        assert!(s.ledger);
    }

    #[test]
    fn classify_dollar_or_euro_also_picks_ledger() {
        assert!(classify("paid $20 for lunch").ledger);
        assert!(classify("€15 spent").ledger);
    }

    #[test]
    fn classify_recipe_picks_hearth() {
        let s = classify("add a new recipe for carbonara");
        assert!(s.hearth);
    }

    #[test]
    fn classify_boiler_picks_bones() {
        let s = classify("when did I service the boiler");
        assert!(s.bones);
    }

    #[test]
    fn classify_multi_room_message() {
        // "remind" → today; "boiler" + "service" → bones.
        let s = classify("remind me to service the boiler tomorrow");
        assert!(s.today);
        assert!(s.bones);
        assert!(!s.ledger);
    }

    #[test]
    fn classify_is_case_insensitive() {
        assert!(classify("BUDGET").ledger);
        assert!(classify("Recipe").hearth);
    }

    // ── render ────────────────────────────────────────────────────────────

    #[test]
    fn render_none_yields_empty_string() {
        let (_d, conn) = fresh_conn();
        let now = local_dt("2026-05-10", 12, 0);
        let out = render(now, &conn, ContextSlices::NONE).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn render_today_only_matches_compose_today_context() {
        // Regression guard: render(today_only) must equal the legacy entry
        // point byte-for-byte. Both routes must converge on the same body.
        let (_d, conn) = fresh_conn();
        let now = local_dt("2026-05-10", 9, 30);
        let via_render = render(now, &conn, ContextSlices::today_only()).unwrap();
        let legacy = super::super::today::compose_today_context(now, &conn).unwrap();
        assert_eq!(via_render, legacy);
    }

    #[test]
    fn render_rhythm_stub_on_empty_db_reports_zero() {
        let (_d, conn) = fresh_conn();
        let now = local_dt("2026-05-10", 12, 0);
        let out = render(
            now,
            &conn,
            ContextSlices {
                rhythm: true,
                ..ContextSlices::NONE
            },
        )
        .unwrap();
        assert_eq!(out, "## Rhythm\n(active chores: 0)\n");
    }

    #[test]
    fn render_hearth_stub_on_empty_db_reports_zero() {
        let (_d, conn) = fresh_conn();
        let now = local_dt("2026-05-10", 12, 0);
        let out = render(
            now,
            &conn,
            ContextSlices {
                hearth: true,
                ..ContextSlices::NONE
            },
        )
        .unwrap();
        assert_eq!(out, "## Hearth\n(recipes: 0)\n");
    }

    #[test]
    fn render_bones_stub_on_empty_db_lists_no_assets() {
        let (_d, conn) = fresh_conn();
        let now = local_dt("2026-05-10", 12, 0);
        let out = render(
            now,
            &conn,
            ContextSlices {
                bones: true,
                ..ContextSlices::NONE
            },
        )
        .unwrap();
        assert_eq!(out, "## Bones\n");
    }

    #[test]
    fn render_bones_stub_lists_inserted_asset() {
        use manor_core::asset::{dal as asset_dal, AssetCategory, AssetDraft};
        let (_d, conn) = fresh_conn();
        let id = asset_dal::insert_asset(
            &conn,
            &AssetDraft {
                name: "Boiler".into(),
                category: AssetCategory::Appliance,
                make: None,
                model: None,
                serial_number: None,
                purchase_date: None,
                notes: String::new(),
                hero_attachment_uuid: None,
            },
        )
        .unwrap();
        let now = local_dt("2026-05-10", 12, 0);
        let out = render(
            now,
            &conn,
            ContextSlices {
                bones: true,
                ..ContextSlices::NONE
            },
        )
        .unwrap();
        assert_eq!(out, format!("## Bones\n- {id} — Boiler — appliance\n"));
    }

    #[test]
    fn render_multi_slice_separates_with_blank_line() {
        let (_d, conn) = fresh_conn();
        let now = local_dt("2026-05-10", 12, 0);
        let out = render(
            now,
            &conn,
            ContextSlices {
                rhythm: true,
                hearth: true,
                ..ContextSlices::NONE
            },
        )
        .unwrap();
        // Sections separated by exactly one blank line ("\n\n").
        assert!(
            out.contains("## Rhythm\n(active chores: 0)\n\n## Hearth\n(recipes: 0)\n"),
            "got: {out:?}",
        );
    }
}
