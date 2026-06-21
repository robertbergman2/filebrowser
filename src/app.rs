//! Application state and the navigation logic that mutates it.
//!
//! `App` owns its own invariants: callers move the selection or change
//! directories through methods, and the struct keeps `selected` and
//! `scroll_offset` consistent with the current entries.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::entry::Entry;
use crate::filesystem;

pub struct App {
    current_dir: PathBuf,
    entries: Vec<Entry>,
    selected: usize,
    scroll_offset: usize,
    preview_scroll: usize,
    should_quit: bool,
}

impl App {
    /// Create an app rooted at the current working directory, falling back to
    /// `.` if it cannot be determined.
    pub fn new() -> Self {
        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let entries = filesystem::list_dir(&current_dir);

        Self {
            current_dir,
            entries,
            selected: 0,
            scroll_offset: 0,
            preview_scroll: 0,
            should_quit: false,
        }
    }

    // --- Read-only accessors used by rendering ---

    pub fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn preview_scroll(&self) -> usize {
        self.preview_scroll
    }

    /// The entry under the cursor, if any.
    pub fn selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    // --- Commands ---

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
            self.preview_scroll = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.preview_scroll = 0;
        }
    }

    pub fn jump_first(&mut self) {
        if self.selected != 0 {
            self.selected = 0;
            self.preview_scroll = 0;
        }
    }

    pub fn jump_last(&mut self) {
        let last = self.entries.len().saturating_sub(1);
        if self.selected != last {
            self.selected = last;
            self.preview_scroll = 0;
        }
    }

    /// Open the selected directory (or follow `..`). Does nothing for files.
    pub fn enter_selected(&mut self) {
        let Some(entry) = self.entries.get(self.selected) else {
            return;
        };

        if !entry.is_dir {
            return;
        }

        let target = if entry.name == ".." {
            self.current_dir.parent().map(Path::to_path_buf)
        } else {
            Some(self.current_dir.join(&entry.name))
        };

        if let Some(path) = target {
            self.navigate_to(path);
        }
    }

    /// Navigate to the parent of the current directory, if one exists.
    pub fn enter_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.navigate_to(parent.to_path_buf());
        }
    }

    /// Adjust the scroll offset so the selected entry stays within a window of
    /// `visible_height` rows. Called by the renderer once the row count of the
    /// list area is known.
    pub fn clamp_scroll(&mut self, visible_height: usize) {
        if visible_height == 0 {
            self.scroll_offset = self.selected;
            return;
        }

        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected + 1 - visible_height;
        }
    }

    /// Over-scrolling is harmless: the renderer clamps to the file's length.
    pub fn scroll_preview_down(&mut self, amount: usize) {
        self.preview_scroll = self.preview_scroll.saturating_add(amount);
    }

    pub fn scroll_preview_up(&mut self, amount: usize) {
        self.preview_scroll = self.preview_scroll.saturating_sub(amount);
    }

    /// Keep `preview_scroll` within `[0, total_lines - visible_height]` so the
    /// viewport never shows empty rows past the end of the file. Called by the
    /// renderer once the preview's wrapped line count and pane height are known.
    pub fn clamp_preview_scroll(&mut self, total_lines: usize, visible_height: usize) {
        let max = total_lines.saturating_sub(visible_height);
        if self.preview_scroll > max {
            self.preview_scroll = max;
        }
    }

    /// Move into `path` if it resolves to a readable directory, resetting the
    /// cursor and reloading entries.
    fn navigate_to(&mut self, path: PathBuf) {
        if !path.is_dir() {
            return;
        }

        self.current_dir = fs::canonicalize(&path).unwrap_or(path);
        self.selected = 0;
        self.scroll_offset = 0;
        self.preview_scroll = 0;
        self.reload_entries();
    }

    fn reload_entries(&mut self) {
        self.entries = filesystem::list_dir(&self.current_dir);
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        self.scroll_offset = self.scroll_offset.min(self.selected);
    }
}
