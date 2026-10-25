use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsFd, OwnedFd};

use rustix::io::dup;
use rustix::stdio::dup2_stdout;

/// Points stdout at the terminal while the TUI is running.
///
/// ratatui's inline viewport queries the cursor position on startup, but the
/// implementation (`crossterm::cursor::position`) writes the query sequence
/// directly to `io::stdout()` instead of the backend's writer. Without this
/// guard, `\x1b[6n` would leak into the output captured by `$(gh yard)`,
/// so fd 1 is swapped out for the duration.
pub struct StdoutGuard {
    saved: OwnedFd,
}

impl StdoutGuard {
    pub fn redirect_to(tty: &File) -> Result<Self, String> {
        let _ = io::stdout().flush();
        let saved = dup(io::stdout().as_fd()).map_err(|e| format!("cannot save stdout: {e}"))?;
        dup2_stdout(tty).map_err(|e| format!("cannot redirect stdout: {e}"))?;
        Ok(Self { saved })
    }
}

impl Drop for StdoutGuard {
    fn drop(&mut self) {
        let _ = io::stdout().flush();
        // Restore fd 1; `saved` closes itself afterwards.
        let _ = dup2_stdout(&self.saved);
    }
}
