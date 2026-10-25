//! Runs the built binary and verifies the output and exit-code contract.
//!
//! Branches that depend on environment variables (YARD_ROOT, HOME, git
//! config) interfere with each other unless each case runs in its own
//! process, so they are covered here rather than in unit tests. The
//! interactive TUI needs a terminal and is out of scope (only the
//! zero-repository early exit is checked).

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_gh-yard");

/// Runs the binary with HOME and git config isolated so the real
/// environment cannot leak in.
fn run(home: &Path, yard_root: Option<&Path>, args: &[&str]) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.args(args)
        .env("HOME", home)
        .env("GIT_CONFIG_GLOBAL", home.join("gitconfig-test"))
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("YARD_ROOT");
    if let Some(root) = yard_root {
        cmd.env("YARD_ROOT", root);
    }
    cmd.output().expect("failed to run binary")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn mkrepo(root: &Path, rel: &str) {
    fs::create_dir_all(root.join(rel).join(".git")).unwrap();
}

// ---- top level ----

#[test]
fn version_prints_package_version() {
    let tmp = tempfile::tempdir().unwrap();
    for flag in ["--version", "-V"] {
        let out = run(tmp.path(), Some(tmp.path()), &[flag]);
        assert_eq!(out.status.code(), Some(0), "{flag}");
        assert_eq!(
            stdout(&out),
            format!("gh-yard {}\n", env!("CARGO_PKG_VERSION")),
            "{flag}"
        );
    }
}

#[test]
fn top_level_help_prints_usage() {
    let tmp = tempfile::tempdir().unwrap();
    for arg in ["-h", "--help", "help"] {
        let out = run(tmp.path(), Some(tmp.path()), &[arg]);
        assert_eq!(out.status.code(), Some(0), "{arg}");
        assert!(stdout(&out).contains("Usage:"), "{arg}");
    }
}

#[test]
fn unknown_subcommand_is_error_with_usage() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["frobnicate"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stdout(&out), "");
    assert!(stderr(&out).contains("unknown subcommand: frobnicate"));
    assert!(stderr(&out).contains("Usage:"));
}

#[test]
fn query_without_a_value_or_with_two_is_error() {
    // Both fail before the selector could open a terminal.
    let tmp = tempfile::tempdir().unwrap();
    for args in [&["-q"][..], &["--query"], &["-q", "a", "b"]] {
        let out = run(tmp.path(), Some(tmp.path()), args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert_eq!(stdout(&out), "", "{args:?}");
    }
}

// ---- list ----

#[test]
fn list_outputs_rel_sorted() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    mkrepo(&root, "github.com/foo/zebra");
    mkrepo(&root, "github.com/foo/apple");
    mkrepo(&root, "gitlab.com/group/sub/repo");

    let out = run(tmp.path(), Some(&root), &["list"]);
    assert!(out.status.success());
    assert_eq!(
        stdout(&out),
        "github.com/foo/apple\ngithub.com/foo/zebra\ngitlab.com/group/sub/repo\n"
    );
}

#[test]
fn list_full_path_outputs_abs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    mkrepo(&root, "github.com/foo/bar");

    for flag in ["-p", "--full-path"] {
        let out = run(tmp.path(), Some(&root), &["list", flag]);
        assert!(out.status.success());
        let line = stdout(&out);
        assert_eq!(
            line.trim_end(),
            root.join("github.com/foo/bar").to_str().unwrap()
        );
        assert!(Path::new(line.trim_end()).is_absolute());
    }
}

