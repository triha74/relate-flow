use crate::error::FlowError;
use crate::model::Pair;
use crate::slug::is_slug;

/// Parse `produce=token`. Both sides are slugs. Nothing else is accepted.
pub fn parse_pair(text: &str) -> Result<Pair, FlowError> {
    let Some((produce, token)) = text.split_once('=') else {
        return Err(bad(text));
    };
    if produce.is_empty() || token.is_empty() || text.matches('=').count() != 1 {
        return Err(bad(text));
    }
    if !is_slug(produce) || !is_slug(token) {
        return Err(bad(text));
    }
    Ok(Pair {
        produce: produce.to_string(),
        token: token.to_string(),
    })
}

/// One `--when-any` flag: commas AND the pairs.
pub fn parse_alternative(text: &str) -> Result<Vec<Pair>, FlowError> {
    if text.is_empty() {
        return Err(bad(text));
    }
    let mut pairs = Vec::new();
    for piece in text.split(',') {
        pairs.push(parse_pair(piece)?);
    }
    Ok(pairs)
}

fn bad(text: &str) -> FlowError {
    FlowError::usage(
        format!("condition '{text}' is not produce=token"),
        "pass produce=token, with a slug on each side",
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_alternative, parse_pair};

    #[test]
    fn pair_and_alternative() {
        let pair = parse_pair("vendor=approved").unwrap();
        assert_eq!(pair.produce, "vendor");
        assert_eq!(pair.token, "approved");
        let alt = parse_alternative("discounts=correct,vendor=approved").unwrap();
        assert_eq!(alt.len(), 2);
        assert_eq!(alt[1].token, "approved");
    }

    #[test]
    fn rejects_prose_and_empty_sides() {
        assert!(parse_pair("vendor").is_err());
        assert!(parse_pair("=approved").is_err());
        assert!(parse_pair("Vendor=approved").is_err());
        assert!(parse_pair("vendor=approved=extra").is_err());
        assert!(parse_alternative("").is_err());
        assert!(parse_alternative("vendor=approved,").is_err());
    }
}
