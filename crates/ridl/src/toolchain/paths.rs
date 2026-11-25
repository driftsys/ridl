use std::{
    collections::BTreeMap,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

/// Resolves the platform data directory without consulting process environment.
pub fn toolchain_root(
    platform: &str,
    env: &BTreeMap<String, OsString>,
    home: Option<&Path>,
) -> io::Result<PathBuf> {
    let value = |key: &str| env.get(key).filter(|v| !v.is_empty()).map(PathBuf::from);
    let home = || {
        home.filter(|h| !h.as_os_str().is_empty())
            .ok_or_else(|| io::Error::other("toolchain directory requires a home directory"))
    };
    let path = if let Some(path) = value("RIDL_TOOLCHAIN_DIR") {
        path
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

pub(super) fn binary_name() -> &'static str {
    if cfg!(windows) { "ridl.exe" } else { "ridl" }
}
