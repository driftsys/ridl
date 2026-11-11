//! Generates the codec module of the single-component fixture with the
//! response bound of every member removed. RIDL source cannot express a
//! command or a query with no bound, because the compiler applies a default
//! bound, so the case that a client method requires `opts.bound` needs a model
//! edited after the compile. The module is written beside the fixture's
//! `single.ts`, which it imports.

use std::path::Path;

use ridl_backend_ts::generate_codec;
use ridl_ir::codegen;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected an output path")?;
    let mut db = ridl_core::RidlDatabase::default();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codec-single");
    let output = ridlc::compile_workspace(&mut db, &entry)?;
    let package = &output
        .checked
        .iter()
        .find(|checked| checked.ir.name == "single")
        .ok_or("the fixture declares single")?
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
    let module = generate_codec(&model, "single")
        .map_err(|error| format!("{error:?}"))?
        .ok_or("the fixture has a codec module")?;
    std::fs::write(path, module.source)?;
    Ok(())
}
