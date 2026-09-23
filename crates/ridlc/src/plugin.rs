//! The process host for the backend contract (ADR-0020 decision 10): an
//! executable `ridlc-gen-<language>`, the request on its standard input, the
//! response on its standard output, both in canonical protobuf JSON
//! (`ridl_ir::codegen::request_to_json`, `response_from_json`).
//!
//! The plugin never touches the filesystem: it reads one document and writes
//! one, and `ridlc` writes the files the response carries, so `--out-dir`
//! and the overwrite behaviour are the same for a plugin as for an in-tree
//! backend (decision 9). Everything that can go wrong on the host's side —
//! the executable not found, not startable, exiting non-zero, exceeding the
//! timeout, or answering with something that is not a response — is a
//! [`PluginError`] whose message names the plugin, and `ridlc` reports it as
//! an error diagnostic (`docs/design/codegen-plugins.md`).
//!
//! **How a plugin is named and found.** `--plugin <LANGUAGE>` runs
//! `ridlc-gen-<LANGUAGE>`, found by walking `PATH` in order and taking the
//! first directory that holds a file of that name (with `.exe` appended on
//! Windows); `--plugin <LANGUAGE>=<PATH>` runs the executable at `<PATH>`
//! and skips the lookup. The precedent is `protoc --plugin=[NAME=]PATH`:
//! one flag, one convention for the name, one escape from it. The language
//! is what `ridlc` reports the plugin under, so an error over
//! `--plugin kotlin=/opt/gen/kt` still says `ridlc-gen-kotlin`, with the
//! path beside it.

use std::io::{Read as _, Write as _};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use ridl_ir::codegen::v1;

/// The timeout `--plugin-timeout` defaults to, in seconds. Sixty seconds is
/// two orders of magnitude above what the largest corpus package needs from
/// the reference plugin, and inside what a JVM launcher needs to start.
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 60;

/// The interval at which the host polls a running plugin for its exit,
/// which bounds how far past the timeout a plugin can run before it is
/// killed.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// The prefix every plugin executable's name carries.
pub const NAME_PREFIX: &str = "ridlc-gen-";

/// One `--plugin` value, parsed: the language, and the executable when the
/// value named one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSpec {
    /// The `<language>` of `ridlc-gen-<language>`.
    pub language: String,
    /// The path after `=`, when the value carried one; `None` means the
    /// executable is looked up on `PATH`.
    pub executable: Option<PathBuf>,
}

impl PluginSpec {
    /// The executable's name under the convention: `ridlc-gen-<language>`.
    pub fn name(&self) -> String {
        format!("{NAME_PREFIX}{}", self.language)
    }
}

impl FromStr for PluginSpec {
    type Err = String;

    /// `<language>` or `<language>=<path>`. The language is what the
    /// convention appends to `ridlc-gen-`, so it is one path component with
    /// nothing in it a shell or a filesystem would read as structure.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (language, executable) = match value.split_once('=') {
            Some((language, path)) => (language, Some(path)),
            None => (value, None),
        };
        if language.is_empty() {
            return Err(
                "the language is empty; expected `<language>` or `<language>=<path>`".into(),
            );
        }
        if language
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_whitespace())
        {
            return Err(format!(
                "`{language}` is not a language name: the value is `<language>` or \
                 `<language>=<path>`, and the language names the executable `ridlc-gen-<language>`"
            ));
        }
        let executable = match executable {
            Some("") => return Err("the path after `=` is empty".into()),
            Some(path) => Some(PathBuf::from(path)),
            None => None,
        };
        Ok(Self {
            language: language.to_string(),
            executable,
        })
    }
}

/// A plugin the host can run: its language and the executable it resolved
/// to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    pub language: String,
    pub executable: PathBuf,
}

impl Plugin {
    /// The name the plugin is reported under: `ridlc-gen-<language>`.
    pub fn name(&self) -> String {
        format!("{NAME_PREFIX}{}", self.language)
    }
}

