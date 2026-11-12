//! Generates the codec module of a fixture (by default the single-component
//! fixture) with the response bound of every member removed. The arguments are
//! the output path, then optionally the fixture directory, its package and the
//! base name of its files, for example `codec-streams streams.demo
//! streams.demo`. RIDL source cannot express a
//! command or a query with no bound, because the compiler applies a default
//! bound, so the case that a client method requires `opts.bound` needs a model
//! edited after the compile. The module is written beside the fixture's
//! domain module, which it imports.

use std::path::Path;

use ridl_backend_ts::generate_codec;
use ridl_ir::codegen;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("expected an output path")?;
    let fixture = args.next().unwrap_or_else(|| "codec-single".to_string());
    let package_name = args.next().unwrap_or_else(|| "single".to_string());
    let base = args.next().unwrap_or_else(|| "single".to_string());
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(&fixture);
    let output = ridlc::compile_workspace(&mut db, &entry)?;
    let package = &output
        .checked
        .iter()
        .find(|checked| checked.ir.name == package_name)
        .ok_or("the fixture declares the package")?
        .ir;
    let mut model = codegen::lower(package, &[&output.std_ir]);
    for interface in &mut model.interfaces {
        for slot in &mut interface.slots {
            if let Some(codegen::v1::interaction_slot::Occupant::Interaction(interaction)) =
                &mut slot.occupant
            {
                interaction.timing = None;
            }
        }
    }
    let module = generate_codec(&model, &base)
        .map_err(|error| format!("{error:?}"))?
        .ok_or("the fixture has a codec module")?;
    std::fs::write(path, module.source)?;
    Ok(())
}
