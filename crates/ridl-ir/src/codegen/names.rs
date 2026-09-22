//! The name transforms the model carries (design note D-2).

use super::v1;
use crate::name::{camel_case, snake_case};

/// Every namespace a target can need for one declared identifier.
pub(crate) fn spellings(declared: &str) -> v1::Spellings {
    let snake = snake_case(declared);
    v1::Spellings {
        declared: declared.to_string(),
        camel: camel_case(declared),
        screaming: snake.to_uppercase(),
        snake,
    }
}

/// A package name or a service name, in the four forms a target reads.
pub(crate) fn dotted_name(dotted: &str) -> v1::DottedName {
    let segments: Vec<String> = dotted
        .split('.')
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect();
    v1::DottedName {
        dotted: dotted.to_string(),
        joined_camel: joined_camel(dotted),
        underscored: segments.join("_"),
        segments,
    }
}

/// The wire backends' `type_name`: one CamelCase identifier from a dotted
/// address, `corpus.baseline.hvac` giving `CorpusBaselineHvac`. ADR-0016 does
/// not pin it, and `camel_case` does not compute it — it splits on
/// underscores, not on dots (design note §9 item 3).
pub(crate) fn joined_camel(dotted: &str) -> String {
    dotted
        .split('.')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}
