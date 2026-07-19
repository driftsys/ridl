//! The `ridl-lsp` binary: the RIDL language server over stdio (docs/
//! ROADMAP.md epic E1.15a, ADR-0004 §6). All behavior lives in the library;
//! this entry point only wires the stdio transport.

use lsp_server::Connection;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (connection, io_threads) = Connection::stdio();
    ridl_lsp::server::run(connection)?;
    io_threads.join()?;
    Ok(())
}
