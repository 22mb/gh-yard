use std::fs::OpenOptions;
use std::io::{self, Stderr, Write};

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode, size as terminal_size};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

use crate::scan::Repo;
use crate::tty;

pub enum Outcome {
    Selected(String),
    Aborted,
}

/// The viewport is 40% of the terminal height (at least 3 rows). It is
/// decided once at startup and does not follow terminal resizes.
const HEIGHT_RATIO: u16 = 40;
const MIN_HEIGHT: u16 = 3;

/// Restores the terminal from raw mode even on panic. With
/// `panic = "abort"` no unwinding happens, so a Drop impl alone is not
/// enough — the panic hook runs before the abort and covers that case.
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> Result<Self, String> {
        enable_raw_mode().map_err(|e| format!("cannot enable raw mode: {e}"))?;
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            default_hook(info);
        }));
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

pub fn run(repos: &[Repo]) -> Result<Outcome, String> {
    // Open read-write: the cursor-position query writes to this fd.
    let tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|e| format!("cannot open terminal (/dev/tty): {e}"))?;
    // crossterm reads the size from /dev/tty and only falls back to stdout
    // when /dev/tty is missing, so this works while stdout is a pipe.
    let rows = match terminal_size() {
        Ok((_, rows)) if rows > 0 => rows,
        _ => 24,
    };
    // Widen before multiplying: rows >= 1639 would overflow u16.
    let height =
        ((rows as u32 * HEIGHT_RATIO as u32 / 100) as u16).clamp(MIN_HEIGHT, rows.max(MIN_HEIGHT));

    // Point stdout at the terminal while the TUI runs so nothing but the
    // resulting path ends up in the captured output.
    let stdout_guard = tty::StdoutGuard::redirect_to(&tty)?;

    let raw_mode = RawModeGuard::enable()?;
    let backend = CrosstermBackend::new(io::stderr());
    let terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(height),
        },
    );

    let result = match terminal {
        Ok(mut terminal) => {
            let outcome = event_loop(&mut terminal, repos);
            // Erase the screen on exit (inline viewport: only the viewport
            // and below are cleared).
            let _ = terminal.clear();
            let _ = terminal.show_cursor();
            // clear() restores the cursor to where the input-line cursor was
            // (column 2 + query length), so anything printed next would start
            // there. Return to column 0.
            let _ = write!(io::stderr(), "\r");
            let _ = io::stderr().flush();
            outcome
        }
        Err(e) => Err(format!("cannot initialize terminal: {e}")),
    };

    drop(raw_mode);
    drop(stdout_guard);
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    repos: &[Repo],
) -> Result<Outcome, String> {
    let mut state = State::new(repos);

    loop {
        terminal
            .draw(|frame| state.render(frame))
            .map_err(|e| format!("draw failed: {e}"))?;

        let ev = event::read().map_err(|e| format!("cannot read input: {e}"))?;
        let Event::Key(key) = ev else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match state.handle(key) {
            Action::Continue => {}
            Action::Abort => return Ok(Outcome::Aborted),
            Action::Accept => {
                return Ok(match state.selected_repo() {
                    Some(repo) => Outcome::Selected(repo.abs.to_string_lossy().into_owned()),
                    None => Outcome::Aborted,
                });
            }
        }
    }
}

enum Action {
    Continue,
    Abort,
    Accept,
}

/// One filtered entry. `indices` are match positions into `rel` (in chars).
struct Hit {
    index: usize,
    indices: Vec<u32>,
}

struct State<'a> {
    repos: &'a [Repo],
    query: String,
    /// Editing cursor inside the query, in chars.
    qcursor: usize,
    hits: Vec<Hit>,
    cursor: usize,
    offset: usize,
    matcher: Matcher,
}

impl<'a> State<'a> {
    fn new(repos: &'a [Repo]) -> Self {
        let mut state = Self {
            repos,
            query: String::new(),
            qcursor: 0,
            hits: Vec::new(),
            cursor: 0,
            offset: 0,
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
        };
        state.refilter();
        state
    }

