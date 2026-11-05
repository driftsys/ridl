//! `cargo xtask bench pixel`: runs the encoding bench on a Pixel 5 over adb.
//!
//! The harness is cross-built for `aarch64-unknown-linux-musl` (a static
//! binary, which runs on Android), pushed to `/data/local/tmp`, and run
//! pinned to one A55 core and, in a second run, to one A76 core. The phone is
//! not rooted, so the run changes no device setting: it pushes a file, runs
//! it, and reads files under `/sys`.
//!
//! The Pixel 5 here is a Snapdragon 765G: cpu0 to cpu5 are A55 cores (up to
//! 1.8 GHz), cpu6 is an A76 (2.2 GHz) and cpu7 is the prime A76 (2.4 GHz).

use std::path::{Path, PathBuf};
use std::process::Command;

const TARGET: &str = "aarch64-unknown-linux-musl";
const DEVICE_DIR: &str = "/data/local/tmp";
const DEVICE_BINARY: &str = "/data/local/tmp/encoding-bench";

/// Seconds of CPU time per cell and encoding of the unpaced samples, and also
/// the wall time of each paced run. The paced figure comes from `getrusage`,
/// whose CPU time may advance in ticks of 4 to 10 ms on Linux. A run of 2 s
/// makes a 10 ms tick 0.5 % of the run, below the 1 % limit.
const DURATION_S: &str = "2";

/// A core class of the phone and the one core a run is pinned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Core {
    A55,
    A76,
}

impl Core {
    pub const ALL: [Core; 2] = [Core::A55, Core::A76];

    /// The label the harness writes to the provenance, and the name in the
    /// result file.
    pub fn label(self) -> &'static str {
        match self {
            Core::A55 => "A55",
            Core::A76 => "A76",
        }
    }

    /// The index of the core the run is pinned to: cpu2 is one of the A55
    /// cores (cpu0 to cpu5), cpu6 is the lower-clocked A76.
    pub fn cpu(self) -> u32 {
        match self {
            Core::A55 => 2,
            Core::A76 => 6,
        }
    }

    /// The hexadecimal affinity mask `taskset` takes for the core.
    pub fn mask(self) -> String {
        format!("{:x}", 1u32 << self.cpu())
    }
}

/// `pixel5-<core>-<date>.json`, for example `pixel5-a55-2026-11-05.json`.
pub fn result_name(core: Core, date: &str) -> String {
    format!("pixel5-{}-{date}.json", core.label().to_lowercase())
}

/// The serials of the devices that `adb devices` lists as ready. A device in
/// another state (`unauthorized`, `offline`) is an error, because a run on it
/// would fail later with a less clear message.
pub fn parse_devices(output: &str) -> Result<Vec<String>, String> {
    let mut ready = Vec::new();
    for line in output
        .lines()
        .skip_while(|l| !l.starts_with("List of devices"))
    {
        let mut fields = line.split_whitespace();
        let (Some(serial), Some(state)) = (fields.next(), fields.next()) else {
            continue;
        };
        if serial == "List" {
            continue;
        }
        if state == "device" {
            ready.push(serial.to_string());
        } else {
            return Err(format!(
                "device {serial} is in the state `{state}`, not `device`; authorize it on the phone"
            ));
        }
    }
    Ok(ready)
}

/// The serial to use, given the listed devices and the `ANDROID_SERIAL`
/// variable. adb itself rejects a variable that names no device only when it
/// runs a command, so the check is made here, before the cross-build.
pub fn choose_device(listed: &[String], android_serial: Option<&str>) -> Result<String, String> {
    if let Some(wanted) = android_serial.filter(|s| !s.is_empty()) {
        return if listed.iter().any(|s| s == wanted) {
            Ok(wanted.to_string())
        } else {
            Err(format!(
                "ANDROID_SERIAL is `{wanted}`, which `adb devices` does not list as ready (listed: {})",
                if listed.is_empty() {
                    "none".to_string()
                } else {
                    listed.join(", ")
                }
            ))
        };
    }
    match listed {
        [] => Err("no Android device is connected: `adb devices` lists none. Connect the Pixel 5 with USB debugging enabled and authorize this computer".to_string()),
        [one] => Ok(one.clone()),
        many => Err(format!(
            "several devices are connected ({}); set ANDROID_SERIAL to the Pixel 5",
            many.join(", ")
        )),
    }
}

/// The shell command that runs the harness pinned to `core`.
pub fn run_command(core: Core, commit: &str) -> String {
    format!(
        "taskset {mask} {DEVICE_BINARY} --out {DEVICE_DIR}/{json} --core-label {label} \
         --duration {DURATION_S} --paced --commit {commit}",
        mask = core.mask(),
        json = device_json(core),
        label = core.label(),
    )
}

fn device_json(core: Core) -> String {
    format!("encoding-bench-{}.json", core.label().to_lowercase())
}

