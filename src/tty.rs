use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, RawFd};

/// Returns the number of rows of the terminal.
///
/// crossterm's `terminal::size()` looks at stdout, which can fail when stdout
/// is a pipe as in `$(gh yard)`. Query the opened /dev/tty directly instead.
pub fn rows(tty: &File) -> Option<u16> {
    #[repr(C)]
    struct WinSize {
        rows: libc::c_ushort,
        cols: libc::c_ushort,
        x: libc::c_ushort,
        y: libc::c_ushort,
    }

    let mut size = WinSize {
        rows: 0,
        cols: 0,
        x: 0,
        y: 0,
    };
    // SAFETY: the fd belongs to an open File; size is the struct ioctl expects.
    let ret = unsafe { libc::ioctl(tty.as_raw_fd(), libc::TIOCGWINSZ, &raw mut size) };
    if ret != 0 || size.rows == 0 {
        return None;
    }
    Some(size.rows)
}

/// Points stdout at the terminal while the TUI is running.
///
/// ratatui's inline viewport queries the cursor position on startup, but the
/// implementation (`crossterm::cursor::position`) writes the query sequence
/// directly to `io::stdout()` instead of the backend's writer. Without this
/// guard, `\x1b[6n` would leak into the output captured by `$(gh yard)`,
/// so fd 1 is swapped out for the duration.
pub struct StdoutGuard {
    saved: RawFd,
}

impl StdoutGuard {
    pub fn redirect_to(tty: &File) -> Result<Self, String> {
        let _ = io::stdout().flush();
        // SAFETY: fd 1 is always valid; failure is detected via the return value.
        let saved = unsafe { libc::dup(libc::STDOUT_FILENO) };
        if saved < 0 {
            return Err(format!(
                "cannot save stdout: {}",
                io::Error::last_os_error()
            ));
        }
        // SAFETY: tty is an open File; fd 1 is valid.
        if unsafe { libc::dup2(tty.as_raw_fd(), libc::STDOUT_FILENO) } < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(saved) };
            return Err(format!("cannot redirect stdout: {err}"));
        }
        Ok(Self { saved })
    }
}

impl Drop for StdoutGuard {
    fn drop(&mut self) {
        let _ = io::stdout().flush();
        // SAFETY: saved is a valid fd obtained from dup; close it after restoring.
        unsafe {
            libc::dup2(self.saved, libc::STDOUT_FILENO);
            libc::close(self.saved);
        }
    }
}