#[test]
fn list_empty_root_is_ok_and_silent() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("no-such-root");

    let out = run(tmp.path(), Some(&root), &["list"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(stdout(&out), "");
}

#[test]
fn list_closed_pipe_exits_0_without_panicking() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    // Well over the 64 KiB pipe buffer, so the process is still writing when
    // the read end goes away; a smaller listing would finish before that.
    for i in 0..1000 {
        mkrepo(
            &root,
            &format!("github.com/owner-with-a-long-name-{i}/repository-with-a-long-name-{i}"),
        );
    }

    let mut child = Command::new(BIN)
        .arg("list")
        .env("HOME", tmp.path())
        .env("GIT_CONFIG_GLOBAL", tmp.path().join("gitconfig-test"))
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("YARD_ROOT", &root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Close the read end without reading, as `head` does once it has enough.
    drop(child.stdout.take());
    let mut err = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    let status = child.wait().unwrap();

    assert_eq!(status.code(), Some(0), "stderr: {err}");
    assert!(!err.contains("panicked"), "stderr: {err}");
}

#[test]
fn list_unknown_flag_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["list", "--bogus"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--bogus"));
}

#[test]
fn subcommand_help_prints_usage() {
    let tmp = tempfile::tempdir().unwrap();
    for sub in ["list", "get", "create", "root"] {
        for flag in ["-h", "--help"] {
            let out = run(tmp.path(), Some(tmp.path()), &[sub, flag]);
            assert_eq!(out.status.code(), Some(0), "{sub} {flag}");
            assert!(stdout(&out).contains("Usage:"), "{sub} {flag}");
        }
    }
}

// ---- selector (zero-repository early exit only) ----

#[test]
fn selector_with_zero_repos_exits_1() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("empty");
    fs::create_dir_all(&root).unwrap();

    let out = run(tmp.path(), Some(&root), &[]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stdout(&out), "");
    assert!(stderr(&out).contains("no repositories"));
}

// ---- root ----

#[test]
fn root_uses_yard_root_env() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("custom");

    let out = run(tmp.path(), Some(&root), &["root"]);
    assert!(out.status.success());
    assert_eq!(stdout(&out).trim_end(), root.to_str().unwrap());
}

#[test]
fn root_expands_tilde_in_env() {
    let tmp = tempfile::tempdir().unwrap();

    let mut cmd = Command::new(BIN);
    let out = cmd
        .arg("root")
        .env("HOME", tmp.path())
        .env("GIT_CONFIG_GLOBAL", tmp.path().join("gitconfig-test"))
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("YARD_ROOT", "~/repos")
        .output()
        .unwrap();
    assert_eq!(
        stdout(&out).trim_end(),
        tmp.path().join("repos").to_str().unwrap()
    );
}

#[test]
fn root_tilde_alone_is_home() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(BIN)
        .arg("root")
        .env("HOME", tmp.path())
        .env("GIT_CONFIG_GLOBAL", tmp.path().join("gitconfig-test"))
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("YARD_ROOT", "~")
        .output()
        .unwrap();
    assert_eq!(stdout(&out).trim_end(), tmp.path().to_str().unwrap());
}

#[test]
fn root_folds_dotdot_in_a_relative_env_path() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(BIN)
        .arg("root")
        .current_dir(tmp.path())
        .env("HOME", tmp.path())
        .env("GIT_CONFIG_GLOBAL", tmp.path().join("gitconfig-test"))
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("YARD_ROOT", "sub/../rel")
        .output()
        .unwrap();
    let printed = stdout(&out);
    let printed = Path::new(printed.trim_end());
    assert_eq!(printed.file_name().unwrap(), "rel");
    assert_eq!(
        printed.parent().unwrap().canonicalize().unwrap(),
        tmp.path().canonicalize().unwrap()
    );
}

#[test]
fn root_without_home_and_config_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(BIN)
        .arg("root")
        .env_remove("HOME")
        .env_remove("YARD_ROOT")
        .env("GIT_CONFIG_GLOBAL", tmp.path().join("gitconfig-test"))
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stdout(&out), "");
    assert!(stderr(&out).contains("home directory"));
}

#[test]
fn root_defaults_to_home_yard() {
    let tmp = tempfile::tempdir().unwrap();

    let out = run(tmp.path(), None, &["root"]);
    assert!(out.status.success());
    assert_eq!(
        stdout(&out).trim_end(),
        tmp.path().join("yard").to_str().unwrap()
    );
}

#[test]
fn root_reads_git_config_yard_root() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("gitconfig-test"),
        "[yard]\n\troot = ~/from-config\n",
    )
    .unwrap();

    let out = run(tmp.path(), None, &["root"]);
    assert!(out.status.success());
    // git's --type=path expands `~`.
    assert_eq!(
        stdout(&out).trim_end(),
        tmp.path().join("from-config").to_str().unwrap()
    );
}