/// What the host reports when it cannot obtain a response from a plugin.
/// Every message names the plugin.
#[derive(Debug)]
pub enum PluginError {
    /// No directory on `PATH` holds the executable.
    NotFound { name: String },
    /// The executable exists but could not be started.
    Spawn {
        name: String,
        executable: PathBuf,
        source: std::io::Error,
    },
    /// Writing the request or reading the response failed.
    Io {
        name: String,
        executable: PathBuf,
        source: std::io::Error,
    },
    /// The plugin exited with a status other than zero, or was killed by a
    /// signal.
    Exit {
        name: String,
        executable: PathBuf,
        status: std::process::ExitStatus,
    },
    /// The plugin did not complete within the timeout; a still-running
    /// process is killed, while an exited process may have left its response
    /// stream open.
    Timeout {
        name: String,
        executable: PathBuf,
        timeout: Duration,
    },
    /// The plugin exited zero but its standard output is not a
    /// `CodegenResponse` in canonical protobuf JSON.
    Malformed {
        name: String,
        executable: PathBuf,
        reason: String,
    },
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { name } => write!(
                f,
                "plugin `{name}` not found: no directory on PATH holds an executable of that \
                 name; install it, or give its path with `--plugin <language>=<path>`"
            ),
            Self::Spawn {
                name,
                executable,
                source,
            } => write!(
                f,
                "plugin `{name}` ({}) cannot be started: {source}",
                executable.display()
            ),
            Self::Io {
                name,
                executable,
                source,
            } => write!(f, "plugin `{name}` ({}): {source}", executable.display()),
            Self::Exit {
                name,
                executable,
                status,
            } => write!(
                f,
                "plugin `{name}` ({}) failed: {status}",
                executable.display()
            ),
            Self::Timeout {
                name,
                executable,
                timeout,
            } => write!(
                f,
                "plugin `{name}` ({}) did not finish within {} s and was killed; raise \
                 `--plugin-timeout` if it needs longer",
                executable.display(),
                timeout.as_secs_f64()
            ),
            Self::Malformed {
                name,
                executable,
                reason,
            } => write!(
                f,
                "plugin `{name}` ({}) returned a malformed response: {reason}",
                executable.display()
            ),
        }
    }
}

impl std::error::Error for PluginError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source, .. } | Self::Io { source, .. } => Some(source),
            Self::NotFound { .. }
            | Self::Exit { .. }
            | Self::Timeout { .. }
            | Self::Malformed { .. } => None,
        }
    }
}

/// Resolves a `--plugin` value to an executable: the path it named, or the
/// first `ridlc-gen-<language>` on `PATH` (module documentation).
pub fn resolve(spec: &PluginSpec) -> Result<Plugin, PluginError> {
    let name = spec.name();
    let executable = match &spec.executable {
        Some(path) => path.clone(),
        None => find_on_path(&name).ok_or(PluginError::NotFound { name: name.clone() })?,
    };
    Ok(Plugin {
        language: spec.language.clone(),
        executable,
    })
}

/// The first file named `name` in a `PATH` directory, in `PATH` order. On
/// Windows the name is also tried with `.exe`, which is how a Rust binary
/// is named there; no other extension is tried.
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let candidates: Vec<String> = if cfg!(windows) {
        vec![name.to_string(), format!("{name}.exe")]
    } else {
        vec![name.to_string()]
    };
    std::env::split_paths(&path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .flat_map(|dir| {
            candidates
                .iter()
                .map(move |candidate| dir.join(candidate))
                .collect::<Vec<_>>()
        })
        .find(|candidate| candidate.is_file())
}

