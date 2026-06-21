use std::{
    cmp::Ordering,
    env,
    error::Error,
    fs,
    io::{self, Stdout},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};

// Data model

struct App {
    current_dir: PathBuf,
    entries: Vec<Entry>,
    selected: usize,
    scroll_offset: usize,
    should_quit: bool,
}

#[derive(Clone)]
struct Entry {
    name: String,
    is_dir: bool,
    size: u64,
    modified: SystemTime,
}

impl App {
    fn new() -> Self {
        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let entries = list_dir(&current_dir);

        Self {
            current_dir,
            entries,
            selected: 0,
            scroll_offset: 0,
            should_quit: false,
        }
    }

    fn reload_entries(&mut self) {
        self.entries = list_dir(&self.current_dir);
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        self.scroll_offset = self.scroll_offset.min(self.selected);
    }

    fn enter_selected(&mut self) {
        let Some(entry) = self.entries.get(self.selected) else {
            return;
        };

        if !entry.is_dir {
            return;
        }

        let next_dir = if entry.name == ".." {
            self.current_dir.parent().map(Path::to_path_buf)
        } else {
            Some(self.current_dir.join(&entry.name))
        };

        if let Some(path) = next_dir {
            if path.is_dir() {
                self.current_dir = fs::canonicalize(&path).unwrap_or(path);
                self.selected = 0;
                self.scroll_offset = 0;
                self.reload_entries();
            }
        }
    }

    fn enter_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.selected = 0;
            self.scroll_offset = 0;
            self.reload_entries();
        }
    }

    fn move_down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn jump_first(&mut self) {
        self.selected = 0;
    }

    fn jump_last(&mut self) {
        self.selected = self.entries.len().saturating_sub(1);
    }
}

// Directory loading

fn list_dir(path: &Path) -> Vec<Entry> {
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
        let name = dir_entry.file_name().to_string_lossy().into_owned();
        let metadata = match dir_entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        let is_dir = metadata.is_dir();
        let size = if is_dir { 0 } else { metadata.len() };
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        discovered.push(Entry {
            name,
            is_dir,
            size,
            modified,
        });
    }

    discovered.sort_by(compare_entries);
    entries.extend(discovered);
    entries
}

fn compare_entries(left: &Entry, right: &Entry) -> Ordering {
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

// Terminal lifecycle

type Tui = Terminal<CrosstermBackend<Stdout>>;

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = teardown_terminal();
    }
}

fn setup_terminal() -> Result<Tui, Box<dyn Error>> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide) {
        let _ = disable_raw_mode();
        return Err(Box::new(error));
    }

    let backend = CrosstermBackend::new(stdout);
    match Terminal::new(backend) {
        Ok(terminal) => Ok(terminal),
        Err(error) => {
            let _ = teardown_terminal();
            Err(Box::new(error))
        }
    }
}

fn teardown_terminal() -> io::Result<()> {
    let mut first_error = disable_raw_mode().err();
    let mut stdout = io::stdout();

    if let Err(error) = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture, Show) {
        if first_error.is_none() {
            first_error = Some(error);
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

// Event loop

fn run() -> Result<(), Box<dyn Error>> {
    let mut terminal = setup_terminal()?;
    let _terminal_guard = TerminalGuard;
    let mut app = App::new();

    loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key_event) = event::read()? {
                handle_event(&mut app, key_event);
            }
        }
    }

    Ok(())
}

fn handle_event(app: &mut App, key_event: KeyEvent) {
    if key_event.kind != KeyEventKind::Press {
        return;
    }

    match key_event.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
        KeyCode::Char('k') | KeyCode::Up => app.move_up(),
        KeyCode::Char('g') | KeyCode::Home => app.jump_first(),
        KeyCode::Char('G') | KeyCode::End => app.jump_last(),
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => app.enter_selected(),
        KeyCode::Char('h') | KeyCode::Left => app.enter_parent(),
        _ => {}
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("filebrowser: {error}");
        std::process::exit(1);
    }
}

// UI rendering

mod ui {
    use super::*;

    pub fn render(frame: &mut Frame<'_>, app: &mut App) {
        let area = frame.area();
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(area);

        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(vertical_chunks[1]);

        let title = Paragraph::new(app.current_dir.display().to_string())
            .block(Block::default().borders(Borders::ALL).title("Path"));
        frame.render_widget(title, vertical_chunks[0]);

        render_file_list(frame, app, body_chunks[0]);
        render_preview(frame, app, body_chunks[1]);

        let status = Paragraph::new(status_text(app))
            .block(Block::default().borders(Borders::ALL).title("Keys"));
        frame.render_widget(status, vertical_chunks[2]);
    }

    fn status_text(app: &App) -> String {
        let selected_details = app.entries.get(app.selected).map_or_else(
            || "no selection".to_string(),
            |entry| {
                let modified = entry
                    .modified
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs());
                format!("selected: {} bytes, modified: {modified}s", entry.size)
            },
        );

        format!(
            "j/k or arrows: move | h/l or arrows: parent/open | g/G: first/last | q or Ctrl+C: quit | {} items | {selected_details}",
            app.entries.len()
        )
    }

    fn render_file_list(frame: &mut Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
        let visible_height = area.height.saturating_sub(2) as usize;
        keep_selected_visible(app, visible_height);

        let items = app
            .entries
            .iter()
            .skip(app.scroll_offset)
            .take(visible_height)
            .enumerate()
            .map(|(visible_index, entry)| {
                let absolute_index = app.scroll_offset + visible_index;
                let prefix = if entry.is_dir { "/" } else { " " };
                let style = if absolute_index == app.selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(vec![
                    Span::raw(prefix),
                    Span::raw(" "),
                    Span::raw(entry.name.as_str()),
                ]))
                .style(style)
            })
            .collect::<Vec<_>>();

        let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Files"));
        frame.render_widget(list, area);
    }

    fn keep_selected_visible(app: &mut App, visible_height: usize) {
        if visible_height == 0 {
            app.scroll_offset = app.selected;
            return;
        }

        if app.selected < app.scroll_offset {
            app.scroll_offset = app.selected;
        } else if app.selected >= app.scroll_offset + visible_height {
            app.scroll_offset = app.selected + 1 - visible_height;
        }
    }

    fn render_preview(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
        let (title, content) = preview_content(app);
        let preview = Paragraph::new(content)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(title));

        frame.render_widget(preview, area);
    }

    fn preview_content(app: &App) -> (String, String) {
        let Some(entry) = app.entries.get(app.selected) else {
            return ("Preview".to_string(), String::new());
        };

        if entry.is_dir {
            return (entry.name.clone(), "<directory>".to_string());
        }

        let path = app.current_dir.join(&entry.name);
        let Ok(bytes) = fs::read(&path) else {
            return (entry.name.clone(), "<binary or unreadable>".to_string());
        };

        let sample_len = bytes.len().min(2_000);
        let sample = &bytes[..sample_len];

        if sample.contains(&0) {
            return (entry.name.clone(), "<binary or unreadable>".to_string());
        }

        let content = String::from_utf8_lossy(sample).into_owned();
        (entry.name.clone(), content)
    }
}
