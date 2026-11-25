//! Hands a command that the running release refuses to a compatible installed
//! release.
//!
//! A workspace pin that the running release does not satisfy (MANI-026) is
//! refused today. When an installed release satisfies every pin of the run, that
//! release runs with the same arguments, directory, environment and standard
//! streams, and nothing is printed by this process. On Unix this process is
//! replaced by the release. On Windows it starts the release inside a Job
//! Object, waits, and exits with the release's exit code.

use ridl_cli::toolchain::{
    BUILD_TARGET, RELEASE_BASE_URL, RELEASE_PUBLIC_KEY, install, process_toolchain_root,
    select_installed,
};
use ridl_core::{
    DiagCode, Diagnostic, SourceMap, is_release_refusal,
    release::{Release, check_release_pin, parse_release_pin},
};
use std::{
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

/// The variable that marks a process started by a hand-off. Its value is the
/// release that handed off. A process that has it never hands off again.
pub(crate) const GUARD: &str = "RIDL_TOOLCHAIN_HANDOFF";

/// The variable that allows a workspace command to install the pinned release
/// when none is installed. Only the value `1` allows it.
pub(crate) const AUTO_INSTALL: &str = "RIDL_TOOLCHAIN_AUTO_INSTALL";

/// Whether a hand-off may install a release that is not installed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Install {
    Never,
    WhenOptedIn,
}

/// The release origin and its public key. A debug build replaces each with the
/// nonempty value of `RIDL_TEST_RELEASE_ORIGIN` or `RIDL_TEST_RELEASE_PUBLIC_KEY`,
/// which integration tests use; a release build has no override.
pub(crate) fn release_origin() -> (String, String) {
    origin_with(cfg!(debug_assertions), |name| std::env::var(name).ok())
}

/// The origin and key, where `read` reads a variable and `allow_test_seams`
/// says whether the two test variables apply. Without the seams the constants
/// are returned whatever `read` holds.
fn origin_with(allow_test_seams: bool, read: impl Fn(&str) -> Option<String>) -> (String, String) {
    let mut origin = (RELEASE_BASE_URL.to_owned(), RELEASE_PUBLIC_KEY.to_owned());
    if allow_test_seams {
        let read = |name: &str| read(name).filter(|v| !v.is_empty());
        if let Some(value) = read("RIDL_TEST_RELEASE_ORIGIN") {
            origin.0 = value;
        }
        if let Some(value) = read("RIDL_TEST_RELEASE_PUBLIC_KEY") {
            origin.1 = value;
        }
    }
    origin
}

/// Hands the command off when the refusals in `diagnostics` allow it. `None`
/// means the caller prints its refusal as before.
pub(crate) fn refusal(diagnostics: &[Diagnostic], sources: &SourceMap) -> Option<ExitCode> {
    hand_off(diagnostics, sources, Install::WhenOptedIn)
}

/// Hands `ridl mcp` off before the server starts, so that the selected release
/// reads the client's first message. It never installs.
pub(crate) fn mcp() -> Option<ExitCode> {
    if guarded(std::env::var_os(GUARD).as_deref()) {
        return None;
    }
    let root = ridl_core::read_root_release(Path::new("."), None).ok()?;
    let (manifest, text) = (root.manifest?, root.text?);
    let mut sources = SourceMap::new();
    let file = sources.file_id(&manifest.to_string_lossy(), &text);
    let diagnostic = check_release_pin(file, &text, ridlc::running_release())?;
    if diagnostic.code != DiagCode::MANI_026 {
        return None;
    }
    hand_off(&[diagnostic], &sources, Install::Never)
}

fn hand_off(diagnostics: &[Diagnostic], sources: &SourceMap, install: Install) -> Option<ExitCode> {
    if guarded(std::env::var_os(GUARD).as_deref()) {
        return None;
    }
    let pins = pins(diagnostics, sources).filter(|pins| !pins.is_empty())?;
    let selected = process_toolchain_root()
        .and_then(|root| select_installed(&root, &pins, BUILD_TARGET))
        .ok()
        .flatten();
    if let Some((release, binary)) = selected {
        return Some(run(release, &binary));
    }
    if install == Install::WhenOptedIn
        && auto_install_enabled(std::env::var_os(AUTO_INSTALL).as_deref())
        && let [pin] = pins[..]
    {
        let binary = install_pin(pin)?;
        return Some(run(pin, &binary));
    }
    None
}

