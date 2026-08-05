//! The pinned name transform (ADR-0016 decisions 1 and 2).
//!
//! One transform serves every backend whose target namespace is snake_case.
//! It lives here rather than in a backend because it is a projection — a pure
//! function from IR identity to a target's namespace — and because `ridl-ir`
//! is the only crate `ridl-sem` and both backends already depend on.

/// snake_case of a ridl name: `currentSpeed` becomes `current_speed`.
///
/// A separator is inserted before an upper-case character that follows a
/// lower-case character or a digit, or that follows an upper-case character
/// and is itself followed by a lower-case character. So an acronym that runs
/// to the end of a name stays one word (`getVIN` gives `get_vin`), while an
/// acronym followed by a word splits (`HTTPServer` gives `http_server`). An
/// underscore already present is kept, and the mapping is stable under
/// repeated application.
///
/// **The transform is not injective, and no case-folding transform can be:**
/// lowercasing destroys what distinguishes two identifiers, so
/// `parseHTTPResponse` and `parseHttpResponse` share an output. A package
/// whose names collide under it is rejected by RIDL-149 (ADR-0016 decision 3),
/// which is where the projection contract's injectivity obligation is
/// discharged.
pub fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (index, &current) in chars.iter().enumerate() {
        if current.is_uppercase() && index > 0 {
            let previous = chars[index - 1];
            let next_lower = chars.get(index + 1).is_some_and(|c| c.is_lowercase());
            if previous.is_lowercase()
                || previous.is_numeric()
                || (previous.is_uppercase() && next_lower)
            {
                out.push('_');
            }
        }
        out.extend(current.to_lowercase());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::snake_case;

    #[test]
    fn an_acronym_stays_one_word() {
        assert_eq!(snake_case("getVIN"), "get_vin");
        assert_eq!(snake_case("ABC"), "abc");
    }

    #[test]
    fn an_acronym_followed_by_a_word_splits() {
        assert_eq!(snake_case("HTTPServer"), "http_server");
        assert_eq!(snake_case("IOError"), "io_error");
        assert_eq!(snake_case("parseHTTPResponse"), "parse_http_response");
    }

    #[test]
    fn a_camel_case_name_splits_on_every_boundary() {
        assert_eq!(snake_case("currentSpeed"), "current_speed");
        assert_eq!(snake_case("speed2Target"), "speed2_target");
        assert_eq!(snake_case("aB"), "a_b");
    }

    #[test]
    fn an_underscore_already_present_is_kept() {
        assert_eq!(snake_case("already_snake"), "already_snake");
        assert_eq!(snake_case("mixed_CaseName"), "mixed_case_name");
    }

    #[test]
    fn the_transform_is_idempotent() {
        for name in [
            "getVIN",
            "HTTPServer",
            "currentSpeed",
            "mixed_CaseName",
            "a1B2",
        ] {
            let once = snake_case(name);
            assert_eq!(snake_case(&once), once, "not idempotent on `{name}`");
        }
    }
}
