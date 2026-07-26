use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Resolves the root directory in the order
/// `YARD_ROOT` → `git config yard.root` → `~/yard`.
/// A path that does not exist is still returned as resolved.
pub fn resolve() -> Result<PathBuf, String> {
    if let Some(v) = env::var_os("YARD_ROOT") {
        let s = v.to_string_lossy().into_owned();
        if !s.is_empty() {
            return normalize(&s);
        }
    }

    if let Some(s) = git_config_yard_root() {
        return normalize(&s);
    }

    let home = home_dir().ok_or_else(|| "cannot determine home directory".to_string())?;
    Ok(home.join("yard"))
}

fn git_config_yard_root() -> Option<String> {
    // With --type=path, git expands `~` for us.
    let out = Command::new("git")
        .args(["config", "--get", "--type=path", "yard.root"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Expands `~` and turns a relative path into an absolute one.
fn normalize(s: &str) -> Result<PathBuf, String> {
    let expanded = if s == "~" {
        home_dir().ok_or_else(|| "cannot determine home directory".to_string())?
    } else if let Some(rest) = s.strip_prefix("~/") {
        home_dir()
            .ok_or_else(|| "cannot determine home directory".to_string())?
            .join(rest)
    } else {
        PathBuf::from(s)
    };

    if expanded.is_absolute() {
        return Ok(expanded);
    }
    let cwd = env::current_dir().map_err(|e| format!("cannot determine current directory: {e}"))?;
    Ok(clean(&cwd.join(expanded)))
}

/// Folds `.` and `..` textually so the path is usable even when it does not exist.
fn clean(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Branches that depend on environment variables (YARD_ROOT, HOME) are
    // covered by tests/cli.rs in separate processes. Only the pure parts
    // are tested here.

    #[test]
    fn clean_folds_dot_and_dotdot() {
        assert_eq!(clean(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
    }

    #[test]
    fn clean_keeps_plain_path() {
        assert_eq!(clean(Path::new("/a/b/c")), PathBuf::from("/a/b/c"));
    }

    #[test]
    fn clean_does_not_escape_root() {
        assert_eq!(clean(Path::new("/../../a")), PathBuf::from("/a"));
    }
}
