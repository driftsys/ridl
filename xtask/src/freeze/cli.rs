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

/// One entry of the `Options:` or `Arguments:` section: the head (the text
/// before the first run of two spaces) and everything the entry prints after
/// it, on the same line and on the lines below.
struct Entry {
    head: String,
    body: String,
}

fn entries(help: &str, section: &str, is_head: fn(usize, &str) -> bool) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let mut in_section = false;
    for line in help.lines() {
        if line == section {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }
        if !line.is_empty() && !line.starts_with("  ") {
            break;
        }
        let indent = line.len() - line.trim_start().len();
        let text = line.trim_start();
        if is_head(indent, text) {
            let (head, rest) = text.split_once("  ").unwrap_or((text, ""));
            out.push(Entry {
                head: head.to_string(),
                body: rest.to_string(),
            });
        } else if let Some(entry) = out.last_mut() {
            entry.body.push('\n');
            entry.body.push_str(text);
        }
    }
    out
}

/// The text of every `[<tag>: ...]` in `body`.
fn tagged<'a>(body: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("[{tag}: ");
    body.match_indices(&open)
        .filter_map(|(at, _)| {
            let rest = &body[at + open.len()..];
            rest.find(']').map(|end| &rest[..end])
        })
        .collect()
}

/// The values an entry lists as possible, from `[possible values: a, b]` and
/// from the `- name: text` lines under `Possible values:`.
fn possible_values(body: &str) -> Vec<String> {
    let mut out: Vec<String> = tagged(body, "possible values")
        .iter()
        .flat_map(|list| list.split(", "))
        .map(str::to_string)
        .collect();
    out.extend(
        body.lines()
            .filter_map(|line| line.strip_prefix("- "))
            .map(|rest| rest.split(':').next().unwrap_or(rest).to_string()),
    );
    out
}

/// `<label> value <v>` for each possible value and `<label> default <v>` for
/// each default of `body`.
fn value_lines(label: &str, body: &str, lines: &mut Vec<String>) {
    for value in possible_values(body) {
        lines.push(format!("{label} value {value}"));
    }
    for default in tagged(body, "default") {
        lines.push(format!("{label} default {default}"));
    }
}

/// The lines of one command: its flags (long and short names, each with the
/// placeholder clap prints or `switch`), the possible values and defaults of
/// each, and its positional arguments in order, each `required` (`<NAME>`) or
/// `optional` (`[NAME]`).
fn command_lines(path: &str, help: &str, lines: &mut Vec<String>) {
    let flags = entries(help, "Options:", |indent, text| {
        indent <= 6 && text.starts_with('-')
    });
    for entry in flags {
        let names: Vec<&str> = entry.head.split(", ").collect();
        let long = names.last().copied().unwrap_or("");
        let mut words = long.split_whitespace();
        let long_name = words.next().unwrap_or("");
        let kind = match words.next() {
            Some(placeholder) if placeholder.starts_with('<') => placeholder,
            _ => "switch",
        };
        for name in &names {
            let name = name.split_whitespace().next().unwrap_or("");
            lines.push(format!("{path} {name} {kind}"));
        }
        value_lines(&format!("{path} {long_name}"), &entry.body, lines);
    }
    let arguments = entries(help, "Arguments:", |indent, text| {
        indent == 2 && (text.starts_with('<') || text.starts_with('['))
    });
    for (index, entry) in arguments.iter().enumerate() {
        let requirement = if entry.head.starts_with('<') {
            "required"
        } else {
            "optional"
        };
        let name = entry.head.trim_matches(|c| "<>[]".contains(c));
        let label = format!("{path} arg {}", index + 1);
        lines.push(format!("{label} <{name}> {requirement}"));
        value_lines(&label, &entry.body, lines);
    }
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
    command_lines(&path, &run.stdout, lines);
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

/// The exit codes ADR-0010 decision 1 lists, each with a command line that
/// produces it: an accepted package, a package with an error, and an unknown
/// flag.
const EXIT_CASES: [(i32, &[&str], &str); 3] = [
    (0, &["check", "."], "warn"),
    (1, &["check", "."], "error"),
    (2, &["check", "--freeze-no-such-flag"], "warn"),
];

/// `exit <n>` for each code of [`EXIT_CASES`] that `ridl` returns for its case.
fn exit_lines(root: &Path) -> Result<Vec<String>, String> {
    let ridl = bin::binary(root, "ridl")?;
    let mut lines = Vec::new();
    for (code, args, fixture) in EXIT_CASES {
        let dir = root.join("xtask/tests/freeze-fixtures").join(fixture);
        let run = bin::run(&ridl, &dir, args)?;
        if run.code == code {
            lines.push(format!("exit {code}"));
        }
    }
    Ok(lines)
}

pub fn dump(root: &Path) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    for program in ["ridl", "ridlc"] {
        let exe = bin::binary(root, program)?;
        walk(&exe, root, program, &[], &mut lines)?;
    }
    lines.extend(exit_lines(root)?);
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

    fn has(lines: &[String], line: &str) -> bool {
        lines.iter().any(|l| l == line)
    }

    #[test]
    fn cli_dump_lists_arguments_values_defaults_and_exits() {
        let lines = lines();
        for line in [
            "ridl lock merge arg 1 <BASE> required",
            "ridl lock merge arg 4 <MARKER_SIZE> required",
            "ridl check arg 1 <PATH> optional",
            "ridl check arg 1 default .",
            "ridl build --emit value rust",
            "ridl build --emit default rust",
            "ridl check --format value sarif",
            "ridl check --format default text",
            "ridl build -h switch",
            "exit 0",
            "exit 1",
            "exit 2",
        ] {
            assert!(has(&lines, line), "missing `{line}`");
        }
    }

    #[test]
    fn arguments_values_defaults_and_short_flags_are_parsed() {
        let help = "Usage: x\n\nArguments:\n  <A>  First\n  [B]  Second [default: .]\n\nOptions:\n  -f, --format <FORMAT>  Output [default: text] [possible values: text, json]\n      --emit <EMIT>\n          Possible values:\n          - rust:  Rust\n          - proto: Proto\n          \n          [default: rust]\n  -h, --help  Print help\n";
        let mut lines = Vec::new();
        command_lines("x", help, &mut lines);
        for line in [
            "x arg 1 <A> required",
            "x arg 2 <B> optional",
            "x arg 2 default .",
            "x -f <FORMAT>",
            "x --format <FORMAT>",
            "x --format value json",
            "x --format default text",
            "x --emit value proto",
            "x --emit default rust",
            "x -h switch",
        ] {
            assert!(has(&lines, line), "missing `{line}` in {lines:?}");
        }
    }
}
