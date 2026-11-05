//! `encoding-bench-report FILE...` reads result JSON files of the encoding
//! bench and writes Markdown tables to standard output, one section per file.

use std::process::ExitCode;

fn main() -> ExitCode {
    let files: Vec<String> = std::env::args().skip(1).collect();
    if files.is_empty() {
        eprintln!("usage: encoding-bench-report RESULT.json...");
        return ExitCode::from(2);
    }
    let mut out = String::new();
    for file in &files {
        let text = match std::fs::read_to_string(file) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("read {file}: {e}");
                return ExitCode::from(1);
            }
        };
        match encoding_bench_report::parse(&text) {
            Ok(result) => out.push_str(&encoding_bench_report::render(&name_of(file), &result)),
            Err(e) => {
                eprintln!("{file}: {e}");
                return ExitCode::from(1);
            }
        }
    }
    print!("{out}");
    ExitCode::SUCCESS
}

/// The file name without its directory, for the title of a section.
fn name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned())
}
