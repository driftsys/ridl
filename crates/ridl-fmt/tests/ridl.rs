//! RIDL reference Appendix A: fixed point, token identity and structure at
//! widths 100, 60 and 40, with only D-4 annotation-order normalization.

#[path = "support/invariants.rs"]
mod invariants;

use ridl_fmt::{FormatOptions, FormatOutcome, format};
use ridl_syntax::Profile;

#[test]
fn the_reference_ridl_appendix_formats_idempotently_and_preserves_content() {
    let reference = include_str!("../../../docs/specification/ridl-language-reference.md");
    let appendix = reference
        .split_once("## Appendix A")
        .unwrap()
        .1
        .split_once("## Appendix B")
        .unwrap()
        .0;
    let sources: Vec<_> = appendix
        .split("```ridl\n")
        .skip(1)
        .map(|rest| rest.split_once("```").unwrap().0)
        .collect();
    assert_eq!(sources.len(), 1, "Appendix A holds one whole-file example");
    for source in sources {
        for width in [100, 60, 40] {
            let options = FormatOptions {
                max_line_length: Some(width),
            };
            let FormatOutcome::Formatted(once) = format(source, Profile::Ridl, &options) else {
                panic!("reference must parse")
            };
            assert_eq!(
                format(&once, Profile::Ridl, &options),
                FormatOutcome::Formatted(once.clone()),
                "width {width}"
            );
            assert_eq!(
                invariants::content_tokens(source, Profile::Ridl),
                invariants::content_tokens(&once, Profile::Ridl),
                "comments and tokens, width {width}"
            );
            assert_eq!(
                invariants::syntax_structure(source, Profile::Ridl),
                invariants::syntax_structure(&once, Profile::Ridl),
                "structure, width {width}"
            );
        }
    }
}
