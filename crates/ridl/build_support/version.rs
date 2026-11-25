/// Chooses the build stamp from an optional workflow value.
pub fn build_version(value: Option<&str>) -> &str {
    normalise_version(value.unwrap_or_default())
}

/// Trims a workflow stamp and removes one tag prefix. Blank input uses the crate version.
pub fn normalise_version(value: &str) -> &str {
    let value = value.trim();
    if value.is_empty() {
        return env!("CARGO_PKG_VERSION");
    }
    value
        .strip_prefix("editor-v")
        .or_else(|| value.strip_prefix('v'))
        .unwrap_or(value)
}