    /// Byte offset of the `pos`-th char in the query.
    fn byte_at(&self, pos: usize) -> usize {
        self.query
            .char_indices()
            .nth(pos)
            .map_or(self.query.len(), |(i, _)| i)
    }

    fn query_chars(&self) -> usize {
        self.query.chars().count()
    }

    /// Display width of the query up to the editing cursor. CJK and other
    /// wide characters occupy two columns, so counting chars is not enough.
    fn width_before_cursor(&self) -> u16 {
        self.query
            .chars()
            .take(self.qcursor)
            .map(|c| c.width().unwrap_or(0) as u32)
            .sum::<u32>()
            .min(u16::MAX as u32) as u16
    }

    fn refilter(&mut self) {
        self.hits.clear();

        if self.query.is_empty() {
            self.hits.extend((0..self.repos.len()).map(|index| Hit {
                index,
                indices: Vec::new(),
            }));
        } else {
            let pattern = Pattern::parse(&self.query, CaseMatching::Smart, Normalization::Smart);
            let mut buf = Vec::new();
            let mut scored: Vec<(u32, Hit)> = Vec::new();
            for (index, repo) in self.repos.iter().enumerate() {
                let mut indices = Vec::new();
                let haystack = Utf32Str::new(&repo.rel, &mut buf);
                if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                    indices.sort_unstable();
                    indices.dedup();
                    scored.push((score, Hit { index, indices }));
                }
            }
            // Score descending; ties keep the original (lexicographic) order.
            scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
            self.hits.extend(scored.into_iter().map(|(_, hit)| hit));
        }