/// A shell command that prints, for each of the eight cores, the governor and
/// the current frequency in kHz: `cpu<N> <governor> <kHz>`.
pub fn cpufreq_command() -> String {
    "for c in 0 1 2 3 4 5 6 7; do d=/sys/devices/system/cpu/cpu$c/cpufreq; \
     echo cpu$c $(cat $d/scaling_governor 2>&1) $(cat $d/scaling_cur_freq 2>&1); done"
        .to_string()
}

/// The text of the sidecar file: the governor and frequencies before and
/// after the run.
pub fn cpufreq_record(before: &str, after: &str) -> String {
    format!(
        "before:\n{}\nafter:\n{}\n",
        before.trim_end(),
        after.trim_end()
    )
}

/// The linker wrapper for the cross-build. rustc passes
/// `-Wl,--fix-cortex-a53-843419` for this target, and `zig cc` of zig 0.16
/// rejects the argument ("unsupported linker arg"). The wrapper drops that
/// argument and runs the same `zig cc` that `cargo zigbuild` runs. The file
/// is named `cc` so that rustc treats it as a `cc`-style linker.
const LINKER_WRAPPER: &str = "#!/bin/sh
for a; do
  shift
  case \"$a\" in
    -Wl,--fix-cortex-a53-843419) ;;
    *) set -- \"$@\" \"$a\" ;;
  esac
done
exec cargo-zigbuild zig cc -- -g -fno-sanitize=all -target aarch64-linux-musl \"$@\"
";

/// Writes the linker wrapper under `dir` and returns its path.
fn write_linker_wrapper(dir: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let path = dir.join("cc");
    std::fs::write(&path, LINKER_WRAPPER).map_err(|e| format!("write {}: {e}", path.display()))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("chmod {}: {e}", path.display()))?;
    Ok(path)
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has a parent directory")
        .to_path_buf()
}

fn output(cmd: &mut Command, what: &str) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("run {what}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{what} failed: {}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn adb(serial: &str) -> Command {
    let mut cmd = Command::new("adb");
    cmd.args(["-s", serial]);
    cmd
}

fn adb_shell(serial: &str, script: &str) -> Result<String, String> {
    output(adb(serial).args(["shell", script]), "adb shell")
}