/// Whether the opt-in variable holds exactly `1`.
fn auto_install_enabled(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new("1"))
}

/// Installs `pin`, reporting progress and failure on standard error. `None`
/// means the caller prints its refusal.
fn install_pin(pin: Release) -> Option<PathBuf> {
    errprintln!("installing ridl {pin}\u{2026}");
    let (base_url, public_key) = release_origin();
    match process_toolchain_root()
        .and_then(|root| install(&root, pin, BUILD_TARGET, &base_url, &public_key))
    {
        Ok(binary) => {
            errprintln!("installed ridl {pin} at {}", binary.display());
            Some(binary)
        }
        Err(error) => {
            errprintln!("error: {error}");
            None
        }
    }
}

/// The pins of every MANI-026 in the run, read again from the manifest text.
/// `None` when any release refusal is not MANI-026 or a pin cannot be read.
fn pins(diagnostics: &[Diagnostic], sources: &SourceMap) -> Option<Vec<Release>> {
    let mut pins = Vec::new();
    for diagnostic in diagnostics.iter().filter(|d| is_release_refusal(d)) {
        if diagnostic.code != DiagCode::MANI_026 {
            return None;
        }
        let file = diagnostic.primary.file;
        match parse_release_pin(file, sources.text(file)?) {
            Ok(Some(pin)) => pins.push(pin.release),
            _ => return None,
        }
    }
    pins.sort();
    pins.dedup();
    Some(pins)
}

/// Whether the guard variable is present, whatever its value.
fn guarded(guard: Option<&OsStr>) -> bool {
    guard.is_some()
}

/// The command that runs `binary` with `args`, marked with the running release.
fn child_command(
    binary: &Path,
    args: impl IntoIterator<Item = OsString>,
    running: Release,
) -> Command {
    let mut command = Command::new(binary);
    command.args(args).env(GUARD, running.to_string());
    command
}

fn exec_failure(release: Release, binary: &Path, error: &io::Error) -> String {
    format!(
        "error: could not run ridl {release} at {}: {error}",
        binary.display()
    )
}

#[cfg(unix)]
fn run(release: Release, binary: &Path) -> ExitCode {
    use std::os::unix::process::CommandExt;
    let error = child_command(
        binary,
        std::env::args_os().skip(1),
        ridlc::running_release(),
    )
    .exec();
    errprintln!("{}", exec_failure(release, binary, &error));
    ExitCode::from(2)
}

/// The error that names the Job Object step that failed.
#[cfg(any(windows, test))]
fn job_error(step: &str, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("cannot {step}: {error}"))
}

