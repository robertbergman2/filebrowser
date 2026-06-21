//! Filesystem access: listing directories and sampling file contents.
//!
//! This module is the only place that touches the filesystem for reading
//! directory contents and previews, keeping IO concerns out of the
//! application state and rendering code.

use std::{
    fs::{self, File, Metadata},
    io::Read,
    path::Path,
    time::SystemTime,
};

use crate::entry::{self, Entry};

/// Number of leading bytes sampled when previewing a file.
const PREVIEW_LIMIT: u64 = 2_000;

/// List the entries in `path`, sorted for display.
///
/// A synthetic `..` entry is prepended when the directory has a parent so the
/// user can always navigate upward. Entries whose metadata cannot be read are
/// skipped rather than aborting the whole listing.
pub fn list_dir(path: &Path) -> Vec<Entry> {
    let mut entries = Vec::new();

    if path.parent().is_some() {
        entries.push(Entry {
            name: "..".to_string(),
            is_dir: true,
            size: 0,
            modified: SystemTime::UNIX_EPOCH,
        });
    }

    let Ok(read_dir) = fs::read_dir(path) else {
        return entries;
    };

    let mut discovered = Vec::new();
    for dir_entry in read_dir.flatten() {
        let Some(metadata) = entry_metadata(&dir_entry) else {
            continue;
        };

        let is_dir = metadata.is_dir();
        discovered.push(Entry {
            name: dir_entry.file_name().to_string_lossy().into_owned(),
            is_dir,
            size: if is_dir { 0 } else { metadata.len() },
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }

    discovered.sort_by(entry::compare);
    entries.extend(discovered);
    entries
}

/// Metadata for a directory entry, following symlinks so a link to a directory
/// is treated as a directory (and is therefore navigable). Falls back to the
/// link's own metadata for broken symlinks so they still appear in the listing.
fn entry_metadata(dir_entry: &fs::DirEntry) -> Option<Metadata> {
    fs::metadata(dir_entry.path())
        .or_else(|_| dir_entry.metadata())
        .ok()
}

/// Read a textual preview of the file at `path`.
///
/// Only the first [`PREVIEW_LIMIT`] bytes are read, so previewing a huge file
/// never loads it entirely into memory. Returns `None` when the file cannot be
/// read, is not a regular file, or appears to be binary (contains a NUL byte
/// in the sampled region). The regular-file check prevents blocking on FIFOs
/// and other special files when the selection changes.
pub fn read_preview(path: &Path) -> Option<String> {
    if !fs::metadata(path).ok()?.is_file() {
        return None;
    }

    let file = File::open(path).ok()?;

    let mut sample = Vec::new();
    file.take(PREVIEW_LIMIT).read_to_end(&mut sample).ok()?;

    if sample.contains(&0) {
        return None;
    }

    Some(String::from_utf8_lossy(&sample).into_owned())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::{
        ffi::CString,
        os::unix::{ffi::OsStrExt, fs::symlink},
    };

    /// A scratch directory removed when the test ends.
    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("filebrowser_{label}_{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn find<'a>(entries: &'a [Entry], name: &str) -> &'a Entry {
        entries
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("entry {name:?} not found"))
    }

    #[test]
    fn symlink_to_directory_is_navigable_as_a_directory() {
        let dir = TempDir::new("symlink_dir");
        fs::create_dir(dir.path.join("realdir")).unwrap();
        symlink(dir.path.join("realdir"), dir.path.join("linkdir")).unwrap();
        fs::write(dir.path.join("file.txt"), b"hello").unwrap();

        let entries = list_dir(&dir.path);

        assert!(
            find(&entries, "linkdir").is_dir,
            "symlink to dir should list as a directory"
        );
        assert!(!find(&entries, "file.txt").is_dir);
    }

    #[test]
    fn broken_symlink_still_appears_in_listing() {
        let dir = TempDir::new("broken_symlink");
        symlink(dir.path.join("missing"), dir.path.join("dangling")).unwrap();

        let entries = list_dir(&dir.path);

        assert!(!find(&entries, "dangling").is_dir);
    }

    #[test]
    fn preview_rejects_non_regular_files() {
        let dir = TempDir::new("non_regular_preview");

        assert_eq!(read_preview(&dir.path), None);
    }

    #[test]
    fn preview_rejects_fifo_without_opening_it() {
        let dir = TempDir::new("fifo_preview");
        let fifo = dir.path.join("pipe");
        let path = CString::new(fifo.as_os_str().as_bytes()).unwrap();

        // SAFETY: `path` is a NUL-terminated path owned by this test, and the
        // requested permissions do not grant access beyond the current user.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);

        assert_eq!(read_preview(&fifo), None);
    }

    #[test]
    fn preview_follows_symlinks_to_regular_files() {
        let dir = TempDir::new("symlink_file_preview");
        fs::write(dir.path.join("target.txt"), b"hello").unwrap();
        symlink(dir.path.join("target.txt"), dir.path.join("link.txt")).unwrap();

        assert_eq!(
            read_preview(&dir.path.join("link.txt")),
            Some("hello".to_string())
        );
    }
}