pub fn run() -> Result<(), String> {
    let listing = output(Command::new("adb").arg("devices"), "adb devices")?;
    let serial = choose_device(
        &parse_devices(&listing)?,
        std::env::var("ANDROID_SERIAL").ok().as_deref(),
    )?;

    super::generate()?;
    let root = root();
    let manifest = root.join("bench/encodings/Cargo.toml");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let linker = write_linker_wrapper(&root.join("bench/encodings/target/zig-linker"))?;
    let rustflags = format!(
        "{} -C linker={}",
        std::env::var("RUSTFLAGS").unwrap_or_default(),
        linker.display()
    );
    let status = Command::new(cargo)
        .env("RUSTFLAGS", rustflags.trim())
        .args([
            "zigbuild",
            "--release",
            "--target",
            TARGET,
            "-p",
            "encoding-bench",
        ])
        .arg("--manifest-path")
        .arg(&manifest)
        .status()
        .map_err(|e| format!("run cargo zigbuild: {e}"))?;
    if !status.success() {
        return Err(format!("cargo zigbuild failed: {status}"));
    }
    let binary = root.join(format!(
        "bench/encodings/target/{TARGET}/release/encoding-bench"
    ));
    output(
        adb(&serial).arg("push").arg(&binary).arg(DEVICE_BINARY),
        "adb push",
    )?;

    // The phone has no repository, so the commit comes from this side.
    let commit = output(
        Command::new("git")
            .current_dir(&root)
            .args(["rev-parse", "HEAD"]),
        "git rev-parse",
    )?;
    let commit = commit.trim();
    let date = output(Command::new("date").args(["-u", "+%F"]), "date")?;
    let date = date.trim();
    let results = root.join("bench/encodings/results");
    std::fs::create_dir_all(&results).map_err(|e| format!("create {}: {e}", results.display()))?;

    for core in Core::ALL {
        println!("running on {} (cpu{})", core.label(), core.cpu());
        let before = adb_shell(&serial, &cpufreq_command())?;
        let log = adb_shell(&serial, &run_command(core, commit))?;
        print!("{log}");
        let after = adb_shell(&serial, &cpufreq_command())?;
        let json = results.join(result_name(core, date));
        output(
            adb(&serial)
                .arg("pull")
                .arg(format!("{DEVICE_DIR}/{}", device_json(core)))
                .arg(&json),
            "adb pull",
        )?;
        let record = json.with_extension("cpufreq.txt");
        std::fs::write(&record, cpufreq_record(&before, &after))
            .map_err(|e| format!("write {}: {e}", record.display()))?;
        println!("wrote {} and {}", json.display(), record.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: &str = "List of devices attached\n0A271FDD4003XY\tdevice\n\n";

    #[test]
    fn an_empty_listing_is_an_explicit_error() {
        let listed = parse_devices("List of devices attached\n\n").unwrap();
        let error = choose_device(&listed, None).unwrap_err();
        assert!(error.contains("no Android device is connected"), "{error}");
    }

    #[test]
    fn a_ready_device_is_chosen_and_daemon_noise_is_ignored() {
        let text = "* daemon not running; starting now at tcp:5037\n* daemon started successfully\n"
            .to_string() + ONE;
        let listed = parse_devices(&text).unwrap();
        assert_eq!(listed, ["0A271FDD4003XY"]);
        assert_eq!(choose_device(&listed, None).unwrap(), "0A271FDD4003XY");
    }

    #[test]
    fn an_unauthorized_device_is_an_error_naming_the_state() {
        let error = parse_devices("List of devices attached\nABC\tunauthorized\n").unwrap_err();
        assert!(
            error.contains("unauthorized") && error.contains("ABC"),
            "{error}"
        );
    }

    #[test]
    fn android_serial_must_name_a_listed_device() {
        let listed = parse_devices(ONE).unwrap();
        let error = choose_device(&listed, Some("none")).unwrap_err();
        assert!(
            error.contains("ANDROID_SERIAL") && error.contains("none"),
            "{error}"
        );
        assert_eq!(
            choose_device(&listed, Some("0A271FDD4003XY")).unwrap(),
            "0A271FDD4003XY"
        );
        assert!(choose_device(&listed, Some("")).is_ok());
    }

    #[test]
    fn two_devices_need_a_serial() {
        let listed = parse_devices("List of devices attached\nA\tdevice\nB\tdevice\n").unwrap();
        assert!(
            choose_device(&listed, None)
                .unwrap_err()
                .contains("several")
        );
        assert_eq!(choose_device(&listed, Some("B")).unwrap(), "B");
    }

    #[test]
    fn the_a55_run_is_pinned_to_one_core_of_cpu0_to_cpu5() {
        let command = run_command(Core::A55, "abc");
        let mask = u32::from_str_radix(&Core::A55.mask(), 16).unwrap();
        assert_eq!(mask.count_ones(), 1, "one core only");
        assert!(
            mask != 0 && mask <= 0x20,
            "mask {mask:#x} is outside cpu0..cpu5"
        );
        assert!(
            command.starts_with(&format!("taskset {} ", Core::A55.mask())),
            "{command}"
        );
        assert!(command.contains("--core-label A55"), "{command}");
    }

    #[test]
    fn the_a76_run_is_pinned_to_cpu6() {
        let command = run_command(Core::A76, "abc");
        assert_eq!(Core::A76.mask(), "40");
        assert!(command.starts_with("taskset 40 "), "{command}");
        assert!(command.contains("--core-label A76"), "{command}");
        assert!(!command.contains("A55"), "{command}");
    }

    #[test]
    fn the_run_is_paced_with_the_commit_and_a_long_enough_duration() {
        let command = run_command(Core::A76, "deadbeef");
        assert!(
            command.contains(" --out /data/local/tmp/encoding-bench-a76.json "),
            "{command}"
        );
        assert!(command.contains("--paced"), "{command}");
        assert!(command.contains("--commit deadbeef"), "{command}");
        assert!(
            command.contains(&format!("--duration {DURATION_S}")),
            "{command}"
        );
        // A 10 ms CPU-time tick must be below 1 % of the run.
        assert!(DURATION_S.parse::<f64>().unwrap() * 0.01 > 0.010);
    }

    #[test]
    fn the_linker_wrapper_drops_only_the_unsupported_argument() {
        assert!(LINKER_WRAPPER.contains("-Wl,--fix-cortex-a53-843419) ;;"));
        assert!(LINKER_WRAPPER.contains("zig cc"));
        assert!(LINKER_WRAPPER.contains("-target aarch64-linux-musl"));
        let dir = std::env::temp_dir().join(format!("pixel-wrapper-{}", std::process::id()));
        let path = write_linker_wrapper(&dir).unwrap();
        assert_eq!(path.file_name().unwrap(), "cc");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_result_file_names_the_core_and_the_date() {
        assert_eq!(
            result_name(Core::A55, "2026-11-05"),
            "pixel5-a55-2026-11-05.json"
        );
        assert_eq!(
            result_name(Core::A76, "2026-11-05"),
            "pixel5-a76-2026-11-05.json"
        );
    }

    #[test]
    fn the_device_json_of_each_core_differs() {
        assert_ne!(device_json(Core::A55), device_json(Core::A76));
        assert!(run_command(Core::A55, "x").contains(&device_json(Core::A55)));
    }

    #[test]
    fn the_cpufreq_command_reads_all_eight_cores_and_only_reads() {
        let command = cpufreq_command();
        for c in 0..8 {
            assert!(command.contains(&c.to_string()));
        }
        assert!(command.contains("scaling_governor") && command.contains("scaling_cur_freq"));
        assert!(!command.contains("setprop") && !command.contains("su "));
    }

    #[test]
    fn the_record_keeps_before_and_after_apart() {
        let text = cpufreq_record("cpu0 schedutil 300000\n", "cpu0 schedutil 1800000\n");
        assert_eq!(
            text,
            "before:\ncpu0 schedutil 300000\nafter:\ncpu0 schedutil 1800000\n"
        );
    }
}
