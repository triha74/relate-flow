/// A slug is a lowercase letter followed by lowercase letters, digits, and hyphens.
pub fn is_slug(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::is_slug;

    #[test]
    fn accepts_the_grammar() {
        assert!(is_slug("a"));
        assert!(is_slug("parse"));
        assert!(is_slug("verify-vendor"));
        assert!(is_slug("a1"));
        assert!(is_slug("sev1"));
    }

    #[test]
    fn rejects_anything_else() {
        for bad in [
            "",
            "A",
            "1a",
            "-a",
            "Parse",
            "has_under",
            "has space",
            "a.b",
        ] {
            assert!(!is_slug(bad), "{bad}");
        }
    }
}
