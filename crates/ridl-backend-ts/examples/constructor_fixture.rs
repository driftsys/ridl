//! Generates the constructor fixture's unconstrained number-backed integer.
//! RIDL source gives an unconstrained integer I64 width, so this case needs IR.

use ridl_ir::v2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected an output path")?;
    let package = v2::Package {
        name: "ctor".into(),
        decls: vec![v2::Decl {
            name: "SafeNumber".into(),
            visibility: v2::Visibility::Public as i32,
            doc: "An integer without range comparisons, for the safe-integer check.".into(),
            kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(
                        v2::PrimitiveType::Integer as i32,
                    )),
                }),
                width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::I32 as i32)),
                ..Default::default()
            })),
            ..Default::default()
        }],
        ..Default::default()
    };
    let module = ridl_backend_ts::generate(&package).map_err(|error| format!("{error:?}"))?;
    std::fs::write(path, module.source)?;
    Ok(())
}
