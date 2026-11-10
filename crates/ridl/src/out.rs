//! Checked writes to stdout for command output.

use std::io::{self, Write as _};
use std::process::ExitCode;

/// Writes and flushes stdout, reporting an I/O failure as exit 2.
pub fn write(text: &str) -> Result<(), ExitCode> {
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|err| {
            let _ = writeln!(io::stderr(), "error: {err}");
            ExitCode::from(2)
        })
}
