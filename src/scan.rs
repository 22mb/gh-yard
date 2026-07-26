use std::fs;
use std::path::{Path, PathBuf};

pub struct Repo {
    /// Absolute path.
    pub abs: PathBuf,
    /// Path relative to the root (`host/owner/repo`).
    pub rel: String,
}

/// Collects directories containing `.git` under the root.
///
/// - Does not follow symlinks
/// - Silently skips unreadable directories
/// - Does not descend into repositories
/// - Returns entries sorted by relative path
pub fn scan(root: &Path) -> Vec<Repo> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let mut subdirs = Vec::new();
        let mut is_repo = false;

        for entry in entries.flatten() {
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            // file_type does not follow links, so symlinks drop out here.
            if !file_type.is_dir() {
                continue;
            }
            if entry.file_name() == ".git" {
                is_repo = true;
                break;
            }
            subdirs.push(entry.path());
        }

        if is_repo {
            if let Some(rel) = relative(root, &dir) {
                found.push(Repo { abs: dir, rel });
            }
            continue; // do not look inside the repository
        }
        stack.extend(subdirs);
    }

    found.sort_by(|a, b| a.rel.cmp(&b.rel));
    found
}

fn relative(root: &Path, dir: &Path) -> Option<String> {
    let rel = dir.strip_prefix(root).ok()?;
    let s = rel.to_string_lossy().into_owned();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Creates a repository (a directory containing `.git`) under `root`.
    fn mkrepo(root: &Path, rel: &str) {
        fs::create_dir_all(root.join(rel).join(".git")).unwrap();
    }

    fn rels(root: &Path) -> Vec<String> {
        scan(root).into_iter().map(|r| r.rel).collect()
    }

    #[test]
    fn finds_repos_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        mkrepo(tmp.path(), "github.com/foo/zebra");
        mkrepo(tmp.path(), "github.com/foo/apple");
        mkrepo(tmp.path(), "gitlab.com/bar/baz");
        assert_eq!(
            rels(tmp.path()),
            [
                "github.com/foo/apple",
                "github.com/foo/zebra",
                "gitlab.com/bar/baz"
            ]
        );
    }

    #[test]
    fn abs_is_root_joined_rel() {
        let tmp = tempfile::tempdir().unwrap();
        mkrepo(tmp.path(), "github.com/foo/bar");
        let repos = scan(tmp.path());
        assert_eq!(repos[0].abs, tmp.path().join("github.com/foo/bar"));
    }

    #[test]
    fn detects_deeper_subgroups() {
        let tmp = tempfile::tempdir().unwrap();
        mkrepo(tmp.path(), "gitlab.com/group/sub/repo");
        assert_eq!(rels(tmp.path()), ["gitlab.com/group/sub/repo"]);
    }

    #[test]
    fn does_not_descend_into_repos() {
        let tmp = tempfile::tempdir().unwrap();
        mkrepo(tmp.path(), "github.com/foo/outer");
        // A repository inside a repository (vendor / submodule) stays hidden.
        mkrepo(tmp.path(), "github.com/foo/outer/vendor/inner");
        assert_eq!(rels(tmp.path()), ["github.com/foo/outer"]);
    }

    #[test]
    fn ignores_dirs_without_dot_git() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("github.com/foo/not-a-repo")).unwrap();
        assert!(rels(tmp.path()).is_empty());
    }

    #[test]
    fn does_not_follow_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tempfile::tempdir().unwrap();
        mkrepo(real.path(), "github.com/foo/linked");
        std::os::unix::fs::symlink(real.path(), tmp.path().join("link")).unwrap();
        assert!(rels(tmp.path()).is_empty());
    }

    #[test]
    fn skips_unreadable_dirs() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        mkrepo(tmp.path(), "github.com/foo/visible");
        let locked = tmp.path().join("locked.example");
        fs::create_dir_all(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        let result = rels(tmp.path());

        // Restore permissions so the tempdir can be removed.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(result, ["github.com/foo/visible"]);
    }

    #[test]
    fn missing_root_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("no-such-dir");
        assert!(scan(&missing).is_empty());
    }
}