        self.cursor = 0;
        self.offset = 0;
    }

    fn selected_repo(&self) -> Option<&Repo> {
        self.hits.get(self.cursor).map(|hit| &self.repos[hit.index])
    }

    fn handle(&mut self, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Enter => return Action::Accept,
            KeyCode::Esc => return Action::Abort,
            KeyCode::Char('c') if ctrl => return Action::Abort,
            KeyCode::Up => self.move_cursor(-1),
            KeyCode::Down => self.move_cursor(1),
            KeyCode::Char('p' | 'k') if ctrl => self.move_cursor(-1),
            KeyCode::Char('n' | 'j') if ctrl => self.move_cursor(1),
            KeyCode::Left => self.qcursor = self.qcursor.saturating_sub(1),
            KeyCode::Right => self.qcursor = (self.qcursor + 1).min(self.query_chars()),
            KeyCode::Char('b') if ctrl => self.qcursor = self.qcursor.saturating_sub(1),
            KeyCode::Char('f') if ctrl => {
                self.qcursor = (self.qcursor + 1).min(self.query_chars());
            }
            KeyCode::Home => self.qcursor = 0,
            KeyCode::End => self.qcursor = self.query_chars(),
            KeyCode::Char('a') if ctrl => self.qcursor = 0,
            KeyCode::Char('e') if ctrl => self.qcursor = self.query_chars(),
            KeyCode::Char('u') if ctrl => {
                // Delete everything before the cursor (fzf: unix-line-discard).
                let end = self.byte_at(self.qcursor);
                self.query.replace_range(..end, "");
                self.qcursor = 0;
                self.refilter();
            }
            KeyCode::Char('w') if ctrl => {
                self.delete_word_before_cursor();
                self.refilter();
            }
            KeyCode::Backspace => {
                if self.qcursor > 0 {
                    let start = self.byte_at(self.qcursor - 1);
                    let end = self.byte_at(self.qcursor);
                    self.query.replace_range(start..end, "");
                    self.qcursor -= 1;
                    self.refilter();
                }
            }
            KeyCode::Delete => self.delete_at_cursor(),
            KeyCode::Char('d') if ctrl => self.delete_at_cursor(),
            KeyCode::Char(c) if !ctrl => {
                let at = self.byte_at(self.qcursor);
                self.query.insert(at, c);
                self.qcursor += 1;
                self.refilter();
            }
            _ => {}
        }
        Action::Continue
    }

    fn delete_at_cursor(&mut self) {
        if self.qcursor < self.query_chars() {
            let start = self.byte_at(self.qcursor);
            let end = self.byte_at(self.qcursor + 1);
            self.query.replace_range(start..end, "");
            self.refilter();
        }
    }

    fn delete_word_before_cursor(&mut self) {
        let end = self.byte_at(self.qcursor);
        let before = &self.query[..end];
        // Skip separators directly before the cursor first (readline
        // behaviour), otherwise Ctrl-W right after `/` deletes nothing.
        let is_sep = |c: char| c.is_whitespace() || c == '/';
        let stripped = before.trim_end_matches(is_sep);
        // Advance past the boundary char by its own UTF-8 length; a fixed +1
        // would split multi-byte whitespace (e.g. U+3000) and panic.
        let start = stripped
            .char_indices()
            .rev()
            .find(|(_, c)| is_sep(*c))
            .map_or(0, |(pos, c)| pos + c.len_utf8());
        self.query.replace_range(start..end, "");
        self.qcursor = self.query[..start].chars().count();
    }

    fn move_cursor(&mut self, delta: i32) {
        if self.hits.is_empty() {
            return;
        }
        let last = self.hits.len() - 1;
        self.cursor = match delta {
            d if d < 0 => self.cursor.saturating_sub(1),
            _ => (self.cursor + 1).min(last),
        };
    }

    fn render(&mut self, frame: &mut Frame) {
        let [prompt_area, list_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(frame.area());

        self.render_prompt(frame, prompt_area);
        self.render_list(frame, list_area);
    }

    fn render_prompt(&self, frame: &mut Frame, area: Rect) {
        let counter = format!("{}/{}", self.hits.len(), self.repos.len());
        let prompt = Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Red)),
            Span::raw(&self.query),
        ]);
        frame.render_widget(Paragraph::new(prompt), area);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                counter,
                Style::default().fg(Color::DarkGray),
            )))
            .right_aligned(),
            area,
        );

        let cursor_x = (area.x + 2).saturating_add(self.width_before_cursor());
        frame.set_cursor_position((cursor_x.min(area.right().saturating_sub(1)), area.y));
    }

    fn render_list(&mut self, frame: &mut Frame, area: Rect) {
        let visible = area.height as usize;
        if visible == 0 {
            return;
        }
        // Keep the cursor inside the viewport by adjusting the scroll offset.
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else if self.cursor >= self.offset + visible {
            self.offset = self.cursor + 1 - visible;
        }

        let lines: Vec<Line> = self
            .hits
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(visible)
            .map(|(i, hit)| self.render_row(hit, i == self.cursor))
            .collect();

        frame.render_widget(Paragraph::new(lines), area);
    }

    fn render_row(&self, hit: &Hit, selected: bool) -> Line<'static> {
        let repo = &self.repos[hit.index];
        // Dim `host/owner/`; the eye should land on the repository name.
        let dim_upto = repo.rel.rfind('/').map_or(0, |pos| pos + 1);

        let mut spans = vec![Span::styled(
            if selected { "❯ " } else { "  " },
            Style::default().fg(Color::Red),
        )];

        let base = if selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let mut current = String::new();
        let mut current_style: Option<Style> = None;
        // nucleo's Utf32Str splits non-ASCII haystacks by grapheme, so the
        // match indices are grapheme positions — enumerate the same way.
        for (grapheme_index, (byte_index, grapheme)) in repo.rel.grapheme_indices(true).enumerate()
        {
            let style = if hit.indices.contains(&(grapheme_index as u32)) {
                base.fg(Color::Green)
            } else if byte_index < dim_upto {
                base.fg(Color::DarkGray)
            } else {
                base
            };
            if current_style != Some(style) {
                if let Some(prev) = current_style {
                    spans.push(Span::styled(std::mem::take(&mut current), prev));
                }
                current_style = Some(style);
            }
            current.push_str(grapheme);
        }
        if let Some(style) = current_style {
            spans.push(Span::styled(current, style));
        }

        let line = Line::from(spans);
        if selected {
            line.style(Style::default().bg(Color::Indexed(236)))
        } else {
            line
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repos(names: &[&str]) -> Vec<Repo> {
        names
            .iter()
            .map(|n| Repo {
                abs: PathBuf::from(format!("/root/{n}")),
                rel: n.to_string(),
            })
            .collect()
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn type_str(state: &mut State, s: &str) {
        for c in s.chars() {
            state.handle(key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn insert_at_cursor_middle() {
        let repos = repos(&["github.com/a/yard"]);
        let mut state = State::new(&repos);
        type_str(&mut state, "ard");
        state.handle(ctrl('a'));
        type_str(&mut state, "y");
        assert_eq!(state.query, "yard");
        assert_eq!(state.qcursor, 1);
    }

    #[test]
    fn backspace_removes_cjk_before_cursor() {
        let repos = repos(&["github.com/a/b"]);
        let mut state = State::new(&repos);
        type_str(&mut state, "日本語");
        state.handle(key(KeyCode::Backspace));
        assert_eq!(state.query, "日本");
        assert_eq!(state.qcursor, 2);
    }

    #[test]
    fn delete_at_cursor_removes_cjk() {
        let repos = repos(&["github.com/a/b"]);
        let mut state = State::new(&repos);
        type_str(&mut state, "日本語");
        state.handle(ctrl('a'));
        state.handle(key(KeyCode::Delete));
        assert_eq!(state.query, "本語");
        assert_eq!(state.qcursor, 0);
    }

    #[test]
    fn ctrl_w_with_fullwidth_space_does_not_panic() {
        // Regression: the word boundary was computed as byte position + 1,
        // which split multi-byte whitespace (U+3000) and panicked.
        let repos = repos(&["github.com/a/b"]);
        let mut state = State::new(&repos);
        type_str(&mut state, "ab\u{3000}cd");
        state.handle(ctrl('w'));
        assert_eq!(state.query, "ab\u{3000}");
        assert_eq!(state.qcursor, 3);
    }

    #[test]
    fn ctrl_w_deletes_word_before_cursor_only() {
        let repos = repos(&["github.com/a/b"]);
        let mut state = State::new(&repos);
        type_str(&mut state, "foo/bar");
        state.handle(ctrl('a'));
        for _ in 0..4 {
            state.handle(key(KeyCode::Right));
        }
        state.handle(ctrl('w'));
        assert_eq!(state.query, "bar");
        assert_eq!(state.qcursor, 0);
    }

    #[test]
    fn ctrl_u_deletes_before_cursor_only() {
        let repos = repos(&["github.com/a/b"]);
        let mut state = State::new(&repos);
        type_str(&mut state, "日本語abc");
        state.handle(key(KeyCode::Left));
        state.handle(ctrl('u'));
        assert_eq!(state.query, "c");
        assert_eq!(state.qcursor, 0);
    }

    #[test]
    fn cursor_movement_clamps_at_edges() {
        let repos = repos(&["github.com/a/b"]);
        let mut state = State::new(&repos);
        type_str(&mut state, "ab");
        state.handle(key(KeyCode::Right));
        assert_eq!(state.qcursor, 2);
        state.handle(ctrl('a'));
        state.handle(key(KeyCode::Left));
        assert_eq!(state.qcursor, 0);
    }

    #[test]
    fn width_before_cursor_counts_wide_chars_as_two() {
        let repos = repos(&["github.com/a/b"]);
        let mut state = State::new(&repos);
        type_str(&mut state, "日a");
        assert_eq!(state.width_before_cursor(), 3);
    }

    #[test]
    fn filter_matches_and_selects() {
        let repos = repos(&["github.com/a/apple", "github.com/a/yard"]);
        let mut state = State::new(&repos);
        type_str(&mut state, "yard");
        assert_eq!(state.hits.len(), 1);
        assert_eq!(state.selected_repo().unwrap().rel, "github.com/a/yard");
    }

    #[test]
    fn filter_matches_cjk() {
        let repos = repos(&["github.com/a/日本語メモ", "github.com/a/other"]);
        let mut state = State::new(&repos);
        type_str(&mut state, "日本");
        assert_eq!(state.hits.len(), 1);
        assert_eq!(
            state.selected_repo().unwrap().rel,
            "github.com/a/日本語メモ"
        );
    }
}
