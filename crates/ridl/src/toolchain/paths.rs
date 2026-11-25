use std::{
    collections::BTreeMap,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

/// Resolves the platform data directory without consulting process environment.
/// A `RIDL_TOOLCHAIN_DIR` override must be an absolute path.
pub fn toolchain_root(
    platform: &str,
    env: &BTreeMap<String, OsString>,
    home: Option<&Path>,
) -> io::Result<PathBuf> {
    let value = |key: &str| {
        let value = if platform == "windows" {
            env.iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(key))
                .map(|(_, value)| value)
        } else {
            env.get(key)
        };
        value.filter(|v| !v.is_empty()).map(PathBuf::from)
    };
    let home = || {
        home.filter(|h| !h.as_os_str().is_empty())
            .ok_or_else(|| io::Error::other("toolchain directory requires a home directory"))
    };
    let path = if let Some(path) = value("RIDL_TOOLCHAIN_DIR") {
        if !path.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "RIDL_TOOLCHAIN_DIR must be an absolute path, not `{}`",
                    path.display()
                ),
            ));
        }
        return Ok(path);
    } else {
        match platform {
            "linux" => match value("XDG_DATA_HOME") {
                Some(data) => data.join("ridl"),
                None => home()?.join(".local/share/ridl"),
            },
            "macos" | "darwin" => home()?.join("Library/Application Support/ridl"),
            "windows" => value("LOCALAPPDATA")
                .ok_or_else(|| io::Error::other("toolchain directory requires LOCALAPPDATA"))?
                .join("ridl"),
            _ => {
                return Err(io::Error::other(format!(
                    "unsupported toolchain platform {platform}"
                )));
            }
        }
    };
    std::path::absolute(path)
}

/// Resolves the toolchain root from the process environment.
pub fn process_toolchain_root() -> io::Result<PathBuf> {
    let environment = std::env::vars_os()
        .filter_map(|(key, value)| key.into_string().ok().map(|key| (key, value)))
        .collect::<BTreeMap<_, _>>();
    let home = environment.get("HOME").map(PathBuf::from);
    toolchain_root(std::env::consts::OS, &environment, home.as_deref())
}

/// Rejects triples for which the editor release publishes no binary archive.
pub fn release_target(target: &str) -> io::Result<&str> {
    match target {
        "x86_64-unknown-linux-gnu"
        | "aarch64-unknown-linux-gnu"
        | "x86_64-apple-darwin"
        | "aarch64-apple-darwin"
        | "x86_64-pc-windows-msvc" => Ok(target),
        _ => Err(io::Error::other(format!(
            "no ridl release archive for target {target}"
        ))),
    }
}

pub(super) fn binary_name(target: &str) -> &'static str {
    if target.contains("-windows-") {
        "ridl.exe"
    } else {
        "ridl"
    }
}
