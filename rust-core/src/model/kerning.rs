// Ported from runebender-xilem/src/model/kerning.rs (Apache-2.0).

//! Kerning lookup algorithm.
//!
//! Implements the UFO spec kerning lookup precedence:
//! 1. Glyph + glyph (highest priority)
//! 2. Glyph + group
//! 3. Group + glyph
//! 4. Group + group (lowest priority)
//! 5. Return 0.0 if no match

use std::collections::HashMap;

/// Look up the kerning value between two glyphs, following UFO-spec
/// precedence (glyph→glyph beats group→glyph beats glyph→group beats
/// group→group). Returns 0.0 if no match.
pub fn lookup_kerning(
    kerning_pairs: &HashMap<String, HashMap<String, f64>>,
    groups: &HashMap<String, Vec<String>>,
    left_glyph: &str,
    left_group: Option<&str>,
    right_glyph: &str,
    right_group: Option<&str>,
) -> f64 {
    // 1. Glyph + glyph.
    if let Some(value) = lookup_pair(kerning_pairs, left_glyph, right_glyph) {
        return value;
    }

    // 2. Glyph + right_group.
    if let Some(value) = lookup_glyph_to_group(
        kerning_pairs,
        groups,
        left_glyph,
        right_glyph,
        right_group,
        false,
    ) {
        return value;
    }

    // 3. left_group + Glyph.
    if let Some(value) = lookup_glyph_to_group(
        kerning_pairs,
        groups,
        right_glyph,
        left_glyph,
        left_group,
        true,
    ) {
        return value;
    }

    // 4. left_group + right_group.
    if let Some(value) = lookup_group_to_group(
        kerning_pairs,
        groups,
        left_glyph,
        left_group,
        right_glyph,
        right_group,
    ) {
        return value;
    }

    // 5. No kerning found.
    0.0
}

fn lookup_pair(
    kerning_pairs: &HashMap<String, HashMap<String, f64>>,
    first: &str,
    second: &str,
) -> Option<f64> {
    kerning_pairs.get(first)?.get(second).copied()
}

/// `reverse=false` looks up `first_glyph + group_containing_second`.
/// `reverse=true` looks up `group_containing_first + second_glyph`.
fn lookup_glyph_to_group(
    kerning_pairs: &HashMap<String, HashMap<String, f64>>,
    groups: &HashMap<String, Vec<String>>,
    first_glyph: &str,
    second_glyph: &str,
    second_group_hint: Option<&str>,
    reverse: bool,
) -> Option<f64> {
    if let Some(group_name) = second_group_hint {
        if let Some(group_members) = groups.get(group_name)
            && group_members.contains(&second_glyph.to_string())
        {
            let value = if reverse {
                lookup_pair(kerning_pairs, group_name, first_glyph)
            } else {
                lookup_pair(kerning_pairs, first_glyph, group_name)
            };
            if value.is_some() {
                return value;
            }
        }
    }

    for (group_name, members) in groups {
        if members.contains(&second_glyph.to_string()) {
            let value = if reverse {
                lookup_pair(kerning_pairs, group_name, first_glyph)
            } else {
                lookup_pair(kerning_pairs, first_glyph, group_name)
            };
            if value.is_some() {
                return value;
            }
        }
    }

    None
}

fn lookup_group_to_group(
    kerning_pairs: &HashMap<String, HashMap<String, f64>>,
    groups: &HashMap<String, Vec<String>>,
    left_glyph: &str,
    left_group_hint: Option<&str>,
    right_glyph: &str,
    right_group_hint: Option<&str>,
) -> Option<f64> {
    let mut left_groups = Vec::new();
    if let Some(hint) = left_group_hint
        && let Some(members) = groups.get(hint)
        && members.contains(&left_glyph.to_string())
    {
        left_groups.push(hint);
    }
    for (group_name, members) in groups {
        if members.contains(&left_glyph.to_string())
            && !left_groups.contains(&group_name.as_str())
        {
            left_groups.push(group_name.as_str());
        }
    }

    let mut right_groups = Vec::new();
    if let Some(hint) = right_group_hint
        && let Some(members) = groups.get(hint)
        && members.contains(&right_glyph.to_string())
    {
        right_groups.push(hint);
    }
    for (group_name, members) in groups {
        if members.contains(&right_glyph.to_string())
            && !right_groups.contains(&group_name.as_str())
        {
            right_groups.push(group_name.as_str());
        }
    }

    for left_group in &left_groups {
        for right_group in &right_groups {
            if let Some(value) = lookup_pair(kerning_pairs, left_group, right_group) {
                return Some(value);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_kerning() -> HashMap<String, HashMap<String, f64>> {
        let mut kerning = HashMap::new();

        let mut a_pairs = HashMap::new();
        a_pairs.insert("V".to_string(), -50.0);
        kerning.insert("A".to_string(), a_pairs);

        let mut round_left_pairs = HashMap::new();
        round_left_pairs.insert("A".to_string(), -40.0);
        round_left_pairs.insert("public.kern2.round".to_string(), -20.0);
        kerning.insert("public.kern1.round".to_string(), round_left_pairs);

        let mut t_pairs = HashMap::new();
        t_pairs.insert("public.kern2.round".to_string(), -30.0);
        kerning.insert("T".to_string(), t_pairs);

        kerning
    }

    fn make_groups() -> HashMap<String, Vec<String>> {
        let mut groups = HashMap::new();
        groups.insert(
            "public.kern1.round".to_string(),
            vec!["O".to_string(), "D".to_string(), "Q".to_string()],
        );
        groups.insert(
            "public.kern2.round".to_string(),
            vec!["o".to_string(), "d".to_string(), "q".to_string()],
        );
        groups
    }

    #[test]
    fn test_glyph_to_glyph() {
        let kerning = make_kerning();
        let groups = make_groups();

        let result = lookup_kerning(&kerning, &groups, "A", None, "V", None);
        assert_eq!(result, -50.0);
    }

    #[test]
    fn test_glyph_to_group() {
        let kerning = make_kerning();
        let groups = make_groups();

        let result = lookup_kerning(
            &kerning,
            &groups,
            "T",
            None,
            "o",
            Some("public.kern2.round"),
        );
        assert_eq!(result, -30.0);
    }

    #[test]
    fn test_group_to_glyph() {
        let kerning = make_kerning();
        let groups = make_groups();

        let result = lookup_kerning(
            &kerning,
            &groups,
            "O",
            Some("public.kern1.round"),
            "A",
            None,
        );
        assert_eq!(result, -40.0);
    }

    #[test]
    fn test_group_to_group() {
        let kerning = make_kerning();
        let groups = make_groups();

        let result = lookup_kerning(
            &kerning,
            &groups,
            "O",
            Some("public.kern1.round"),
            "o",
            Some("public.kern2.round"),
        );
        assert_eq!(result, -20.0);
    }

    #[test]
    fn test_no_kerning() {
        let kerning = make_kerning();
        let groups = make_groups();

        let result = lookup_kerning(&kerning, &groups, "X", None, "Y", None);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_precedence() {
        let mut kerning = make_kerning();
        let groups = make_groups();

        kerning.insert("O".to_string(), HashMap::new());
        kerning
            .get_mut("O")
            .unwrap()
            .insert("o".to_string(), -100.0);

        // glyph→glyph (-100) should beat group→group (-20).
        let result = lookup_kerning(
            &kerning,
            &groups,
            "O",
            Some("public.kern1.round"),
            "o",
            Some("public.kern2.round"),
        );
        assert_eq!(result, -100.0);
    }
}
