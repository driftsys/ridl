//! Hands a command that the running release refuses to a compatible installed
//! release.
//!
//! A workspace pin that the running release does not satisfy (MANI-026) is
//! refused today. When an installed release satisfies every pin of the run, that
//! release runs with the same arguments, directory, environment and standard
//! streams, and nothing is printed by this process. On Unix this process is
//! replaced by the release. On Windows it starts the release inside a Job
//! Object, waits, and exits with the release's exit code.

use ridl_cli::toolchain::{BUILD_TARGET, process_toolchain_root, select_installed};
use ridl_core::{
    DiagCode, Diagnostic, SourceMap, is_release_refusal,
    release::{Release, parse_release_pin},
};
use std::{
    ffi::{OsStr, OsString},
    io,
    path::Path,
    process::{Command, ExitCode},
};

/// The variable that marks a process started by a hand-off. Its value is the
/// release that handed off. A process that has it never hands off again.
pub(crate) const GUARD: &str = "RIDL_TOOLCHAIN_HANDOFF";

/// Hands the command off when the refusals in `diagnostics` allow it. `None`
/// means the caller prints its refusal as before.
pub(crate) fn refusal(diagnostics: &[Diagnostic], sources: &SourceMap) -> Option<ExitCode> {
    hand_off(diagnostics, sources)
}

fn hand_off(diagnostics: &[Diagnostic], sources: &SourceMap) -> Option<ExitCode> {
    if guarded(std::env::var_os(GUARD).as_deref()) {
        return None;
    }
    let pins = pins(diagnostics, sources).filter(|pins| !pins.is_empty())?;
    let selected = process_toolchain_root()
        .and_then(|root| select_installed(&root, &pins, BUILD_TARGET))
        .ok()
        .flatten();
    let (release, binary) = selected?;
    Some(run(release, &binary))
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
