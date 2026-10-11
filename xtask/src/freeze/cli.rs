//! The command line of `ridl` and `ridlc`: subcommands, flags with their value
//! kind, the exit codes, and the key paths of the `json` and `sarif` reports.

use super::bin;
use serde_json::Value;
use std::path::Path;

/// The subcommands a help text lists under `Commands:`, without `help`. The
/// same parse as `crates/ridl/tests/cli_reference_support/mod.rs`.
fn listed_commands(help: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_section = false;
    for line in help.lines() {
        if line == "Commands:" {
            in_section = true;
        } else if in_section {
            if !line.starts_with("  ") {
                break;
            }
            // A command line starts at two spaces; a continuation is deeper.
            if !line.starts_with("   ")
                && let Some(name) = line.split_whitespace().next()
                && name != "help"
            {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// The flags a help text lists under `Options:`, as `(flag, value kind)`. The
/// value kind is the placeholder clap prints, or `switch`. A flag with a long
/// name is written by that name; a flag with only a short name by the short.
fn listed_flags(help: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut in_section = false;
    for line in help.lines() {
        if line == "Options:" {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }
        if !line.is_empty() && !line.starts_with("  ") {
            break;
        }
        // A flag line is indented by two spaces (with a short name) or six
        // (without); description lines are indented deeper.
        let indent = line.len() - line.trim_start().len();
        let text = line.trim_start();
        if indent > 6 || !text.starts_with('-') {
            continue;
        }
        let mut names = text.split("  ").next().unwrap_or("").split(", ");
        let first = names.next().unwrap_or("");
        let flag = names.next().unwrap_or(first);
        let mut words = flag.split_whitespace();
        let Some(name) = words.next() else { continue };
        let kind = match words.next() {
            Some(placeholder) if placeholder.starts_with('<') => placeholder,
            _ => "switch",
        };
        out.push((name.to_string(), kind.to_string()));
    }
    out
}

fn walk(
    exe: &Path,
    dir: &Path,
    program: &str,
    prefix: &[String],
    lines: &mut Vec<String>,
) -> Result<(), String> {
    let mut args: Vec<&str> = prefix.iter().map(String::as_str).collect();
    args.push("--help");
    let run = bin::run(exe, dir, &args)?;
    if run.code != 0 {
        return Err(format!(
            "`{program} {}` exited {}",
            args.join(" "),
            run.code
        ));
    }
    let path = std::iter::once(program.to_string())
        .chain(prefix.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ");
    for (flag, kind) in listed_flags(&run.stdout) {
        lines.push(format!("{path} {flag} {kind}"));
    }
    for name in listed_commands(&run.stdout) {
        lines.push(format!("{path} {name}"));
        let mut next = prefix.to_vec();
        next.push(name);
        walk(exe, dir, program, &next, lines)?;
    }
    Ok(())
}

/// Every key path of `value`, with array indices written as `*`.
fn pointers(value: &Value, prefix: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let pointer = format!("{prefix}/{}", key.replace('~', "~0").replace('/', "~1"));
                out.push(pointer.clone());
                pointers(child, &pointer, out);
            }
        }
        Value::Array(items) => {
            let pointer = format!("{prefix}/*");
            out.push(pointer.clone());
            for item in items {
                pointers(item, &pointer, out);
            }
        }
        _ => {}
    }
}

/// `<kind> <pointer>` for every key path of the report that `ridl check
/// --format <kind>` prints on the package that draws one warning.
fn report_pointers(root: &Path, kind: &str) -> Result<Vec<String>, String> {
    let ridl = bin::binary(root, "ridl")?;
    let fixture = root.join("xtask/tests/freeze-fixtures/warn");
    let run = bin::run(&ridl, &fixture, &["check", "--format", kind, "."])?;
    if run.code != 0 {
        return Err(format!(
            "`ridl check --format {kind}` on the warn fixture exited {}:\n{}",
            run.code, run.stderr
        ));
    }
    let report: Value = serde_json::from_str(&run.stdout)
        .map_err(|e| format!("`ridl check --format {kind}` printed no JSON: {e}"))?;
    let mut out = Vec::new();
    pointers(&report, "", &mut out);
    Ok(out.into_iter().map(|p| format!("{kind} {p}")).collect())
}

/// The exit codes ADR-0010 decision 1 lists: 0, 1 and 2.
const EXIT_CODES: [u8; 3] = [0, 1, 2];

pub fn dump(root: &Path) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    for program in ["ridl", "ridlc"] {
        let exe = bin::binary(root, program)?;
        walk(&exe, root, program, &[], &mut lines)?;
    }
    for code in EXIT_CODES {
        lines.push(format!("exit {code}"));
    }
    for kind in ["json", "sarif"] {
        lines.extend(report_pointers(root, kind)?);
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines() -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        dump(root).unwrap()
    }

    #[test]
    fn cli_dump_lists_ridl_build_out_dir() {
        let lines = lines();
        assert!(
            lines.iter().any(|l| l.starts_with("ridl build --out-dir")),
            "{lines:?}"
        );
    }

    #[test]
    fn cli_dump_lists_json_code_key() {
        let lines = lines();
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("json ") && l.ends_with("/code")),
            "{lines:?}"
        );
    }

    #[test]
    fn flags_carry_their_value_kind() {
        let help = "Options:\n      --out-dir <OUT_DIR>\n          [default: out]\n\n      --frozen\n          Verify\n\n  -h, --help     Print help\n  -V, --version  Print version\n";
        assert_eq!(
            listed_flags(help),
            vec![
                ("--out-dir".to_string(), "<OUT_DIR>".to_string()),
                ("--frozen".to_string(), "switch".to_string()),
                ("--help".to_string(), "switch".to_string()),
                ("--version".to_string(), "switch".to_string()),
            ]
        );
    }
}
