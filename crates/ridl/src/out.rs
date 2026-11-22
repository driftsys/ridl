//! Checked writes to stdout for command output, and unchecked writes to stderr
//! for messages.

use std::fmt;
use std::io::{self, Write};
use std::process::ExitCode;

/// Writes and flushes stdout, reporting an I/O failure as exit 2.
pub fn write(text: &str) -> Result<(), ExitCode> {
    let mut stdout = io::stdout().lock();
    write_to(&mut stdout, text).map_err(|err| {
        let _ = writeln!(io::stderr(), "error: {err}");
        ExitCode::from(2)
    })
}

/// Writes formatted text to stderr and ignores a write error. A closed stderr
/// reader (EPIPE) must not panic the way `eprint!` does, so the command keeps
/// the exit code it intended.
pub fn write_err(args: fmt::Arguments<'_>) {
    let _ = io::stderr().write_fmt(args);
}

/// Like `eprint!`, but ignores a write error.
macro_rules! errprint {
    ($($arg:tt)*) => {
        $crate::out::write_err(format_args!($($arg)*))
    };
}

/// Like `eprintln!`, but ignores a write error.
macro_rules! errprintln {
    () => {
        $crate::out::write_err(format_args!("\n"))
    };
    ($($arg:tt)*) => {
        $crate::out::write_err(format_args!("{}\n", format_args!($($arg)*)))
    };
}

fn write_to(out: &mut impl Write, text: &str) -> io::Result<()> {
    out.write_all(text.as_bytes())?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct BufferedWriter {
        pending: Vec<u8>,
        visible: Vec<u8>,
    }

    impl Write for BufferedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.pending.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.visible.append(&mut self.pending);
            Ok(())
        }
    }

    #[test]
    fn flushes_text_without_final_newline() {
        let mut out = BufferedWriter::default();

        write_to(&mut out, "output without a newline").unwrap();

        assert_eq!(out.visible, b"output without a newline");
        assert!(out.pending.is_empty());
    }

    struct FlushFailure;

    impl Write for FlushFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "flush failed"))
        }
    }

    #[test]
    fn returns_flush_error() {
        let err = write_to(&mut FlushFailure, "output").unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(err.to_string(), "flush failed");
    }
}