/// Runs `plugin` over one request: the request to its standard input, the
/// response from its standard output, its standard error inherited so a
/// plugin's own messages reach the terminal as `ridlc`'s do. Bounds both the
/// plugin's exit and response collection by `timeout`, killing a still-running
/// plugin when the deadline passes.
///
/// A response that carries an error-severity diagnostic is not a failure of
/// the host: it is returned, and the caller reports the diagnostic and
/// writes none of its files, exactly as for an in-tree backend.
pub fn run(
    plugin: &Plugin,
    request: &v1::CodegenRequest,
    timeout: Duration,
) -> Result<v1::CodegenResponse, PluginError> {
    let name = plugin.name();
    let executable = plugin.executable.clone();
    let io_error = |source: std::io::Error| PluginError::Io {
        name: name.clone(),
        executable: executable.clone(),
        source,
    };

    let request_json = ridl_ir::codegen::request_to_json(request).map_err(|err| {
        io_error(std::io::Error::other(format!(
            "the request cannot be rendered: {err}"
        )))
    })?;

    let mut child = Command::new(&plugin.executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|source| PluginError::Spawn {
            name: name.clone(),
            executable: executable.clone(),
            source,
        })?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let mut stdout = child.stdout.take().expect("stdout is piped");

    // The request is written and the response read on their own threads,
    // so a plugin that writes before it has read everything, or reads
    // after it has started writing, cannot block the host on a full pipe
    // in either direction, and so the loop below owns the clock. The two
    // threads are not scoped: after a kill they are left to end on their
    // own rather than joined, because a plugin that started a child of its
    // own — a launcher script starting a JVM — leaves that child holding
    // both pipes, and a join would wait for it, not for the plugin.
    std::thread::spawn(move || {
        // A plugin that exits, or stops reading, before the request is
        // fully written closes its end; the write then fails with a broken
        // pipe, and the exit status is the fact worth reporting, so the
        // write's own error is dropped here.
        let _ = stdin.write_all(request_json.as_bytes());
        drop(stdin);
    });
    let (output_sender, output_receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut output = Vec::new();
        let output = stdout.read_to_end(&mut output).map(|_| output);
        let _ = output_sender.send(output);
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(PluginError::Timeout {
                        name,
                        executable,
                        timeout,
                    });
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(source) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io_error(source));
            }
        }
    };

    // A child of the plugin may outlive it while holding the pipe open. The
    // response must still be collected, but never beyond the same deadline
    // that bounds the plugin process itself.
    let output =
        match output_receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(output) => output,
            Err(RecvTimeoutError::Timeout) => {
                return Err(PluginError::Timeout {
                    name,
                    executable,
                    timeout,
                });
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(io_error(std::io::Error::other(
                    "the plugin response reader stopped unexpectedly",
                )));
            }
        };
    let output = output.map_err(io_error)?;
    if !status.success() {
        return Err(PluginError::Exit {
            name,
            executable,
            status,
        });
    }
    let malformed = |reason: String| PluginError::Malformed {
        name: name.clone(),
        executable: executable.clone(),
        reason,
    };
    let text = String::from_utf8(output)
        .map_err(|err| malformed(format!("standard output is not UTF-8: {err}")))?;
    ridl_ir::codegen::response_from_json(&text)
        .map_err(|err| malformed(format!("not a `ridl.codegen.v1.CodegenResponse`: {err}")))
}

