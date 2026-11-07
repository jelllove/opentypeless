//! Licensed starter spellings; no runtime downloads and no definitions.
pub const SEED_ID: &str = "starter-v1";
pub const TERMS: &str = include_str!("../resources/dictionary/starter-terms.txt");
pub const MAX_PROMPT_TERMS: usize = 64;

pub fn terms() -> Vec<&'static str> {
    TERMS
        .lines()
        .filter(|word| !word.trim().is_empty())
        .collect()
}

pub fn relevant_terms(words: &[String], text: &str) -> Vec<String> {
    let text = text.to_lowercase();
    words
        .iter()
        .filter(|word| {
            let needle = word.to_lowercase();
            if needle.is_empty() {
                return false;
            }
            text.match_indices(&needle).any(|(index, _)| {
                if !needle.is_ascii() {
                    return true;
                }
                let before = text[..index].chars().next_back();
                let after = text[index + needle.len()..].chars().next();
                !before.is_some_and(|c| c.is_ascii_alphanumeric())
                    && !after.is_some_and(|c| c.is_ascii_alphanumeric())
            })
        })
        .take(MAX_PROMPT_TERMS)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_pack_is_nonempty_valid_and_contains_common_technical_spellings() {
        let terms = terms();
        assert!(terms.len() > 2500 && terms.len() < 4000);
        assert!(terms.contains(&"Azure OpenAI"));
        assert!(terms.contains(&"API"));
        assert!(terms.iter().all(|word| word.chars().count() <= 100));
    }
    #[test]
    fn prompt_terms_are_relevant_bounded_and_do_not_match_inside_english_words() {
        let words = vec![
            "API".into(),
            "IT".into(),
            "Azure OpenAI".into(),
            "无关词语".into(),
        ];
        assert_eq!(
            relevant_terms(&words, "use Azure OpenAI API with a guitar"),
            vec!["API", "Azure OpenAI"]
        );
        assert!(relevant_terms(&words, "nothing relevant").is_empty());
        let large: Vec<String> = (0..100).map(|n| format!("词汇{n}")).collect();
        assert!(relevant_terms(&large, &large.join(" ")).len() <= MAX_PROMPT_TERMS);
    }
}
