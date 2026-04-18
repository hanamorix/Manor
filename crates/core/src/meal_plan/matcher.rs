//! Staple-matcher — pure function called from L3c's shopping list to decide whether
//! a recipe ingredient should be excluded because it matches a household staple.

use super::StapleItem;

/// Returns true if the ingredient name matches any staple (or staple alias).
/// Matching is: lowercase + trim, strip trailing 's'/'es' for crude singularisation,
/// substring-or-word match against each candidate string.
pub fn staple_matches(ingredient_name: &str, staples: &[StapleItem]) -> bool {
    let ing = normalize(ingredient_name);
    for s in staples {
        if candidate_matches(&ing, &s.name) {
            return true;
        }
        for alias in &s.aliases {
            if candidate_matches(&ing, alias) {
                return true;
            }
        }
    }
    false
}

fn candidate_matches(ing: &str, candidate: &str) -> bool {
    let cand = normalize(candidate);
    if cand.is_empty() {
        return false;
    }
    ing == cand
        || ing.split_whitespace().any(|w| w == cand)
        || ing.contains(&cand)
        || cand.contains(ing)
}

fn normalize(s: &str) -> String {
    let mut out = s.trim().to_lowercase();
    while out.ends_with(|c: char| !c.is_alphanumeric()) {
        out.pop();
    }
    if out.ends_with("es") && out.len() > 3 {
        out.truncate(out.len() - 2);
    } else if out.ends_with('s') && out.len() > 2 {
        out.truncate(out.len() - 1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staple(name: &str, aliases: &[&str]) -> StapleItem {
        StapleItem {
            id: "id".into(),
            name: name.into(),
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
        }
    }

    #[test]
    fn exact_match() {
        let s = vec![staple("olive oil", &[])];
        assert!(staple_matches("olive oil", &s));
    }

    #[test]
    fn alias_match() {
        let s = vec![staple("olive oil", &["EVOO"])];
        assert!(staple_matches("EVOO", &s));
    }

    #[test]
    fn plural_ingredient_vs_singular_staple() {
        let s = vec![staple("garlic clove", &[])];
        assert!(staple_matches("garlic cloves", &s));
    }

    #[test]
    fn substring_in_either_direction() {
        let s = vec![staple("olive oil", &[])];
        assert!(staple_matches("extra virgin olive oil", &s));
    }

    #[test]
    fn no_match_case() {
        let s = vec![staple("salt", &[])];
        assert!(!staple_matches("butter", &s));
    }

    #[test]
    fn empty_aliases_ok() {
        let s = vec![staple("salt", &[])];
        assert!(staple_matches("sea salt", &s));
    }

    #[test]
    fn empty_staples_never_matches() {
        assert!(!staple_matches("anything", &[]));
    }
}
