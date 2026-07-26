use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::spec::Target;

/// Returns the clone destination. Does not clone when it already exists.
pub fn run(root: &Path, target: &Target) -> Result<PathBuf, String> {
    let mut dest = root.join(&target.host);
    for part in &target.path {
        dest.push(part);
    }

    if dest.exists() {
        return Ok(dest);
    }

    let parent = dest
        .parent()
        .ok_or_else(|| format!("invalid clone destination: {}", dest.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;

    let status = if target.is_github() {
        // Leave authentication and protocol selection to gh.
        Command::new("gh")
            .arg("repo")
            .arg("clone")
            .arg(target.path_str())
            .arg(&dest)
            .status()
    } else {
        // For other hosts, build an HTTPS URL and hand it to git
        // (authentication is left to git's credential helper).
        Command::new("git")
            .arg("clone")
            .arg(target.https_url())
            .arg(&dest)
            .status()
    };

    match status {
        Ok(status) if status.success() => Ok(dest),
        Ok(status) => Err(format!(
            "clone failed (exit code {})",
            status.code().unwrap_or(-1)
        )),
        Err(e) => Err(format!("cannot run clone command: {e}")),
    }
}
