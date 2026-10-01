//! Resolve the width separately from the pure formatting entry point.

use crate::FormatOptions;
use ec4rs::property::MaxLineLen;
use std::path::Path;

impl FormatOptions {
    /// Resolve `max_line_length` for a file using EditorConfig precedence.
    ///
    /// Missing, unset, invalid, or unreadable configuration uses the default.
    /// Indentation always remains canonical; no other property is read.
    pub fn for_path(path: &Path) -> Self {
        let width = ec4rs::properties_of(path)
            .ok()
            .and_then(|properties| properties.get::<MaxLineLen>().ok());
        match width {
            Some(MaxLineLen::Value(width)) => Self {
                max_line_length: Some(width),
            },
            Some(MaxLineLen::Off) => Self {
                max_line_length: None,
            },
            None => Self::default(),
        }
    }
}