/// Puts this process in a Job Object that kills its processes when the job
/// closes. The handle is never closed, so the job lives as long as this
/// process, and a child started afterwards inherits the job. Doing this before
/// the child starts leaves no moment at which a killed parent leaves a child
/// outside the job.
#[cfg(windows)]
fn join_kill_on_close_job() -> io::Result<()> {
    use windows_sys::Win32::System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        },
        Threading::GetCurrentProcess,
    };
    // SAFETY: the limit structure is zeroed and sized for the class named, and
    // the handles are valid for the calls.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(job_error("create a job object", io::Error::last_os_error()));
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            std::ptr::addr_of!(limits).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0
        {
            return Err(job_error(
                "configure the job object",
                io::Error::last_os_error(),
            ));
        }
        if AssignProcessToJobObject(job, GetCurrentProcess()) == 0 {
            return Err(job_error("join the job object", io::Error::last_os_error()));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn run(release: Release, binary: &Path) -> ExitCode {
    use windows_sys::Win32::System::Console::{
        CTRL_BREAK_EVENT, CTRL_C_EVENT, SetConsoleCtrlHandler,
    };
    /// Consumes Ctrl+C and Ctrl+Break in this process. The child receives the
    /// same event through the console and handles it as it would on its own.
    unsafe extern "system" fn consume_interrupt(event: u32) -> i32 {
        i32::from(event == CTRL_C_EVENT || event == CTRL_BREAK_EVENT)
    }
    // Without the job the child could outlive a killed parent, so no job means
    // no child.
    if let Err(error) = join_kill_on_close_job() {
        errprintln!("{}", exec_failure(release, binary, &error));
        return ExitCode::from(2);
    }
    // A registered handler is not inherited, unlike the "ignore" attribute that
    // a null handler sets, so the child keeps its default Ctrl+C behaviour.
    // SAFETY: the routine is a valid `extern "system"` function that touches no
    // state, and it stays valid for the life of the process.
    unsafe {
        SetConsoleCtrlHandler(Some(consume_interrupt), 1);
    }
    let mut child = match child_command(
        binary,
        std::env::args_os().skip(1),
        ridlc::running_release(),
    )
    .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            errprintln!("{}", exec_failure(release, binary, &error));
            return ExitCode::from(2);
        }
    };
    match child.wait() {
        Ok(status) => std::process::exit(status.code().unwrap_or(2)),
        Err(error) => {
            errprintln!("{}", exec_failure(release, binary, &error));
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_core::release::{check_release_pin, parse_release};

    fn release(text: &str) -> Release {
        parse_release(text).unwrap()
    }

    /// One manifest with `pin`, refused by a running release 0.22.5.
    fn refused(sources: &mut SourceMap, name: &str, pin: &str) -> Diagnostic {
        let text = format!("[package]\nname = 'p'\nridl-version = '{pin}'\n");
        let file = sources.file_id(name, &text);
        check_release_pin(file, &text, release("0.22.5")).expect("a refusal")
    }

    #[test]
    fn a_release_build_ignores_the_test_origin_and_key() {
        let set = |name: &str| Some(format!("{name}-value"));
        assert_eq!(
            origin_with(false, set),
            (RELEASE_BASE_URL.to_owned(), RELEASE_PUBLIC_KEY.to_owned())
        );
        assert_eq!(
            origin_with(true, set),
            (
                "RIDL_TEST_RELEASE_ORIGIN-value".to_owned(),
                "RIDL_TEST_RELEASE_PUBLIC_KEY-value".to_owned()
            )
        );
        // An empty value is no override.
        assert_eq!(
            origin_with(true, |_| Some(String::new())),
            (RELEASE_BASE_URL.to_owned(), RELEASE_PUBLIC_KEY.to_owned())
        );
    }

    #[test]
    fn pins_reparse_each_mani_026_and_refuse_other_codes() {
        let mut sources = SourceMap::new();
        let first = refused(&mut sources, "a/ridl.toml", "0.23.4");
        let second = refused(&mut sources, "b/ridl.toml", "0.23.1");
        let again = refused(&mut sources, "c/ridl.toml", "0.23.4");
        assert_eq!(first.code, DiagCode::MANI_026);
        let all = [first.clone(), second.clone(), again];
        assert_eq!(
            pins(&all, &sources),
            Some(vec![release("0.23.1"), release("0.23.4")])
        );

        let other = refused(&mut sources, "d/ridl.toml", "42");
        assert_eq!(other.code, DiagCode::MANI_024);
        let mixed = [first.clone(), second, other];
        assert_eq!(pins(&mixed, &sources), None);

        // The same diagnostic against a map that does not hold its file.
        assert_eq!(pins(&[first], &SourceMap::new()), None);
    }

    #[test]
    fn job_error_names_the_step_and_keeps_the_cause() {
        let error = job_error(
            "join the job object",
            io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        );
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(error.to_string(), "cannot join the job object: denied");
        let message = exec_failure(release("0.23.4"), Path::new("/x/ridl"), &error);
        assert_eq!(
            message,
            "error: could not run ridl 0.23.4 at /x/ridl: cannot join the job object: denied"
        );
    }

    #[test]
    fn guard_is_present_for_any_value() {
        assert!(guarded(Some(OsStr::new(""))));
        assert!(guarded(Some(OsStr::new("x"))));
        assert!(!guarded(None));
    }

    #[test]
    fn child_command_passes_arguments_and_sets_only_the_guard() {
        let args = ["check", "src dir", "--format", "json"].map(OsString::from);
        let command = child_command(Path::new("/x/ridl"), args.clone(), release("0.22.5"));
        assert_eq!(command.get_program(), "/x/ridl");
        assert_eq!(command.get_args().collect::<Vec<_>>(), args);
        assert_eq!(
            command.get_envs().collect::<Vec<_>>(),
            [(OsStr::new(GUARD), Some(OsStr::new("0.22.5")))]
        );
        assert_eq!(command.get_current_dir(), None);
    }
}
