//! Install and select immutable versioned toolchains.
mod install;
mod paths;
mod selection;
mod uninstall;
pub use install::install;
pub use paths::{process_toolchain_root, release_target, toolchain_root};
pub use selection::*;
pub use uninstall::uninstall;
pub const RELEASE_BASE_URL: &str = "https://github.com/driftsys/ridl/releases/download";
/// The minisign public key of the release signing key. `ridl toolchain install`
/// verifies every release archive against it.
pub const RELEASE_PUBLIC_KEY: &str = "RWSIXzkszArAWT0wSHqsRJyxVp7NvZHNFhPmgJo49DBvXZS40DgAmSjn";

#[cfg(test)]
mod tests {
    #[test]
    fn release_public_key_is_a_minisign_public_key() {
        assert!(minisign_verify::PublicKey::from_base64(super::RELEASE_PUBLIC_KEY).is_ok());
    }
}
