//! The compiler release, with a debug-only integration-test override.
use ridl_core::release::{Release, parse_release};

/// The release used for root pin checks. Invalid stamps use the package release.
pub fn running_release() -> Release {
    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var("RIDL_TEST_RUNNING_RELEASE")
        && let Some(release) = parse_release(&value)
    {
        return release;
    }
    stamped_release(env!("RIDL_BUILD_VERSION"))
}

fn stamped_release(stamp: &str) -> Release {
    parse_release(stamp)
        .unwrap_or_else(|| parse_release(env!("CARGO_PKG_VERSION")).expect("package release"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_stamp_with_prerelease_suffix_uses_package_version() {
        assert_eq!(
            stamped_release("9.9.9-test"),
            parse_release(env!("CARGO_PKG_VERSION")).unwrap()
        );
        assert_eq!(stamped_release("0.23.2").to_string(), "0.23.2");
    }
}
