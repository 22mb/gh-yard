use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::spec::Target;

/// Creates `root/host/...` and runs `git init` there. Nothing is created
/// remotely. Unlike `get`, an existing destination is an error: silently
/// succeeding would hide typos in a "create something new" operation.
pub fn run(root: &Path, target: &Target) -> Result<PathBuf, String> {
    let mut dest = root.join(&target.host);
    for part in &target.path {
        dest.push(part);
    }

    if dest.exists() {
        return Err(format!("already exists: {}", dest.display()));
    }

    fs::create_dir_all(&dest).map_err(|e| format!("cannot create {}: {e}", dest.display()))?;

    // The default branch name follows git's own config (init.defaultBranch).
    // --quiet: git init prints its success message to stdout, which would
    // pollute the path-only stdout contract. Errors still go to stderr.
    let status = Command::new("git")
        .arg("init")
        .arg("--quiet")
        .arg(&dest)
        .status()
        .map_err(|e| format!("cannot run git init: {e}"))?;

    if !status.success() {
        // Remove what we created; otherwise a retry would hit the misleading
        // "already exists" error on a half-made directory.
        let _ = fs::remove_dir_all(&dest);
        return Err(format!(
            "git init failed (exit code {})",
            status.code().unwrap_or(-1)
        ));
    }
    Ok(dest)
}