/// The directory `find_on_path` walks, for a test that needs to know which
/// one it would choose.
#[cfg(test)]
pub(crate) fn first_on_path(name: &str) -> Option<PathBuf> {
    find_on_path(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spec_is_a_language_or_a_language_and_a_path() {
        assert_eq!(
            "kotlin".parse::<PluginSpec>().unwrap(),
            PluginSpec {
                language: "kotlin".into(),
                executable: None
            }
        );
        assert_eq!(
            "kotlin=/opt/gen/ridlc-gen-kt"
                .parse::<PluginSpec>()
                .unwrap(),
            PluginSpec {
                language: "kotlin".into(),
                executable: Some(PathBuf::from("/opt/gen/ridlc-gen-kt"))
            }
        );
        // A path with `=` in it: the split is at the first `=`.
        assert_eq!(
            "k=/a=b".parse::<PluginSpec>().unwrap().executable,
            Some(PathBuf::from("/a=b"))
        );
        assert_eq!(
            "kotlin".parse::<PluginSpec>().unwrap().name(),
            "ridlc-gen-kotlin"
        );
        assert!("".parse::<PluginSpec>().is_err());
        assert!("=/x".parse::<PluginSpec>().is_err());
        assert!("kotlin=".parse::<PluginSpec>().is_err());
        assert!("a/b".parse::<PluginSpec>().is_err());
        assert!("a b".parse::<PluginSpec>().is_err());
    }

    #[test]
    fn a_language_with_no_executable_on_path_is_not_found_by_name() {
        let spec: PluginSpec = "no-such-language-ridl-p3".parse().unwrap();
        let err = resolve(&spec).unwrap_err();
        assert!(matches!(err, PluginError::NotFound { .. }));
        let message = err.to_string();
        assert!(
            message.contains("`ridlc-gen-no-such-language-ridl-p3`"),
            "{message}"
        );
        assert!(message.contains("--plugin <language>=<path>"), "{message}");
    }

    #[test]
    fn a_spec_with_a_path_resolves_to_that_path_without_a_lookup() {
        let spec: PluginSpec = "kotlin=/nowhere/ridlc-gen-kt".parse().unwrap();
        let plugin = resolve(&spec).unwrap();
        assert_eq!(plugin.executable, PathBuf::from("/nowhere/ridlc-gen-kt"));
        assert_eq!(plugin.name(), "ridlc-gen-kotlin");
    }

    #[test]
    fn a_missing_executable_cannot_be_started_and_the_error_names_the_plugin() {
        let plugin = Plugin {
            language: "kotlin".into(),
            executable: PathBuf::from("/nowhere/ridlc-gen-kt"),
        };
        let err = run(&plugin, &request(), Duration::from_secs(5)).unwrap_err();
        assert!(matches!(err, PluginError::Spawn { .. }), "{err}");
        let message = err.to_string();
        assert!(message.contains("`ridlc-gen-kotlin`"), "{message}");
        assert!(message.contains("/nowhere/ridlc-gen-kt"), "{message}");
    }

    fn request() -> v1::CodegenRequest {
        v1::CodegenRequest {
            schema: ridl_ir::codegen::SCHEMA.to_string(),
            toolchain: "0.0.0".to_string(),
            model: Some(v1::Model::default()),
            options: Vec::new(),
            artifact_base: "p".to_string(),
        }
    }

    /// The failure modes that need a real child process, each driven by a
    /// small shell script. Unix only: the scripts are `sh`, and CI runs on
    /// Ubuntu; the reference plugin's parity test in `ridlc-gen-model`
    /// covers the success path on every platform.
    #[cfg(unix)]
    mod with_a_child {
        use std::os::unix::fs::PermissionsExt as _;
        use std::sync::{Mutex, MutexGuard};

        use super::*;

        /// One test at a time: a script written by one test while another
        /// test forks its child is, for the instant between that fork and
        /// its exec, open for writing in two processes, and the exec of the
        /// script then fails with `ETXTBSY`. Holding this for the whole of
        /// each test — the write and the run — keeps the two apart.
        static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

        fn serialized() -> MutexGuard<'static, ()> {
            ONE_AT_A_TIME
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
        }

        fn script(dir: &std::path::Path, name: &str, body: &str) -> Plugin {
            let path = dir.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            Plugin {
                language: "test".into(),
                executable: path,
            }
        }

        #[test]
        fn a_non_zero_exit_is_an_error_naming_the_plugin() {
            let _guard = serialized();
            let dir = tempfile::tempdir().unwrap();
            let plugin = script(dir.path(), "exits-3", "cat >/dev/null; exit 3");
            let err = run(&plugin, &request(), Duration::from_secs(10)).unwrap_err();
            assert!(matches!(err, PluginError::Exit { .. }), "{err}");
            let message = err.to_string();
            assert!(message.contains("`ridlc-gen-test`"), "{message}");
            assert!(message.contains("exit status: 3"), "{message}");
        }

        #[test]
        fn output_that_is_not_a_response_is_malformed() {
            let _guard = serialized();
            let dir = tempfile::tempdir().unwrap();
            let plugin = script(dir.path(), "garbage", "cat >/dev/null; echo 'not json'");
            let err = run(&plugin, &request(), Duration::from_secs(10)).unwrap_err();
            assert!(matches!(err, PluginError::Malformed { .. }), "{err}");
            let message = err.to_string();
            assert!(message.contains("`ridlc-gen-test`"), "{message}");
            assert!(message.contains("CodegenResponse"), "{message}");
        }

        #[test]
        fn a_response_with_an_unknown_key_is_malformed() {
            // The generated reader rejects unknown fields, the strictness
            // ADR-0014 decision 11's conformance test relies on; a plugin
            // that writes a field this schema does not have is caught here
            // rather than silently ignored.
            let _guard = serialized();
            let dir = tempfile::tempdir().unwrap();
            let plugin = script(
                dir.path(),
                "unknown-key",
                r#"cat >/dev/null; echo '{"files": [], "diagnostics": [], "extra": 1}'"#,
            );
            let err = run(&plugin, &request(), Duration::from_secs(10)).unwrap_err();
            assert!(matches!(err, PluginError::Malformed { .. }), "{err}");
        }

        #[test]
        fn a_plugin_that_does_not_finish_is_killed_at_the_timeout() {
            let _guard = serialized();
            let dir = tempfile::tempdir().unwrap();
            let plugin = script(dir.path(), "hangs", "cat >/dev/null; sleep 30");
            let started = Instant::now();
            let err = run(&plugin, &request(), Duration::from_millis(300)).unwrap_err();
            assert!(matches!(err, PluginError::Timeout { .. }), "{err}");
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "the plugin was killed, not waited for: {:?}",
                started.elapsed()
            );
            let message = err.to_string();
            assert!(message.contains("`ridlc-gen-test`"), "{message}");
            assert!(message.contains("--plugin-timeout"), "{message}");
        }

        #[test]
        fn a_plugin_that_exits_with_stdout_still_open_is_bounded() {
            let _guard = serialized();
            let dir = tempfile::tempdir().unwrap();
            let plugin = script(
                dir.path(),
                "leaves-stdout-open",
                "cat >/dev/null; sleep 30 & exit 0",
            );
            let started = Instant::now();
            let err = run(&plugin, &request(), Duration::from_millis(300)).unwrap_err();
            assert!(matches!(err, PluginError::Timeout { .. }), "{err}");
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "the host waited for a descendant holding stdout: {:?}",
                started.elapsed()
            );
        }

        #[test]
        fn a_plugin_that_never_reads_its_input_still_reports_its_exit() {
            // The request is larger than a pipe buffer only for a big
            // package, but the write happens on its own thread either way:
            // a plugin that exits without reading leaves the writer with a
            // broken pipe, which is dropped, and the exit status is what is
            // reported.
            let _guard = serialized();
            let dir = tempfile::tempdir().unwrap();
            let plugin = script(dir.path(), "exits-early", "exit 7");
            let err = run(&plugin, &request(), Duration::from_secs(10)).unwrap_err();
            assert!(matches!(err, PluginError::Exit { .. }), "{err}");
        }

        #[test]
        fn the_response_is_read_from_standard_output_and_is_the_parsed_message() {
            let _guard = serialized();
            let dir = tempfile::tempdir().unwrap();
            let plugin = script(
                dir.path(),
                "answers",
                r#"cat >/dev/null; echo '{"files": [{"path": "a.txt", "text": "hello"}], "diagnostics": [{"severity": "DIAGNOSTIC_SEVERITY_WARNING", "message": "w"}]}'"#,
            );
            let response = run(&plugin, &request(), Duration::from_secs(10)).unwrap();
            assert_eq!(response.files.len(), 1);
            assert_eq!(response.files[0].path, "a.txt");
            assert_eq!(
                response.files[0].content,
                Some(v1::generated_file::Content::Text("hello".into()))
            );
            assert_eq!(response.diagnostics[0].message, "w");
            assert!(!ridl_ir::codegen::has_error(&response));
        }

        #[test]
        fn the_lookup_takes_the_first_path_directory_that_holds_the_name() {
            // `PATH` is process-wide, so this test does not set it; it
            // checks the walk against the process's own `PATH`, which
            // holds `sh` somewhere, and that a name no directory holds is
            // absent.
            let sh = first_on_path("sh").expect("`sh` is on PATH");
            assert!(sh.is_file());
            assert!(first_on_path("ridlc-gen-no-such-language-ridl-p3").is_none());
        }
    }
}
