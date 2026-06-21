//! The domain model: a single entry within a directory.

use std::{cmp::Ordering, time::SystemTime};

/// A file or directory listed in the current view.
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: SystemTime,
}

/// Ordering used to display entries: directories first, then a
/// case-insensitive name comparison (falling back to a case-sensitive
/// comparison so that names differing only in case remain stable).
pub fn compare(left: &Entry, right: &Entry) -> Ordering {
    match (left.is_dir, right.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => left
            .name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name)),
    }
}
