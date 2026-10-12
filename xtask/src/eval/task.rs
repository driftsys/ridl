//! Eval tasks under `evals/tasks/`: their kind, starting workspace and
//! mechanical expectations.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Design,
    Evolve,
    Review,
}

#[derive(Debug)]
pub struct Task {
    pub id: String,
    pub kind: Kind,
    /// `evals/tasks/<id>`.
    pub dir: PathBuf,
    /// The workspace the agent starts from: `evals/corpus/<corpus>`, else
    /// the task's `start/`, else none (an empty workspace).
    pub start: Option<PathBuf>,
    pub expect_lints: Vec<String>,
    pub expect_diff: Option<String>,
}

/// Every task under `evals/tasks/`, in directory order, or only those whose
/// id `filter` names. An id in `filter` that names no task is an error.
pub fn load_tasks(root: &Path, filter: &[String]) -> Result<Vec<Task>, String> {
    let tasks_dir = root.join("evals/tasks");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&tasks_dir)
        .map_err(|e| format!("read {}: {e}", tasks_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("task.toml").is_file())
        .collect();
    dirs.sort();
    let mut tasks = Vec::new();
    for dir in dirs {
        let task = load_task(root, &dir)?;
        if filter.is_empty() || filter.contains(&task.id) {
            tasks.push(task);
        }
    }
    for wanted in filter {
        if !tasks.iter().any(|t| &t.id == wanted) {
            return Err(format!("unknown task: {wanted}"));
        }
    }
    Ok(tasks)
}

fn load_task(root: &Path, dir: &Path) -> Result<Task, String> {
    let text = std::fs::read_to_string(dir.join("task.toml")).map_err(|e| e.to_string())?;
    let value: toml::Value =
        toml::from_str(&text).map_err(|e| format!("{}: {e}", dir.display()))?;
    let field = |name: &str| value.get(name).and_then(toml::Value::as_str);
    let id = field("id").ok_or("task lacks id")?.to_owned();
    let kind = match field("kind") {
        Some("design") => Kind::Design,
        Some("evolve") => Kind::Evolve,
        Some("review") => Kind::Review,
        other => return Err(format!("{id}: unknown kind {other:?}")),
    };
    let start = match field("corpus") {
        Some(corpus) => Some(root.join("evals/corpus").join(corpus)),
        None if dir.join("start").is_dir() => Some(dir.join("start")),
        None => None,
    };
    let expect = value
        .get("expect")
        .ok_or_else(|| format!("{id}: no [expect]"))?;
    let expect_lints = expect
        .get("lints")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{id}: no expect.lints"))?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let expect_diff = expect
        .get("diff")
        .and_then(toml::Value::as_str)
        .map(str::to_owned);
    Ok(Task {
        id,
        kind,
        dir: dir.to_owned(),
        start,
        expect_lints,
        expect_diff,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "xtask-eval-task-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let write = |rel: &str, text: &str| {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write("evals/corpus/c/ridl.toml", "[workspace]\nmembers = []\n");
        write(
            "evals/tasks/review-0001/task.toml",
            "id = \"review-0001\"\nkind = \"review\"\ncorpus = \"c\"\ntitle = \"t\"\n[expect]\ncompiles = true\nlints = [\"duplicate-shape\"]\n",
        );
        write(
            "evals/tasks/evolve-0009/task.toml",
            "id = \"evolve-0009\"\nkind = \"evolve\"\ntitle = \"t\"\n[expect]\ncompiles = true\nlints = []\ndiff = \"compatible\"\n",
        );
        write(
            "evals/tasks/evolve-0009/start/ridl.toml",
            "[package]\nname = \"p\"\nversion = \"1.0.0\"\n",
        );
        write(
            "evals/tasks/design-0009/task.toml",
            "id = \"design-0009\"\nkind = \"design\"\ntitle = \"t\"\n[expect]\ncompiles = true\nlints = []\n",
        );
        dir
    }

    #[test]
    fn a_task_starts_from_its_corpus_its_start_directory_or_nothing() {
        let root = root();
        let tasks = load_tasks(&root, &[]).unwrap();
        let start = |id: &str| tasks.iter().find(|t| t.id == id).unwrap().start.clone();
        assert_eq!(start("review-0001"), Some(root.join("evals/corpus/c")));
        assert_eq!(
            start("evolve-0009"),
            Some(root.join("evals/tasks/evolve-0009/start"))
        );
        assert_eq!(start("design-0009"), None);
    }

    #[test]
    fn expectations_are_read_from_the_manifest() {
        let root = root();
        let tasks = load_tasks(&root, &[]).unwrap();
        let review = tasks.iter().find(|t| t.id == "review-0001").unwrap();
        assert_eq!(review.kind, Kind::Review);
        assert_eq!(review.expect_lints, ["duplicate-shape"]);
        let evolve = tasks.iter().find(|t| t.id == "evolve-0009").unwrap();
        assert_eq!(evolve.expect_diff.as_deref(), Some("compatible"));
    }

    #[test]
    fn the_filter_selects_tasks_by_id_and_rejects_an_unknown_one() {
        let root = root();
        let ids: Vec<_> = load_tasks(&root, &["design-0009".to_owned()])
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(ids, ["design-0009"]);
        assert_eq!(
            load_tasks(&root, &["nope-0001".to_owned()]).unwrap_err(),
            "unknown task: nope-0001"
        );
    }
}