#[test]
fn root_normalizes_relative_config_path() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("gitconfig-test"),
        "[yard]\n\troot = rel-root\n",
    )
    .unwrap();

    let mut cmd = Command::new(BIN);
    let out = cmd
        .arg("root")
        .current_dir(tmp.path())
        .env("HOME", tmp.path())
        .env("GIT_CONFIG_GLOBAL", tmp.path().join("gitconfig-test"))
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("YARD_ROOT")
        .output()
        .unwrap();
    assert!(out.status.success());
    let printed = stdout(&out);
    let printed = Path::new(printed.trim_end());
    assert!(printed.is_absolute());
    // Compare canonicalized: the tempdir path may itself contain symlinks.
    assert_eq!(
        printed.parent().unwrap().canonicalize().unwrap(),
        tmp.path().canonicalize().unwrap()
    );
    assert_eq!(printed.file_name().unwrap(), "rel-root");
}

#[test]
fn empty_yard_root_falls_back_to_git_config() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("gitconfig-test"),
        "[yard]\n\troot = ~/from-config\n",
    )
    .unwrap();

    let mut cmd = Command::new(BIN);
    let out = cmd
        .arg("root")
        .env("HOME", tmp.path())
        .env("GIT_CONFIG_GLOBAL", tmp.path().join("gitconfig-test"))
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("YARD_ROOT", "")
        .output()
        .unwrap();
    assert_eq!(
        stdout(&out).trim_end(),
        tmp.path().join("from-config").to_str().unwrap()
    );
}

#[test]
fn yard_root_env_wins_over_git_config() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("gitconfig-test"),
        "[yard]\n\troot = ~/from-config\n",
    )
    .unwrap();
    let root = tmp.path().join("from-env");

    let out = run(tmp.path(), Some(&root), &["root"]);
    assert_eq!(stdout(&out).trim_end(), root.to_str().unwrap());
}

// ---- get ----

#[test]
fn get_existing_repo_prints_path_without_cloning() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    mkrepo(&root, "github.com/cli/cli");

    let out = run(tmp.path(), Some(&root), &["get", "cli/cli"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        stdout(&out).trim_end(),
        root.join("github.com/cli/cli").to_str().unwrap()
    );
}

#[test]
fn get_existing_repo_accepts_url_and_scp_specs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    mkrepo(&root, "github.com/cli/cli");
    mkrepo(&root, "gitlab.com/group/sub/repo");

    for (spec, rel) in [
        ("https://github.com/cli/cli", "github.com/cli/cli"),
        ("https://github.com/cli/cli.git", "github.com/cli/cli"),
        ("git@github.com:cli/cli.git", "github.com/cli/cli"),
        ("gitlab.com/group/sub/repo", "gitlab.com/group/sub/repo"),
        (
            "ssh://git@gitlab.com/group/sub/repo.git",
            "gitlab.com/group/sub/repo",
        ),
    ] {
        let out = run(tmp.path(), Some(&root), &["get", spec]);
        assert_eq!(out.status.code(), Some(0), "spec: {spec}");
        assert_eq!(
            stdout(&out).trim_end(),
            root.join(rel).to_str().unwrap(),
            "spec: {spec}"
        );
    }
}

#[test]
fn get_existing_worktree_prints_path_without_cloning() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let wt = root.join("github.com/cli/wt");
    fs::create_dir_all(&wt).unwrap();
    // A worktree keeps its .git as a file; scan and get treat it alike.
    fs::write(wt.join(".git"), "gitdir: /elsewhere/.git/worktrees/wt\n").unwrap();

    let out = run(tmp.path(), Some(&root), &["get", "cli/wt"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out).trim_end(), wt.to_str().unwrap());
}

#[test]
fn get_directory_without_dot_git_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    fs::create_dir_all(root.join("github.com/cli/cli")).unwrap();

    let out = run(tmp.path(), Some(&root), &["get", "cli/cli"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stdout(&out), "");
    assert!(stderr(&out).contains("not a repository"));
}

#[test]
fn get_dotless_three_parts_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["get", "foo/bar/baz"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stdout(&out), "");
}

