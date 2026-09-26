//! The pinned name transforms (ADR-0016 decisions 1 and 2).
//!
//! [`snake_case`] serves every target whose namespace is snake_case, and
//! [`camel_case`] every target whose namespace is CamelCase — the Rust
//! backend's union variants and its induced tuple struct names.
//! [`pascal_case`], the composition of the two, serves the Rust backend's
//! enum variants. They live here rather than in a backend because a
//! projection is a pure function from IR identity to a target's namespace,
//! and because `ridl-ir` is the only crate `ridl-sem` and the backends
//! already depend on.
//!
//! [`snake_case`] and [`camel_case`] are **incomparable**: neither collision
//! set contains the other. `XY` and `x_y` collide under [`camel_case`] and not
//! under [`snake_case`]; `HTTPServer` and `httpServer` collide under
//! [`snake_case`] and not under [`camel_case`]. A namespace projected through
//! both is therefore checked under both. [`pascal_case`] is [`camel_case`] of
//! [`snake_case`], so its collision set contains [`snake_case`]'s, and a
//! namespace projected through it is checked under it alone.

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
/// **The transform is not injective**, for two reasons. The underscores it
/// removes are what distinguished `foo_bar` from `fooBar`, so those two share
/// an output; and upper-casing a segment's first character destroys the case
/// that distinguished `fooBar` from `FooBar`, so those two do as well. A
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

/// PascalCase of any name the lexer admits: `CHECK_ENGINE` becomes
/// `CheckEngine`. Used for the Rust backend's enum variant names (ADR-0016,
/// 2026-09-26 amendment).
///
/// It is [`camel_case`] of [`snake_case`]: `snake_case` lower-cases the name
/// and separates its words, and `camel_case` then upper-cases the first
/// character of each word and removes the separators. [`camel_case`] alone
/// does not serve, because it leaves each segment's tail as written and so
/// gives `CHECKENGINE`. Composing the two also defines the result for a name
/// outside the typl convention: `checkEngine` gives `CheckEngine`.
///
/// **The transform is not injective**, and its collision set contains
/// [`snake_case`]'s: two names that share a `snake_case` output share this
/// one, because this is a function of that output. The converse fails —
/// `CHECK_ENGINE` and `CHECK__ENGINE` collide here only. So RIDL-149 checks
/// an enum's values under this transform alone. It is not idempotent either:
/// `A_B` gives `AB`, and `AB` gives `Ab`. Nothing applies it twice.
pub fn pascal_case(name: &str) -> String {
    camel_case(&snake_case(name))
}

#[cfg(test)]
mod tests {
    use super::{camel_case, pascal_case, snake_case};

    /// The outputs the docstring names, pinned as values. The relational
    /// tests below constrain which names share an output, not what that
    /// output is, so without these a transform composed with any injective
    /// suffix would satisfy them.
    #[test]
    fn camel_case_pins_the_outputs_its_docstring_names() {
        assert_eq!(camel_case("httpServer"), "HttpServer");
        assert_eq!(camel_case("HTTPServer"), "HTTPServer");
        assert_eq!(camel_case("foo_bar"), "FooBar");
    }

    /// The two sources of non-injectivity the docstring names: the removed
    /// underscore, and the upper-cased first character.
    #[test]
    fn camel_case_is_not_injective_in_two_ways() {
        assert_eq!(camel_case("foo_bar"), camel_case("fooBar"));
        assert_eq!(camel_case("fooBar"), camel_case("FooBar"));
    }

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

    /// The outputs `docs/archive/2026-09-26-enum-variant-pascal-case-design.md` §3 names,
    /// pinned as values.
    #[test]
    fn pascal_case_pins_the_outputs_the_design_names() {
        for (input, expected) in [
            ("CHECK_ENGINE", "CheckEngine"),
            ("PARK", "Park"),
            ("OK", "Ok"),
            ("A", "A"),
            ("X2", "X2"),
            ("ABS_V2", "AbsV2"),
            ("V2_ABS", "V2Abs"),
            ("LEVEL_10", "Level10"),
            ("HTTP_SERVER", "HttpServer"),
            ("A__B", "AB"),
            ("A_", "A"),
            ("checkEngine", "CheckEngine"),
            ("HTTPServer", "HttpServer"),
            ("SELF", "Self"),
        ] {
            assert_eq!(pascal_case(input), expected, "pascal_case(`{input}`)");
        }
    }

    /// Every name of one to five characters over `a`, `B`, `_` and `2` that
    /// starts with a letter, as the lexer requires.
    fn enumerated_names() -> Vec<String> {
        let alphabet = ['a', 'B', '_', '2'];
        let mut names: Vec<String> = vec!["a".to_string(), "B".to_string()];
        let mut frontier = names.clone();
        for _ in 1..5 {
            let mut next = Vec::new();
            for name in &frontier {
                for c in alphabet {
                    next.push(format!("{name}{c}"));
                }
            }
            names.extend(next.iter().cloned());
            frontier = next;
        }
        names
    }

    /// Property 4 of `docs/archive/2026-09-26-enum-variant-pascal-case-design.md` §3:
    /// two names that share a `snake_case` output share a `pascal_case`
    /// output. This is why RIDL-149 keys an enum's values on `pascal_case`
    /// alone.
    #[test]
    fn pascal_case_collides_wherever_snake_case_does() {
        let mut by_snake: std::collections::HashMap<String, (String, String)> =
            std::collections::HashMap::new();
        for name in enumerated_names() {
            let pascal = pascal_case(&name);
            let (first, first_pascal) = by_snake
                .entry(snake_case(&name))
                .or_insert_with(|| (name.clone(), pascal.clone()));
            assert_eq!(
                *first_pascal, pascal,
                "`{first}` and `{name}` share a snake_case output but not a pascal_case one"
            );
        }
    }

    /// The containment is strict: this pair differs under `snake_case` and
    /// collides under `pascal_case`.
    #[test]
    fn pascal_case_collides_where_snake_case_does_not() {
        assert_ne!(snake_case("CHECK_ENGINE"), snake_case("CHECK__ENGINE"));
        assert_eq!(pascal_case("CHECK_ENGINE"), pascal_case("CHECK__ENGINE"));
    }

    /// Property 2 of `docs/archive/2026-09-26-enum-variant-pascal-case-design.md` §3:
    /// every output is a name rustc's `non_camel_case_types` accepts —
    /// non-empty, no underscore, and an upper-case first character.
    #[test]
    fn every_pascal_case_output_satisfies_non_camel_case_types() {
        for name in enumerated_names() {
            let pascal = pascal_case(&name);
            assert!(!pascal.is_empty(), "`{name}` gives an empty name");
            assert!(!pascal.contains('_'), "`{name}` gives `{pascal}`");
            assert!(
                pascal.starts_with(|c: char| c.is_ascii_uppercase()),
                "`{name}` gives `{pascal}`"
            );
        }
    }
}
