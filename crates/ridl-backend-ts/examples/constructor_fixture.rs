//! Generates the constructor fixture's integer cases that RIDL source cannot
//! express. An unconstrained integer in source always has I64 width, and the
//! checker refuses `step` on an integer (TYPL-105), so these cases need IR.

use ridl_ir::v2;

/// An integer declaration with the given width and constraint.
fn integer(
    name: &str,
    doc: &str,
    width: v2::IntWidth,
    constraint: Option<v2::Constraint>,
) -> v2::Decl {
    v2::Decl {
        name: name.into(),
        visibility: v2::Visibility::Public as i32,
        doc: doc.into(),
        kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Integer as i32,
                )),
            }),
            width: Some(v2::type_def::Width::IntWidth(width as i32)),
            constraint,
            ..Default::default()
        })),
        ..Default::default()
    }
}

fn grid(min: Option<&str>, max: Option<&str>, step: &str) -> Option<v2::Constraint> {
    Some(v2::Constraint {
        min: min.map(str::to_string),
        max: max.map(str::to_string),
        step: Some(step.to_string()),
        ..Default::default()
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected an output path")?;
    let package = v2::Package {
        name: "ctor".into(),
        decls: vec![
            integer(
                "SafeNumber",
                "An integer without range comparisons, for the safe-integer check.",
                v2::IntWidth::I32,
                None,
            ),
            integer(
                "Stepped",
                "An integer on a grid that starts at its minimum.",
                v2::IntWidth::I32,
                grid(Some("-10"), Some("20"), "5"),
            ),
            integer(
                "FromZero",
                "An integer on a grid that starts at zero, because it has no minimum.",
                v2::IntWidth::I32,
                grid(None, Some("20"), "5"),
            ),
            integer(
                "Unbounded32",
                "An integer with a grid and no maximum, so its values reach the safe limit.",
                v2::IntWidth::I32,
                grid(Some("-10"), None, "5"),
            ),
            integer(
                "HugeStep",
                "An integer whose step is beyond the safe number interval.",
                v2::IntWidth::I32,
                grid(Some("-3"), Some("100"), "100000000000000000000"),
            ),
            integer(
                "WideStepped",
                "A bigint-backed integer on a grid that starts at its odd minimum.",
                v2::IntWidth::I64,
                grid(Some("-9007199254740993"), Some("9007199254740993"), "4"),
            ),
            integer(
                "WideFromZero",
                "An unsigned bigint-backed integer on a grid that starts at zero.",
                v2::IntWidth::U64,
                grid(None, None, "3"),
            ),
        ],
        ..Default::default()
    };
    let module = ridl_backend_ts::generate(&package).map_err(|error| format!("{error:?}"))?;
    std::fs::write(path, module.source)?;
    Ok(())
}