#[test]
fn get_without_spec_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["get"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn get_with_two_specs_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["get", "a/b", "c/d"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn get_leading_dash_component_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["get", "-foo/bar"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stdout(&out), "");
}

// ---- create ----

#[test]
fn create_inits_repo_and_prints_path() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");

    let out = run(tmp.path(), Some(&root), &["create", "22mb/newrepo"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let dest = root.join("github.com/22mb/newrepo");
    assert_eq!(stdout(&out).trim_end(), dest.to_str().unwrap());
    assert!(dest.join(".git").is_dir());
}

#[test]
fn create_appears_in_list() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");

    run(tmp.path(), Some(&root), &["create", "22mb/newrepo"]);
    let out = run(tmp.path(), Some(&root), &["list"]);
    assert_eq!(stdout(&out), "github.com/22mb/newrepo\n");
}

#[test]
fn create_accepts_url_and_scp_specs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    for (spec, rel) in [
        (
            "https://gitlab.com/group/sub/repo.git",
            "gitlab.com/group/sub/repo",
        ),
        ("git@github.com:22mb/scp.git", "github.com/22mb/scp"),
    ] {
        let out = run(tmp.path(), Some(&root), &["create", spec]);
        assert_eq!(out.status.code(), Some(0), "{spec}: {}", stderr(&out));
        assert_eq!(stdout(&out).trim_end(), root.join(rel).to_str().unwrap());
        assert!(root.join(rel).join(".git").is_dir(), "{spec}");
    }
}

#[test]
fn create_leading_dash_component_is_error_and_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    fs::create_dir_all(&root).unwrap();
    let out = run(tmp.path(), Some(&root), &["create", "-foo/bar"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stdout(&out), "");
    assert!(!root.join("github.com").exists());
}

#[test]
fn create_with_two_specs_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["create", "a/b", "c/d"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn create_existing_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    mkrepo(&root, "github.com/22mb/taken");

    let out = run(tmp.path(), Some(&root), &["create", "22mb/taken"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stdout(&out), "");
    assert!(stderr(&out).contains("already exists"));
}

#[test]
fn create_dotless_three_parts_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["create", "foo/bar/baz"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn create_with_dotdot_component_is_error_and_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    fs::create_dir_all(&root).unwrap();

    // `..` as the first of three or more components used to pass as a host,
    // so the destination landed outside the root.
    let out = run(tmp.path(), Some(&root), &["create", "../escaped/x/y"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stdout(&out), "");
    assert!(!tmp.path().join("escaped").exists());
}

#[test]
fn create_without_spec_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["create"]);
    assert_eq!(out.status.code(), Some(2));
}

// ---- query ----
//
// Only queries that narrow the list to one repository are covered: that
// path answers before the selector opens /dev/tty. Any other query would
// open the selector, which needs a terminal.

#[test]
fn query_with_single_match_prints_path_without_terminal() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    mkrepo(&root, "github.com/foo/zod");
    mkrepo(&root, "github.com/foo/apple");

    let out = run(tmp.path(), Some(&root), &["-q", "zod"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_eq!(
        stdout(&out).trim_end(),
        root.join("github.com/foo/zod").to_str().unwrap()
    );
}

#[test]
fn query_value_may_contain_spaces() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    mkrepo(&root, "github.com/foo/zod");
    mkrepo(&root, "github.com/bar/zod");

    // "bar zod" narrows to one repository; "zod" alone would match both.
    let out = run(tmp.path(), Some(&root), &["--query", "bar zod"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_eq!(
        stdout(&out).trim_end(),
        root.join("github.com/bar/zod").to_str().unwrap()
    );
}

#[test]
fn query_with_no_repositories_exits_1() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("empty");
    fs::create_dir_all(&root).unwrap();

    let out = run(tmp.path(), Some(&root), &["-q", "zod"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stdout(&out), "");
}

#[test]
fn query_flag_takes_exactly_one_value() {
    let tmp = tempfile::tempdir().unwrap();
    for args in [&["-q"][..], &["-q", "a", "b"][..]] {
        let out = run(tmp.path(), Some(tmp.path()), args);
        assert_eq!(out.status.code(), Some(2), "args: {args:?}");
        assert_eq!(stdout(&out), "", "args: {args:?}");
    }
}

// ---- misc ----

#[test]
fn unknown_subcommand_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    // A bare word is not a query (that is `-q`), so it stays an error.
    for arg in ["bogus", "--bogus"] {
        let out = run(tmp.path(), Some(tmp.path()), &[arg]);
        assert_eq!(out.status.code(), Some(2), "arg: {arg}");
        assert!(stderr(&out).contains(arg), "arg: {arg}");
    }
}

#[test]
fn help_and_version_exit_0() {
    let tmp = tempfile::tempdir().unwrap();
    for arg in ["--help", "--version"] {
        let out = run(tmp.path(), Some(tmp.path()), &[arg]);
        assert_eq!(out.status.code(), Some(0), "arg: {arg}");
        assert!(!stdout(&out).is_empty());
    }
}
