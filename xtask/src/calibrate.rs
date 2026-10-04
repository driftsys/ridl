//! Offline calibration of design checks through the public CLI JSON contract.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const CHECKS: [&str; 5] = [
    "inconsistent-unit",
    "inconsistent-abbreviation",
    "duplicate-shape",
    "low-cohesion-interface",
    "package-fan-out",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Metric {
    #[serde(rename = "struct")]
    Struct { count: usize },
    #[serde(rename = "enum")]
    Enum { count: usize },
    #[serde(rename = "cohesion")]
    Cohesion {
        groups: usize,
        min_group_size: usize,
    },
    #[serde(rename = "fan-out")]
    FanOut { count: usize },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Finding {
    id: String,
    workspace: String,
    location: String,
    message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    metric: Option<Metric>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    claude: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    claude_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sol_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    r#final: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Threshold {
    None,
    Shape(usize, usize),
    Cohesion(usize, usize),
    FanOut(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Dropped,
    Info,
    Warning,
}
#[derive(Debug)]
struct Candidate {
    threshold: Threshold,
    findings: usize,
    accepted: usize,
    detected: usize,
    applicable: usize,
    level: Level,
}
impl Candidate {
    fn recall(&self) -> String {
        if self.applicable == 0 {
            "not applicable".into()
        } else {
            format!(
                "{:.2}%",
                self.detected as f64 * 100.0 / self.applicable as f64
            )
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IssueRow {
    id: String,
    applicable: bool,
    reason: String,
    findings: Vec<String>,
}
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Dump(PathBuf),
    Derive { write: bool },
    Help,
}

fn number(text: &str) -> Result<usize> {
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(format!("invalid unsigned integer `{text}`").into());
    }
    Ok(text.parse()?)
}

fn positive(text: &str) -> Result<usize> {
    let n = number(text)?;
    if n == 0 || n == usize::MAX {
        return Err("metric coordinates must be positive and permit a next threshold".into());
    }
    Ok(n)
}

fn metric_from_message(check: &str, message: &str) -> Result<Option<Metric>> {
    let missing = || format!("missing or malformed metric for {check}: {message}");
    let metric = match check {
        "inconsistent-unit" | "inconsistent-abbreviation" => return Ok(None),
        "duplicate-shape" => {
            let (name, rest) = message.split_once(" has the same ").ok_or_else(missing)?;
            if !name.starts_with('`') || !name.ends_with('`') {
                return Err(missing().into());
            }
            let (count, rest) = rest.split_once(' ').ok_or_else(missing)?;
            let count = positive(count)?;
            if rest.starts_with("fields as `") && rest.ends_with('`') {
                Metric::Struct { count }
            } else if rest.starts_with("variants as `") && rest.ends_with('`') {
                Metric::Enum { count }
            } else {
                return Err(missing().into());
            }
        }
        "low-cohesion-interface" => {
            let (name, rest) = message.split_once(" splits into ").ok_or_else(missing)?;
            if !name.starts_with("interface `") || !name.ends_with('`') {
                return Err(missing().into());
            }
            let (count, listed) = rest
                .split_once(" groups of members that share no type: ")
                .ok_or_else(missing)?;
            let groups = positive(count)?;
            let mut sizes = Vec::new();
            let mut members = BTreeSet::new();
            let listed = listed.strip_prefix('[').ok_or_else(missing)?;
            for group in listed.split(", [") {
                let group = group.strip_suffix(']').ok_or_else(missing)?;
                let mut size = 0;
                for member in group.split(", ") {
                    if member.is_empty()
                        || !member.chars().all(|c| c.is_alphanumeric() || c == '_')
                        || !members.insert(member)
                    {
                        return Err(missing().into());
                    }
                    size += 1;
                }
                sizes.push(size);
            }
            if sizes.len() != groups {
                return Err(missing().into());
            }
            Metric::Cohesion {
                groups,
                min_group_size: *sizes.iter().min().ok_or_else(missing)?,
            }
        }
        "package-fan-out" => {
            let (name, rest) = message.split_once(" depends on ").ok_or_else(missing)?;
            if !name.starts_with("package `") || !name.ends_with('`') {
                return Err(missing().into());
            }
            let (count, listed) = rest
                .split_once(" workspace packages: ")
                .ok_or_else(missing)?;
            let count = positive(count)?;
            let names: Vec<_> = listed.split(", ").collect();
            if names.len() != count
                || names.iter().any(|n| n.is_empty())
                || names.iter().collect::<BTreeSet<_>>().len() != count
            {
                return Err(missing().into());
            }
            Metric::FanOut { count }
        }
        _ => return Err(format!("unknown check `{check}`").into()),
    };
    Ok(Some(metric))
}

fn retained(threshold: Threshold, finding: &Finding) -> bool {
    match (threshold, &finding.metric) {
        (Threshold::None, None) => true,
        (Threshold::Shape(fields, _), Some(Metric::Struct { count })) => *count >= fields,
        (Threshold::Shape(_, variants), Some(Metric::Enum { count })) => *count >= variants,
        (
            Threshold::Cohesion(g, s),
            Some(Metric::Cohesion {
                groups,
                min_group_size,
            }),
        ) => *groups >= g && *min_group_size >= s,
        (Threshold::FanOut(max), Some(Metric::FanOut { count })) => *count > max,
        _ => false,
    }
}

fn candidates(check: &str, findings: &[Finding], issues: &[IssueRow]) -> Vec<Candidate> {
    // Only boundaries that change the retained set are needed. The first
    // value after an observed coordinate is the least strict such boundary.
    let thresholds = match check {
        "duplicate-shape" => {
            let mut fields = BTreeSet::from([2]);
            let mut variants = BTreeSet::from([2]);
            for finding in findings {
                match finding.metric {
                    Some(Metric::Struct { count }) => {
                        fields.insert(count + 1);
                    }
                    Some(Metric::Enum { count }) => {
                        variants.insert(count + 1);
                    }
                    _ => {}
                }
            }
            fields
                .into_iter()
                .flat_map(|f| variants.iter().map(move |&v| Threshold::Shape(f, v)))
                .collect::<Vec<_>>()
        }
        "low-cohesion-interface" => {
            let mut groups = BTreeSet::from([2]);
            let mut sizes = BTreeSet::from([1]);
            for finding in findings {
                if let Some(Metric::Cohesion {
                    groups: g,
                    min_group_size: s,
                }) = finding.metric
                {
                    groups.insert(g + 1);
                    sizes.insert(s + 1);
                }
            }
            groups
                .into_iter()
                .flat_map(|g| sizes.iter().map(move |&s| Threshold::Cohesion(g, s)))
                .collect()
        }
        "package-fan-out" => {
            let mut values = BTreeSet::from([3]);
            for finding in findings {
                if let Some(Metric::FanOut { count }) = finding.metric {
                    values.insert(count);
                }
            }
            values.into_iter().map(Threshold::FanOut).collect()
        }
        _ => vec![Threshold::None],
    };
    thresholds
        .into_iter()
        .map(|threshold| {
            let retained: Vec<_> = findings.iter().filter(|f| retained(threshold, f)).collect();
            let findings = retained.len();
            let accepted = retained
                .iter()
                .filter(|f| f.r#final.as_deref() == Some("accept"))
                .count();
            let ids: BTreeSet<_> = retained.iter().map(|f| f.id.as_str()).collect();
            let applicable = issues.iter().filter(|i| i.applicable).count();
            let detected = issues
                .iter()
                .filter(|i| i.applicable && i.findings.iter().any(|id| ids.contains(id.as_str())))
                .count();
            let level = if findings == 0 || accepted * 100 < findings * 50 {
                Level::Dropped
            } else if findings >= 10 && accepted * 100 >= findings * 80 {
                Level::Warning
            } else {
                Level::Info
            };
            Candidate {
                threshold,
                findings,
                accepted,
                applicable,
                detected,
                level,
            }
        })
        .collect()
}

fn choose(rows: &[Candidate]) -> Option<&Candidate> {
    let level = rows.iter().map(|r| r.level).max()?;
    if level == Level::Dropped {
        return None;
    }
    rows.iter()
        .filter(|r| r.level == level)
        .min_by_key(|r| (std::cmp::Reverse(r.findings), r.threshold))
}

const HELP: &str = "usage: cargo xtask <codegen|descriptor-codegen|calibrate>\n\
  cargo xtask calibrate dump <out-dir>\n\
  cargo xtask calibrate derive [--write]\n\
  cargo xtask calibrate --derive [--write] (alias)\n\
Derive validates adjudicated labels and the reviewed recall join. --write also\n\
writes evals/calibration/summary.md. Dump uses copied corpus workspaces and\n\
a separate build target directory inside <out-dir>.";

fn parse_args(args: &[String]) -> Result<Action> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["dump", out] if !out.starts_with('-') => Ok(Action::Dump(out.into())),
        ["derive" | "--derive"] => Ok(Action::Derive { write: false }),
        ["derive" | "--derive", "--write"] => Ok(Action::Derive { write: true }),
        ["--help" | "-h"] => Ok(Action::Help),
        _ => Err(HELP.into()),
    }
}

pub(crate) fn run(args: &[String]) -> Result<()> {
    // Calibration reads the workspace selected by the caller, as documented.
    run_at(&std::env::current_dir()?, args)
}

fn run_at(root: &Path, args: &[String]) -> Result<()> {
    match parse_args(args)? {
        Action::Help => println!("{HELP}"),
        Action::Dump(out) => dump(root, &out)?,
        Action::Derive { write } => {
            let summary = read_calibration(root)?;
            if write {
                std::fs::write(root.join("evals/calibration/summary.md"), &summary)?;
            }
            print!("{summary}");
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct Position {
    line: usize,
    column: usize,
}
#[derive(Deserialize)]
struct JsonSpan {
    path: PathBuf,
    start: Position,
    end: Position,
}
#[derive(Deserialize)]
struct JsonDiagnostic {
    severity: String,
    lint: Option<String>,
    message: String,
    span: JsonSpan,
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = if path.is_absolute() {
        path.strip_prefix(root)?
    } else {
        path
    };
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(format!("source path is outside workspace: {}", path.display()).into());
    }
    let text = relative
        .to_str()
        .ok_or("source path is not UTF-8")?
        .replace('\\', "/");
    if text.contains(':') {
        return Err("source path contains an ID delimiter".into());
    }
    Ok(text)
}

fn byte_offset(text: &str, position: &Position) -> Result<usize> {
    if position.line == 0 || position.column == 0 {
        return Err("source positions are one-based".into());
    }
    let mut offset = 0;
    for (index, line) in text.split('\n').enumerate() {
        if index + 1 == position.line {
            let column = position.column - 1;
            if column == line.chars().count() {
                return Ok(offset + line.len());
            }
            return line
                .char_indices()
                .nth(column)
                .map(|(byte, _)| offset + byte)
                .ok_or_else(|| "column outside source line".into());
        }
        offset += line.len() + 1;
    }
    Err("line outside source file".into())
}

fn records_from_json(
    root: &Path,
    workspace: &str,
    json: &[u8],
) -> Result<BTreeMap<String, Vec<Finding>>> {
    let diagnostics: Vec<JsonDiagnostic> = serde_json::from_slice(json)?;
    let mut raw = Vec::new();
    for diagnostic in diagnostics {
        if diagnostic.severity == "error" {
            return Err(format!("corpus has an error: {}", diagnostic.message).into());
        }
        let Some(check) = diagnostic.lint.filter(|l| CHECKS.contains(&l.as_str())) else {
            continue;
        };
        let relative = relative_path(root, &diagnostic.span.path)?;
        let text = std::fs::read_to_string(root.join(&relative))?;
        let start = byte_offset(&text, &diagnostic.span.start)?;
        let end = byte_offset(&text, &diagnostic.span.end)?;
        if start > end {
            return Err("reversed diagnostic byte range".into());
        }
        let message = diagnostic
            .message
            .replace(&format!("{}/", root.display()), "");
        let metric = metric_from_message(&check, &message)?;
        raw.push((
            check,
            relative,
            start,
            end,
            diagnostic.span.start.line,
            message,
            metric,
        ));
    }
    // Source coordinates determine order. For several occurrences at the same
    // span, preserve the compiler's deterministic diagnostic order.
    raw.sort_by(|a, b| (&a.0, &a.1, a.2, a.3).cmp(&(&b.0, &b.1, b.2, b.3)));
    let mut records: BTreeMap<String, Vec<Finding>> =
        CHECKS.iter().map(|c| (c.to_string(), Vec::new())).collect();
    let mut occurrences = BTreeMap::new();
    for (check, path, start, end, line, message, metric) in raw {
        let occurrence = occurrences
            .entry((check.clone(), path.clone(), start, end))
            .or_insert(0usize);
        let id = format!("{check}:{workspace}:{path}:{start}-{end}:{occurrence}");
        *occurrence += 1;
        records
            .get_mut(&check)
            .ok_or("unknown check")?
            .push(Finding {
                id,
                workspace: workspace.into(),
                location: format!("{path}:{line}"),
                message,
                metric,
                claude: None,
                claude_reason: None,
                sol: None,
                sol_reason: None,
                r#final: None,
            });
    }
    Ok(records)
}

struct Scratch(PathBuf);
impl Scratch {
    fn new(parent: &Path) -> Result<Self> {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = parent.join(format!(
            ".calibrate-work-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sorted_entries(root: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = std::fs::read_dir(root)?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    Ok(entries)
}

fn copy_directory(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir(target)?;
    for path in sorted_entries(source)? {
        let metadata = std::fs::symlink_metadata(&path)?;
        let destination = target.join(path.file_name().ok_or("missing filename")?);
        if metadata.is_symlink() {
            return Err(format!("corpus contains a symlink: {}", path.display()).into());
        }
        if metadata.is_dir() {
            copy_directory(&path, &destination)?;
        } else if metadata.is_file() {
            std::fs::copy(&path, &destination)?;
        } else {
            return Err("corpus contains a non-regular file".into());
        }
    }
    Ok(())
}

fn checked_dump_destination(root: &Path, out: &Path) -> Result<PathBuf> {
    let corpus = root.join("evals/corpus").canonicalize()?;
    let proposed = if out.is_absolute() {
        out.to_path_buf()
    } else {
        std::env::current_dir()?.join(out)
    };
    let mut resolved = PathBuf::new();
    // Resolve existing components as we encounter them. A missing component
    // followed by `..` must not hide a later, existing symlink into the corpus.
    // No directory is created while determining the destination.
    for component in proposed.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                resolved.pop();
            }
            component => {
                resolved.push(component.as_os_str());
                match std::fs::symlink_metadata(&resolved) {
                    Ok(_) => resolved = resolved.canonicalize()?,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }
    if resolved.starts_with(&corpus) {
        return Err(format!(
            "dump destination is inside the corpus: {}",
            resolved.display()
        )
        .into());
    }
    Ok(resolved)
}

fn dump(root: &Path, out: &Path) -> Result<()> {
    let out = checked_dump_destination(root, out)?;
    std::fs::create_dir_all(&out)?;
    let out = out.canonicalize()?;
    let target = out.join(".calibrate-target");
    let build =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .current_dir(root)
            .args(["build", "--locked", "-p", "ridl-cli", "--target-dir"])
            .arg(&target)
            .output()?;
    if !build.status.success() {
        return Err(format!(
            "ridl build failed:\n{}",
            String::from_utf8_lossy(&build.stderr)
        )
        .into());
    }
    let scratch = Scratch::new(&out)?;
    let binary = target
        .join("debug")
        .join(format!("ridl{}", std::env::consts::EXE_SUFFIX));
    let mut all: BTreeMap<String, Vec<Finding>> =
        CHECKS.iter().map(|c| (c.to_string(), Vec::new())).collect();
    let workspaces = sorted_entries(&root.join("evals/corpus"))?;
    if workspaces.is_empty() {
        return Err("no corpus workspaces".into());
    }
    for source in workspaces {
        if !source.is_dir() || std::fs::symlink_metadata(&source)?.is_symlink() {
            return Err("corpus workspace must be a real directory".into());
        }
        let name = source
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("workspace name is not UTF-8")?;
        if name.contains(':') {
            return Err("workspace name contains an ID delimiter".into());
        }
        let copied = scratch.0.join(name);
        copy_directory(&source, &copied)?;
        let manifest = copied.join("ridl.toml");
        let mut text = std::fs::read_to_string(&manifest)?;
        let parsed: toml::Value = toml::from_str(&text)?;
        if parsed.get("lints").is_some() {
            return Err("corpus already has a lints table; refusing to overwrite it".into());
        }
        text.push_str("\n[lints]\n");
        for check in CHECKS {
            text.push_str(&format!("{check} = \"warn\"\n"));
        }
        std::fs::write(manifest, text)?;
        let checked = std::process::Command::new(&binary)
            .current_dir(&copied)
            .args(["check", "--format", "json"])
            .arg(&copied)
            .output()?;
        if !checked.status.success() {
            return Err(format!(
                "check failed for {name}:\n{}\n{}",
                String::from_utf8_lossy(&checked.stderr),
                String::from_utf8_lossy(&checked.stdout)
            )
            .into());
        }
        for (check, findings) in records_from_json(&copied, name, &checked.stdout)? {
            all.get_mut(&check).ok_or("unknown check")?.extend(findings);
        }
    }
    // Validate every workspace before publishing any of the five arrays.
    for (check, findings) in all {
        let path = out.join(format!("{check}.json"));
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&findings)?),
        )?;
        println!("{}: {} findings", path.display(), findings.len());
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Labels {
    finding: Vec<Finding>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryItem {
    id: String,
    workspace: String,
    kind: String,
    reason: String,
    canonical: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckJoin {
    name: String,
    issue: Vec<IssueRow>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recall {
    item: Vec<InventoryItem>,
    check: Vec<CheckJoin>,
}

fn required_reason(reason: &str) -> Result<()> {
    if reason.trim().is_empty() {
        return Err("review reason is empty".into());
    }
    Ok(())
}
fn label(label: Option<&str>) -> Result<()> {
    if !matches!(label, Some("accept" | "dismiss")) {
        return Err("missing or invalid adjudicated label".into());
    }
    Ok(())
}

fn validate_finding(root: &Path, check: &str, finding: &Finding) -> Result<()> {
    for value in [
        finding.claude.as_deref(),
        finding.sol.as_deref(),
        finding.r#final.as_deref(),
    ] {
        label(value)?;
    }
    if finding.claude == finding.sol && finding.r#final != finding.claude {
        return Err("final label must preserve reviewer agreement".into());
    }
    for reason in [&finding.claude_reason, &finding.sol_reason] {
        required_reason(reason.as_deref().ok_or("missing label reason")?)?;
    }
    if finding.message.trim().is_empty() {
        return Err("finding message is empty".into());
    }
    let parts: Vec<_> = finding.id.split(':').collect();
    if parts.len() != 5
        || parts[0] != check
        || parts[1] != finding.workspace
        || finding.workspace.is_empty()
    {
        return Err(format!("invalid finding ID `{}`", finding.id).into());
    }
    let workspace = root.join("evals/corpus").join(&finding.workspace);
    if !workspace.is_dir()
        || relative_path(Path::new(""), Path::new(&finding.workspace))? != finding.workspace
    {
        return Err("unknown or invalid finding workspace".into());
    }
    let path = relative_path(&workspace, Path::new(parts[2]))?;
    let (start, end) = parts[3]
        .split_once('-')
        .ok_or("missing finding byte range")?;
    let (start, end) = (number(start)?, number(end)?);
    number(parts[4])?;
    let source = std::fs::read_to_string(workspace.join(&path))?;
    if start > end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return Err("invalid finding byte range".into());
    }
    let line = source[..start].bytes().filter(|&c| c == b'\n').count() + 1;
    if finding.location != format!("{path}:{line}") {
        return Err("finding location disagrees with ID".into());
    }
    if metric_from_message(check, &finding.message)? != finding.metric {
        return Err("finding metric disagrees with diagnostic message".into());
    }
    match &finding.metric {
        Some(Metric::Struct { count } | Metric::Enum { count }) if *count < 2 => {
            return Err("shape count below search start".into());
        }
        Some(Metric::Cohesion {
            groups,
            min_group_size,
        }) if *groups < 2 || *min_group_size == 0 => {
            return Err("cohesion coordinates below search start".into());
        }
        Some(Metric::FanOut { count }) if *count <= 3 => {
            return Err("fan-out below search start".into());
        }
        _ => {}
    }
    Ok(())
}

fn rubric_items(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut inventory = BTreeMap::new();
    for task in sorted_entries(&root.join("evals/tasks"))? {
        let manifest: toml::Value =
            toml::from_str(&std::fs::read_to_string(task.join("task.toml"))?)?;
        if manifest.get("kind").and_then(toml::Value::as_str) != Some("review") {
            continue;
        }
        let id = manifest
            .get("id")
            .and_then(toml::Value::as_str)
            .ok_or("review task lacks ID")?;
        if task.file_name().and_then(|s| s.to_str()) != Some(id) {
            return Err("review task ID disagrees with directory".into());
        }
        let workspace = manifest
            .get("corpus")
            .and_then(toml::Value::as_str)
            .ok_or("review task lacks corpus")?;
        if !root.join("evals/corpus").join(workspace).is_dir() {
            return Err("review task names unknown workspace".into());
        }
        let text = std::fs::read_to_string(task.join("rubric.md"))?;
        let mut count = 0;
        for line in text.lines() {
            let Some((n, rest)) = line.split_once(". ") else {
                continue;
            };
            if !n.bytes().all(|c| c.is_ascii_digit()) || n.is_empty() {
                continue;
            }
            let n = positive(n)?;
            if !["**must**", "**should**", "**must not**"]
                .iter()
                .any(|prefix| rest.starts_with(prefix))
            {
                return Err("review rubric item lacks strength".into());
            }
            if inventory
                .insert(format!("{id}:{n}"), workspace.to_string())
                .is_some()
            {
                return Err("duplicate rubric item ID".into());
            }
            count += 1;
        }
        if count == 0 {
            return Err("review rubric has no numbered items".into());
        }
    }
    if inventory.is_empty() {
        return Err("no review rubric items".into());
    }
    Ok(inventory)
}

fn validate_recall(
    root: &Path,
    recall: &Recall,
    labels: &BTreeMap<String, Vec<Finding>>,
) -> Result<()> {
    let rubric = rubric_items(root)?;
    let mut items = BTreeMap::new();
    for item in &recall.item {
        required_reason(&item.reason)?;
        if rubric.get(&item.id) != Some(&item.workspace)
            || items.insert(item.id.clone(), item).is_some()
        {
            return Err(format!(
                "unknown, duplicated or mismatched inventory ID `{}`",
                item.id
            )
            .into());
        }
        match item.kind.as_str() {
            "issue" | "excluded" if item.canonical.is_none() => {}
            "alias" if item.canonical.is_some() => {}
            _ => return Err("invalid inventory kind or canonical field".into()),
        }
    }
    if items.len() != rubric.len() {
        return Err("recall inventory must classify every numbered review item".into());
    }
    for item in items.values() {
        if let Some(canonical) = &item.canonical {
            let target = items.get(canonical).ok_or("unknown canonical issue ID")?;
            if target.kind != "issue" || target.workspace != item.workspace || canonical >= &item.id
            {
                return Err(
                    "alias must refer to the lexically first canonical issue in its workspace"
                        .into(),
                );
            }
        }
    }
    let canonical: BTreeSet<_> = items
        .values()
        .filter(|i| i.kind == "issue")
        .map(|i| i.id.as_str())
        .collect();
    let mut checks = BTreeSet::new();
    for check in &recall.check {
        if !CHECKS.contains(&check.name.as_str()) || !checks.insert(check.name.as_str()) {
            return Err("unknown or duplicate recall check".into());
        }
        let findings: BTreeMap<_, _> = labels[&check.name]
            .iter()
            .map(|f| (f.id.as_str(), f))
            .collect();
        let mut seen = BTreeSet::new();
        for issue in &check.issue {
            required_reason(&issue.reason)?;
            if !canonical.contains(issue.id.as_str()) || !seen.insert(issue.id.as_str()) {
                return Err("unknown, alias or duplicate applicability issue ID".into());
            }
            if !issue.applicable && !issue.findings.is_empty() {
                return Err("inapplicable issue must have no matching findings".into());
            }
            let mut matched = BTreeSet::new();
            for id in &issue.findings {
                let finding = findings
                    .get(id.as_str())
                    .ok_or("recall refers to an unknown finding ID for this check")?;
                if !matched.insert(id) || finding.workspace != items[&issue.id].workspace {
                    return Err("duplicate or cross-workspace recall finding".into());
                }
            }
        }
        if seen != canonical {
            return Err("missing applicability rows for canonical issues".into());
        }
    }
    if checks.len() != CHECKS.len() {
        return Err("missing recall check".into());
    }
    Ok(())
}

impl std::fmt::Display for Threshold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Shape(fields, variants) => {
                write!(f, "fields >= {fields}, variants >= {variants}")
            }
            Self::Cohesion(groups, size) => {
                write!(f, "groups >= {groups}, min group size >= {size}")
            }
            Self::FanOut(max) => write!(f, "fan-out > {max}"),
        }
    }
}

fn read_calibration(root: &Path) -> Result<String> {
    let mut labels = BTreeMap::new();
    let mut all_ids = BTreeSet::new();
    for check in CHECKS {
        let path = root.join(format!("evals/calibration/{check}.toml"));
        let parsed: Labels = toml::from_str(&std::fs::read_to_string(&path)?)?;
        let mut occurrences: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
        for finding in &parsed.finding {
            validate_finding(root, check, finding)?;
            if !all_ids.insert(finding.id.clone()) {
                return Err("duplicate finding ID".into());
            }
            let (prefix, index) = finding.id.rsplit_once(':').ok_or("invalid ID")?;
            occurrences
                .entry(prefix.into())
                .or_default()
                .insert(number(index)?);
        }
        for indices in occurrences.values() {
            if indices.iter().copied().ne(0..indices.len()) {
                return Err("finding occurrence indices must be contiguous from zero".into());
            }
        }
        labels.insert(check.to_string(), parsed.finding);
    }
    let recall: Recall = toml::from_str(&std::fs::read_to_string(
        root.join("evals/calibration/recall.toml"),
    )?)?;
    validate_recall(root, &recall, &labels)?;
    let mut summary = String::from(
        "# Design check calibration\n\nPrecision bar: 80%. Floor: 50%. Fewer than ten retained findings permits Info at most. Recall is reported and does not select or gate thresholds. Zero findings have undefined precision.\n\n",
    );
    for check in CHECKS {
        let issues = &recall
            .check
            .iter()
            .find(|c| c.name == check)
            .ok_or("missing recall check")?
            .issue;
        let rows = candidates(check, &labels[check], issues);
        let workspaces: BTreeSet<_> = labels[check].iter().map(|f| f.workspace.as_str()).collect();
        summary.push_str(&format!("## {check}\n\nFinding workspaces: {}.\n\n| Candidate threshold | Findings | Accepted | Precision | Detected applicable issues | Total applicable issues | Recall | Level |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n", if workspaces.is_empty() {"none".into()} else {workspaces.into_iter().collect::<Vec<_>>().join(", ")}));
        for row in &rows {
            let precision = if row.findings == 0 {
                "undefined".into()
            } else {
                format!("{:.2}%", row.accepted as f64 * 100.0 / row.findings as f64)
            };
            summary.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {:?} |\n",
                row.threshold,
                row.findings,
                row.accepted,
                precision,
                row.detected,
                row.applicable,
                row.recall(),
                row.level
            ));
        }
        match choose(&rows) {
            Some(best) => summary.push_str(&format!(
                "\nSelected level: {:?}. Threshold: {}. Retained findings: {}.{}\n\n",
                best.level,
                best.threshold,
                best.findings,
                if best.findings < 10 {
                    " Small sample: fewer than ten findings."
                } else {
                    ""
                }
            )),
            None => summary.push_str("\nSelected level: not a lint. No candidate qualifies.\n\n"),
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn finding(n: usize, metric: Metric, accept: bool) -> Finding {
        Finding {
            id: format!("package-fan-out:fixture:p/a.typl:{}-{}:0", n, n + 1),
            workspace: "fixture".into(),
            location: "p/a.typl:1".into(),
            message: String::new(),
            metric: Some(metric),
            claude: Some("accept".into()),
            claude_reason: Some("reason".into()),
            sol: Some("accept".into()),
            sol_reason: Some("reason".into()),
            r#final: Some(if accept { "accept" } else { "dismiss" }.into()),
        }
    }
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join(".superpowers/sdd/2026-10-04-design-lints-plan")
                .join(format!(
                    "task-12-test-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn write(&self, path: &str, text: &str) {
            let path = self.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn dump_ids_use_utf8_byte_ranges_and_ignore_temporary_roots() {
        let mut results = vec![];
        for _ in 0..2 {
            let fixture = Fixture::new();
            fixture.write("p/a.typl", "// é\npackage p;\n");
            let diagnostic = serde_json::json!({"code":"RIDL-415", "severity":"warning", "lint":"package-fan-out", "message":format!("package `p` depends on 4 workspace packages: a, b, c, d ({}/p/a.typl:2)", fixture.0.display()), "span":{"path":fixture.0.join("p/a.typl"), "start":{"line":2,"column":1}, "end":{"line":2,"column":8}}, "labels":[], "fixes":[]});
            let json = serde_json::to_vec(&vec![diagnostic.clone(), diagnostic]).unwrap();
            let records = records_from_json(&fixture.0, "fixture", &json).unwrap();
            let findings = &records["package-fan-out"];
            assert_eq!(findings[0].id, "package-fan-out:fixture:p/a.typl:6-13:0");
            assert_eq!(findings[1].id, "package-fan-out:fixture:p/a.typl:6-13:1");
            assert_eq!(findings[0].location, "p/a.typl:2");
            assert!(
                !findings[0]
                    .message
                    .contains(&fixture.0.display().to_string())
            );
            results.push(serde_json::to_value(records).unwrap());
        }
        assert_eq!(results[0], results[1]);
    }
    fn calibration_fixture() -> Fixture {
        let fixture = Fixture::new();
        fixture.write("evals/corpus/fixture/p/a.typl", "package p;\n");
        fixture.write(
            "evals/tasks/review-0001/task.toml",
            "id = \"review-0001\"\nkind = \"review\"\ncorpus = \"fixture\"\n",
        );
        fixture.write(
            "evals/tasks/review-0001/rubric.md",
            "1. **must** identify one issue.\n2. **must not** change a name.\n",
        );
        let item = "[[item]]\nid = \"review-0001:1\"\nworkspace = \"fixture\"\nkind = \"issue\"\nreason = \"Design issue\"\n\n[[item]]\nid = \"review-0001:2\"\nworkspace = \"fixture\"\nkind = \"excluded\"\nreason = \"Answer constraint\"\n";
        let mut recall = item.to_owned();
        for check in CHECKS {
            let text = if check == "package-fan-out" {
                "[[finding]]\nid = \"package-fan-out:fixture:p/a.typl:0-7:0\"\nworkspace = \"fixture\"\nlocation = \"p/a.typl:1\"\nmessage = \"package `p` depends on 4 workspace packages: a, b, c, d\"\nclaude = \"accept\"\nclaude_reason = \"reason\"\nsol = \"accept\"\nsol_reason = \"reason\"\nfinal = \"accept\"\n[finding.metric]\nkind = \"fan-out\"\ncount = 4\n"
            } else {
                "finding = []\n"
            };
            fixture.write(&format!("evals/calibration/{check}.toml"), text);
            recall.push_str(&format!("\n[[check]]\nname = \"{check}\"\n[[check.issue]]\nid = \"review-0001:1\"\napplicable = {}\nreason = \"Reviewed applicability\"\nfindings = {}\n", check == "package-fan-out", if check == "package-fan-out" { "[\"package-fan-out:fixture:p/a.typl:0-7:0\"]" } else { "[]" }));
        }
        fixture.write("evals/calibration/recall.toml", &recall);
        fixture
    }
    #[test]
    fn derive_validates_labels_metadata_inventory_and_complete_join() {
        let fixture = calibration_fixture();
        let summary = read_calibration(&fixture.0).unwrap();
        assert!(summary.contains("not applicable"));
        assert!(summary.contains("100.00%"));
        assert!(summary.contains("Info"));
        let label_path = "evals/calibration/package-fan-out.toml";
        let original = std::fs::read_to_string(fixture.0.join(label_path)).unwrap();
        for bad in [
            original.replace("final = \"accept\"", "final = \"pending\""),
            original.replace("final = \"accept\"\n", ""),
            original.replace("final = \"accept\"", "final = \"dismiss\""),
            original.replace("count = 4", "count = 0"),
            original.replace("count = 4", "count = 5"),
            original.replace("0-7:0", "7-0:0"),
            original.replace("0-7:0", "0-7:x"),
            original.replace("p/a.typl:1", "p/a.typl:2"),
        ] {
            fixture.write(label_path, &bad);
            assert!(read_calibration(&fixture.0).is_err());
        }
        fixture.write(label_path, &original);
        let path = "evals/calibration/recall.toml";
        let original = std::fs::read_to_string(fixture.0.join(path)).unwrap();
        for bad in [
            original.replace("review-0001:1", "review-0001:99"),
            original.replace("name = \"inconsistent-unit\"", "name = \"unknown\""),
            original.replace("applicable = false\n", ""),
            original.replace("kind = \"excluded\"", "kind = \"issue\""),
            original.replace("0-7:0", "0-7:99"),
        ] {
            fixture.write(path, &bad);
            assert!(read_calibration(&fixture.0).is_err());
        }
        fixture.write(path, &original);
        std::fs::remove_file(fixture.0.join(label_path)).unwrap();
        assert!(read_calibration(&fixture.0).is_err());
    }
    #[test]
    fn dump_rejects_errors_unsafe_paths_and_invalid_positions() {
        let fixture = Fixture::new();
        fixture.write("p/a.typl", "package p;\n");
        let diagnostic = serde_json::json!({"severity":"warning","lint":"package-fan-out","message":"package `p` depends on 4 workspace packages: a, b, c, d","span":{"path":"p/a.typl","start":{"line":1,"column":1},"end":{"line":1,"column":8}}});
        for (pointer, value) in [
            ("/severity", serde_json::json!("error")),
            ("/span/path", serde_json::json!("../outside.typl")),
            ("/span/start/line", serde_json::json!(0)),
            ("/span/start/column", serde_json::json!(99)),
            ("/span/end/column", serde_json::json!(0)),
            ("/message", serde_json::json!("no count")),
        ] {
            let mut bad = diagnostic.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(
                records_from_json(
                    &fixture.0,
                    "fixture",
                    &serde_json::to_vec(&vec![bad]).unwrap()
                )
                .is_err()
            );
        }
        let records = records_from_json(&fixture.0, "fixture", b"[]").unwrap();
        assert_eq!(records.len(), 5);
        assert!(records.values().all(Vec::is_empty));
    }
    #[test]
    fn cohesion_requires_the_first_opening_bracket() {
        let message = "interface `I` splits into 2 groups of members that share no type: a], [b]";
        assert!(metric_from_message("low-cohesion-interface", message).is_err());
        let fixture = calibration_fixture();
        let mut finding = finding(
            0,
            Metric::Cohesion {
                groups: 2,
                min_group_size: 1,
            },
            true,
        );
        finding.id = "low-cohesion-interface:fixture:p/a.typl:0-7:0".into();
        finding.message = message.into();
        assert!(validate_finding(&fixture.0, "low-cohesion-interface", &finding).is_err());
    }
    #[test]
    fn dump_destination_rejects_corpus_descendants_before_creation() {
        let fixture = calibration_fixture();
        let corpus = fixture.0.join("evals/corpus");
        for out in [
            corpus.clone(),
            corpus.join("fixture/new/deep"),
            corpus.join("fixture/missing/../new"),
        ] {
            assert!(
                checked_dump_destination(&fixture.0, &out)
                    .unwrap_err()
                    .to_string()
                    .contains("inside the corpus")
            );
            assert!(!corpus.join("fixture/new").exists());
            assert!(!corpus.join("fixture/missing").exists());
        }
        let outside = fixture.0.join("new/deep");
        assert_eq!(
            checked_dump_destination(&fixture.0, &outside).unwrap(),
            outside
        );
        assert!(!outside.exists());
    }
    #[cfg(unix)]
    #[test]
    fn dump_destination_resolves_symlinks_and_missing_parent_segments() {
        let fixture = calibration_fixture();
        let corpus = fixture.0.join("evals/corpus");
        std::os::unix::fs::symlink(corpus.join("fixture"), fixture.0.join("alias")).unwrap();
        for out in [
            fixture.0.join("alias/new"),
            fixture.0.join("missing/../alias/new"),
            fixture.0.join("alias/missing/../new"),
        ] {
            assert!(
                checked_dump_destination(&fixture.0, &out)
                    .unwrap_err()
                    .to_string()
                    .contains("inside the corpus")
            );
            assert!(!corpus.join("fixture/new").exists());
        }
    }
    #[test]
    fn unicode_columns_reconstruct_multibyte_starts_and_endpoints() {
        let fixture = Fixture::new();
        fixture.write("p/a.typl", "// é\né🙂xβ\n");
        let json=serde_json::to_vec(&vec![serde_json::json!({"severity":"warning","lint":"inconsistent-unit","message":"synthetic unit finding","span":{"path":"p/a.typl","start":{"line":2,"column":2},"end":{"line":2,"column":5}}}),serde_json::json!({"severity":"warning","lint":"inconsistent-unit","message":"synthetic unit finding","span":{"path":"p/a.typl","start":{"line":2,"column":3},"end":{"line":2,"column":4}}})]).unwrap();
        let records = records_from_json(&fixture.0, "fixture", &json).unwrap();
        assert_eq!(
            records["inconsistent-unit"][0].id,
            "inconsistent-unit:fixture:p/a.typl:8-15:0"
        );
        assert_eq!(
            records["inconsistent-unit"][1].id,
            "inconsistent-unit:fixture:p/a.typl:12-13:0"
        );
    }
    #[test]
    fn precision_floor_and_nine_finding_cap_are_exact() {
        for (count, accepted, level) in [
            (10, 5, Level::Info),
            (10, 4, Level::Dropped),
            (100, 49, Level::Dropped),
            (9, 9, Level::Info),
            (10, 8, Level::Warning),
        ] {
            let findings = (0..count)
                .map(|n| finding(n, Metric::FanOut { count: 4 }, n < accepted))
                .collect::<Vec<_>>();
            let rows = candidates("package-fan-out", &findings, &[]);
            assert_eq!(
                (rows[0].findings, rows[0].accepted, rows[0].level),
                (count, accepted, level)
            );
        }
    }
    #[test]
    fn recall_does_not_change_threshold_or_level_selection() {
        let findings = (0..16)
            .map(|n| {
                finding(
                    n,
                    Metric::FanOut {
                        count: if n < 4 {
                            4
                        } else if n < 6 {
                            5
                        } else {
                            6
                        },
                    },
                    (6..14).contains(&n),
                )
            })
            .collect::<Vec<_>>();
        for matches in [
            vec![vec![findings[6].id.clone()]],
            vec![vec![findings[6].id.clone()], vec![]],
            vec![vec![]],
            vec![],
        ] {
            let issues = matches
                .into_iter()
                .enumerate()
                .map(|(n, findings)| IssueRow {
                    id: format!("review-0001:{}", n + 1),
                    applicable: true,
                    reason: "Reviewed issue".into(),
                    findings,
                })
                .collect::<Vec<_>>();
            let rows = candidates("package-fan-out", &findings, &issues);
            let selected = choose(&rows).unwrap();
            assert_eq!(
                (selected.threshold, selected.level, selected.findings),
                (Threshold::FanOut(5), Level::Warning, 10)
            );
        }
    }
    #[test]
    fn matching_metadata_below_search_starts_is_rejected() {
        let fixture = calibration_fixture();
        for (check, metric, message) in [
            (
                "duplicate-shape",
                Metric::Struct { count: 1 },
                "`b.B` has the same 1 fields as `a.A`",
            ),
            (
                "duplicate-shape",
                Metric::Enum { count: 1 },
                "`b.B` has the same 1 variants as `a.A`",
            ),
            (
                "low-cohesion-interface",
                Metric::Cohesion {
                    groups: 1,
                    min_group_size: 1,
                },
                "interface `I` splits into 1 groups of members that share no type: [a]",
            ),
            (
                "package-fan-out",
                Metric::FanOut { count: 3 },
                "package `p` depends on 3 workspace packages: a, b, c",
            ),
        ] {
            assert_eq!(
                metric_from_message(check, message).unwrap(),
                Some(metric.clone())
            );
            let mut finding = finding(0, metric, true);
            finding.id = format!("{check}:fixture:p/a.typl:0-7:0");
            finding.message = message.into();
            assert!(
                validate_finding(&fixture.0, check, &finding)
                    .unwrap_err()
                    .to_string()
                    .contains("below search start")
            );
        }
    }
    #[test]
    fn equal_shape_samples_prefer_fields_before_variants() {
        let findings = (0..30)
            .map(|n| {
                finding(
                    n,
                    if n < 5 {
                        Metric::Struct { count: 2 }
                    } else if n < 10 {
                        Metric::Enum { count: 2 }
                    } else if n < 20 {
                        Metric::Struct { count: 3 }
                    } else {
                        Metric::Enum { count: 3 }
                    },
                    n >= 10,
                )
            })
            .collect::<Vec<_>>();
        let rows = candidates("duplicate-shape", &findings, &[]);
        for threshold in [Threshold::Shape(2, 3), Threshold::Shape(3, 2)] {
            let row = rows.iter().find(|r| r.threshold == threshold).unwrap();
            assert_eq!(
                (row.findings, row.accepted, row.level),
                (25, 20, Level::Warning)
            );
        }
        assert_eq!(choose(&rows).unwrap().threshold, Threshold::Shape(2, 3));
    }
    fn recall_inputs(fixture: &Fixture) -> (Recall, BTreeMap<String, Vec<Finding>>) {
        let recall = toml::from_str(
            &std::fs::read_to_string(fixture.0.join("evals/calibration/recall.toml")).unwrap(),
        )
        .unwrap();
        let labels = CHECKS
            .into_iter()
            .map(|check| {
                let parsed: Labels = toml::from_str(
                    &std::fs::read_to_string(
                        fixture.0.join(format!("evals/calibration/{check}.toml")),
                    )
                    .unwrap(),
                )
                .unwrap();
                (check.to_owned(), parsed.finding)
            })
            .collect();
        (recall, labels)
    }
    #[test]
    fn inventory_requires_excluded_items_and_complete_applicability() {
        let fixture = calibration_fixture();
        let (mut recall, labels) = recall_inputs(&fixture);
        validate_recall(&fixture.0, &recall, &labels).unwrap();
        recall.item.remove(1);
        assert!(
            validate_recall(&fixture.0, &recall, &labels)
                .unwrap_err()
                .to_string()
                .contains("classify every")
        );
        let (mut recall, labels) = recall_inputs(&fixture);
        recall.check[0].issue.clear();
        assert!(
            validate_recall(&fixture.0, &recall, &labels)
                .unwrap_err()
                .to_string()
                .contains("missing applicability")
        );
    }
    #[test]
    fn aliases_require_lexical_order_and_the_same_workspace() {
        let fixture = calibration_fixture();
        let (mut recall, labels) = recall_inputs(&fixture);
        recall.item[1].kind = "alias".into();
        recall.item[1].canonical = Some("review-0001:1".into());
        validate_recall(&fixture.0, &recall, &labels).unwrap();
        recall.item[0].kind = "alias".into();
        recall.item[0].canonical = Some("review-0001:2".into());
        recall.item[1].kind = "issue".into();
        recall.item[1].canonical = None;
        for check in &mut recall.check {
            check.issue[0].id = "review-0001:2".into();
        }
        assert!(
            validate_recall(&fixture.0, &recall, &labels)
                .unwrap_err()
                .to_string()
                .contains("lexically first")
        );
        let (mut recall, labels) = recall_inputs(&fixture);
        fixture.write("evals/corpus/other/p/a.typl", "package p;\n");
        fixture.write(
            "evals/tasks/review-0002/task.toml",
            "id = \"review-0002\"\nkind = \"review\"\ncorpus = \"other\"\n",
        );
        fixture.write(
            "evals/tasks/review-0002/rubric.md",
            "1. **must** identify an issue.\n",
        );
        recall.item.push(InventoryItem {
            id: "review-0002:1".into(),
            workspace: "other".into(),
            kind: "alias".into(),
            reason: "Same issue claimed in another workspace".into(),
            canonical: Some("review-0001:1".into()),
        });
        assert!(
            validate_recall(&fixture.0, &recall, &labels)
                .unwrap_err()
                .to_string()
                .contains("in its workspace")
        );
    }
    #[test]
    fn numeric_occurrence_indices_must_start_at_zero_and_have_no_gaps() {
        let fixture = calibration_fixture();
        let labels_path = "evals/calibration/package-fan-out.toml";
        let recall_path = "evals/calibration/recall.toml";
        let labels = std::fs::read_to_string(fixture.0.join(labels_path)).unwrap();
        let recall = std::fs::read_to_string(fixture.0.join(recall_path)).unwrap();
        for (labels,recall) in [(labels.replace("0-7:0","0-7:1"),recall.replace("0-7:0","0-7:1")),(format!("{labels}\n{}",labels.replace("0-7:0","0-7:2")),recall.replace("[\"package-fan-out:fixture:p/a.typl:0-7:0\"]","[\"package-fan-out:fixture:p/a.typl:0-7:0\", \"package-fan-out:fixture:p/a.typl:0-7:2\"]"))] {
            fixture.write(labels_path,&labels);fixture.write(recall_path,&recall);
            assert!(read_calibration(&fixture.0).unwrap_err().to_string().contains("contiguous"));
        }
    }
    #[test]
    fn metric_is_parsed_from_each_message_form() {
        assert_eq!(
            metric_from_message("duplicate-shape", "`b.B` has the same 3 fields as `a.A`").unwrap(),
            Some(Metric::Struct { count: 3 })
        );
        assert_eq!(
            metric_from_message("duplicate-shape", "`b.B` has the same 3 variants as `a.A`")
                .unwrap(),
            Some(Metric::Enum { count: 3 })
        );
        for (listed, size) in [("[a], [b, c]", 1), ("[a, b], [c, d]", 2)] {
            assert_eq!(metric_from_message("low-cohesion-interface", &format!("interface `p.I` splits into 2 groups of members that share no type: {listed}")).unwrap(), Some(Metric::Cohesion { groups: 2, min_group_size: size }));
        }
        assert_eq!(
            metric_from_message(
                "package-fan-out",
                "package `p` depends on 4 workspace packages: a, b, c, d"
            )
            .unwrap(),
            Some(Metric::FanOut { count: 4 })
        );
        for (check, msg) in [
            ("duplicate-shape", "same fields"),
            ("duplicate-shape", "`b` has the same x fields as `a`"),
            (
                "low-cohesion-interface",
                "interface `p.I` splits into 3 groups of members that share no type: [a], [b]",
            ),
            (
                "low-cohesion-interface",
                "interface `p.I` splits into 2 groups of members that share no type: [], [b]",
            ),
            (
                "low-cohesion-interface",
                "interface `p.I` splits into 2 groups of members that share no type: [a], b]",
            ),
            (
                "package-fan-out",
                "package `p` depends on 4 workspace packages: a",
            ),
        ] {
            assert!(metric_from_message(check, msg).is_err(), "{msg}");
        }
        assert_eq!(
            metric_from_message("inconsistent-unit", "message").unwrap(),
            None
        );
        assert_eq!(
            metric_from_message("inconsistent-abbreviation", "message").unwrap(),
            None
        );
    }
    #[test]
    fn derive_picks_the_least_strict_threshold_meeting_the_bar() {
        let findings: Vec<_> = (0..16)
            .map(|n| {
                finding(
                    n,
                    Metric::FanOut {
                        count: if n < 4 {
                            4
                        } else if n < 6 {
                            5
                        } else {
                            6
                        },
                    },
                    (6..14).contains(&n),
                )
            })
            .collect();
        let rows = candidates("package-fan-out", &findings, &[]);
        assert_eq!(
            rows.iter()
                .map(|r| (r.threshold, r.findings, r.accepted))
                .collect::<Vec<_>>(),
            vec![
                (Threshold::FanOut(3), 16, 8),
                (Threshold::FanOut(4), 12, 8),
                (Threshold::FanOut(5), 10, 8),
                (Threshold::FanOut(6), 0, 0)
            ]
        );
        let best = choose(&rows).unwrap();
        assert_eq!(
            (best.threshold, best.level, best.findings),
            (Threshold::FanOut(5), Level::Warning, 10)
        );
    }
    #[test]
    fn derive_caps_a_check_with_fewer_than_ten_findings_at_info() {
        let rows = candidates(
            "package-fan-out",
            &[finding(0, Metric::FanOut { count: 4 }, true)],
            &[],
        );
        assert_eq!(choose(&rows).unwrap().level, Level::Info);
    }
    #[test]
    fn derive_drops_a_check_below_the_floor() {
        let rows = candidates(
            "package-fan-out",
            &[finding(0, Metric::FanOut { count: 4 }, false)],
            &[],
        );
        assert!(choose(&rows).is_none());
        assert!(choose(&candidates("package-fan-out", &[], &[])).is_none());
    }
    #[test]
    fn derive_preserves_both_shape_thresholds_and_cohesion_coordinates() {
        let shapes = vec![
            finding(0, Metric::Struct { count: 2 }, false),
            finding(1, Metric::Struct { count: 3 }, true),
            finding(2, Metric::Enum { count: 2 }, true),
        ];
        let rows = candidates("duplicate-shape", &shapes, &[]);
        assert!(
            rows.iter().any(|r| r.threshold == Threshold::Shape(3, 2)
                && r.findings == 2
                && r.accepted == 2)
        );
        assert_eq!(choose(&rows).unwrap().threshold, Threshold::Shape(2, 2));
        let groups = vec![
            finding(
                0,
                Metric::Cohesion {
                    groups: 2,
                    min_group_size: 1,
                },
                false,
            ),
            finding(
                1,
                Metric::Cohesion {
                    groups: 2,
                    min_group_size: 2,
                },
                true,
            ),
            finding(
                2,
                Metric::Cohesion {
                    groups: 3,
                    min_group_size: 1,
                },
                true,
            ),
        ];
        let rows = candidates("low-cohesion-interface", &groups, &[]);
        assert!(rows.iter().any(|r| r.threshold == Threshold::Cohesion(2, 2)
            && r.findings == 1
            && r.accepted == 1));
        assert!(rows.iter().any(|r| r.threshold == Threshold::Cohesion(3, 1)
            && r.findings == 1
            && r.accepted == 1));
        assert_eq!(choose(&rows).unwrap().threshold, Threshold::Cohesion(2, 1));
    }
    #[test]
    fn paired_search_selects_warning_coordinates_and_breaks_ties() {
        let mut shapes = (0..4)
            .map(|n| finding(n, Metric::Struct { count: 2 }, false))
            .collect::<Vec<_>>();
        shapes.extend((4..9).map(|n| finding(n, Metric::Struct { count: 3 }, true)));
        shapes.extend((9..14).map(|n| finding(n, Metric::Enum { count: 2 }, true)));
        let rows = candidates("duplicate-shape", &shapes, &[]);
        assert_eq!(
            (
                choose(&rows).unwrap().threshold,
                choose(&rows).unwrap().level
            ),
            (Threshold::Shape(3, 2), Level::Warning)
        );
        for shape in &mut shapes {
            shape.metric = match shape.metric {
                Some(Metric::Struct { count }) => Some(Metric::Enum { count }),
                Some(Metric::Enum { count }) => Some(Metric::Struct { count }),
                _ => unreachable!(),
            };
        }
        let rows = candidates("duplicate-shape", &shapes, &[]);
        assert_eq!(choose(&rows).unwrap().threshold, Threshold::Shape(2, 3));
        let groups = (0..45)
            .map(|n| {
                finding(
                    n,
                    if n < 25 {
                        Metric::Cohesion {
                            groups: 2,
                            min_group_size: 1,
                        }
                    } else if n < 35 {
                        Metric::Cohesion {
                            groups: 2,
                            min_group_size: 2,
                        }
                    } else {
                        Metric::Cohesion {
                            groups: 3,
                            min_group_size: 1,
                        }
                    },
                    n >= 25,
                )
            })
            .collect::<Vec<_>>();
        let rows = candidates("low-cohesion-interface", &groups, &[]);
        assert_eq!(
            (
                choose(&rows).unwrap().threshold,
                choose(&rows).unwrap().findings,
                choose(&rows).unwrap().level
            ),
            (Threshold::Cohesion(2, 2), 10, Level::Warning)
        );
    }
    #[test]
    fn recall_aliases_share_one_canonical_issue() {
        let fixture = calibration_fixture();
        let path = "evals/calibration/recall.toml";
        let original = std::fs::read_to_string(fixture.0.join(path)).unwrap();
        let alias = original.replace(
            "kind = \"excluded\"",
            "kind = \"alias\"\ncanonical = \"review-0001:1\"",
        );
        fixture.write(path, &alias);
        assert!(read_calibration(&fixture.0).unwrap().contains("100.00%"));
        for bad in [
            alias.replace(
                "canonical = \"review-0001:1\"",
                "canonical = \"review-0001:99\"",
            ),
            alias.replace(
                "canonical = \"review-0001:1\"",
                "canonical = \"review-0001:2\"",
            ),
        ] {
            fixture.write(path, &bad);
            assert!(read_calibration(&fixture.0).is_err());
        }
    }
    #[test]
    fn recall_counts_distinct_applicable_issues() {
        let findings = vec![
            finding(0, Metric::FanOut { count: 4 }, true),
            finding(1, Metric::FanOut { count: 5 }, false),
        ];
        let issues = vec![
            IssueRow {
                id: "review-0001:1".into(),
                applicable: true,
                reason: "reason".into(),
                findings: findings.iter().map(|f| f.id.clone()).collect(),
            },
            IssueRow {
                id: "review-0001:2".into(),
                applicable: true,
                reason: "miss".into(),
                findings: vec![],
            },
            IssueRow {
                id: "review-0001:3".into(),
                applicable: false,
                reason: "outside".into(),
                findings: vec![],
            },
        ];
        let rows = candidates("package-fan-out", &findings, &issues);
        assert_eq!(
            (rows[0].detected, rows[0].applicable, rows[0].recall()),
            (1, 2, "50.00%".into())
        );
        assert_eq!((rows[1].detected, rows[1].applicable), (1, 2));
        assert_eq!(
            (
                rows.last().unwrap().detected,
                rows.last().unwrap().applicable
            ),
            (0, 2)
        );
        assert_eq!(
            candidates("package-fan-out", &findings, &[])[0].recall(),
            "not applicable"
        );
    }
    #[test]
    fn derive_alias_matches_the_subcommand() {
        for suffix in [vec![], vec!["--write"]] {
            let canonical: Vec<_> = std::iter::once("derive")
                .chain(suffix.iter().copied())
                .map(str::to_owned)
                .collect();
            let alias: Vec<_> = std::iter::once("--derive")
                .chain(suffix.iter().copied())
                .map(str::to_owned)
                .collect();
            assert_eq!(parse_args(&canonical).unwrap(), parse_args(&alias).unwrap());
        }
        assert!(parse_args(&["derive".into(), "unknown".into()]).is_err());
        let fixture = calibration_fixture();
        let summary_path = fixture.0.join("evals/calibration/summary.md");
        run_at(&fixture.0, &["derive".into()]).unwrap();
        assert!(!summary_path.exists());
        run_at(&fixture.0, &["derive".into(), "--write".into()]).unwrap();
        let canonical = std::fs::read_to_string(&summary_path).unwrap();
        std::fs::remove_file(&summary_path).unwrap();
        run_at(&fixture.0, &["--derive".into(), "--write".into()]).unwrap();
        assert_eq!(std::fs::read_to_string(summary_path).unwrap(), canonical);
    }
}
