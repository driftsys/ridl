//! The public API of the semver test fixture: `kept`, which every test keeps,
//! and `removed`, which `semver_check_flags_a_removed_item` deletes. `removed`
//! has no doc comment, so that deleting its one line leaves a crate that
//! compiles.

/// Kept by every test.
pub fn kept() {}

pub fn removed() {}
