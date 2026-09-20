//! The pinned name transforms (ADR-0016 decisions 1 and 2).
//!
//! [`snake_case`] serves every target whose namespace is snake_case, and
//! [`camel_case`] every target whose namespace is CamelCase — the Rust
//! backend's union variants and its induced tuple struct names. They live
//! here rather than in a backend because a projection is a pure function from
//! IR identity to a target's namespace, and because `ridl-ir` is the only
//! crate `ridl-sem` and the backends already depend on.
//!
//! The two are **incomparable**: neither collision set contains the other.
//! `XY` and `x_y` collide under [`camel_case`] and not under [`snake_case`];
//! `HTTPServer` and `httpServer` collide under [`snake_case`] and not under
//! [`camel_case`]. A namespace projected through both is therefore checked
//! under both.

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

/// CamelCase of a snake, screaming-snake, or camel name: `foo_bar` becomes
/// `FooBar`. Used for the Rust backend's union variant names and for the
/// names of its induced tuple structs.
///
/// Each underscore-separated segment has its first character upper-cased and
/// the rest left as written, so an acronym already spelled in capitals keeps
/// them (`httpServer` gives `HttpServer`, `HTTPServer` gives `HTTPServer`).
///
/// **The transform is not injective:** the underscores it removes are what
/// distinguished `foo_bar` from `fooBar`, so the two share an output. A
/// package whose names collide under it is rejected by RIDL-149 (ADR-0016
/// decision 3 as amended), which is where the projection contract's
/// injectivity obligation is discharged.
pub fn camel_case(name: &str) -> String {
    name.split('_')
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

#[cfg(test)]
mod tests {
    use super::{camel_case, snake_case};

    /// Neither collision set contains the other, which is why a union's arms
    /// are checked under both (ADR-0016 amendment, Task 11 decision D).
    #[test]
    fn the_two_transforms_are_incomparable() {
        assert_eq!(camel_case("XY"), camel_case("x_y"));
        assert_ne!(snake_case("XY"), snake_case("x_y"));
        assert_ne!(camel_case("HTTPServer"), camel_case("httpServer"));
        assert_eq!(snake_case("HTTPServer"), snake_case("httpServer"));
    }

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
