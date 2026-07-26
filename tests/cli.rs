//! Runs the built binary and verifies the output and exit-code contract.
//!
//! Branches that depend on environment variables (YARD_ROOT, HOME, git
//! config) interfere with each other unless each case runs in its own
//! process, so they are covered here rather than in unit tests. The
//! interactive TUI needs a terminal and is out of scope (only the
//! zero-repository early exit is checked).

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

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
fn list_unknown_flag_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["list", "--bogus"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--bogus"));
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
fn create_without_spec_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["create"]);
    assert_eq!(out.status.code(), Some(2));
}

// ---- misc ----

#[test]
fn unknown_subcommand_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), Some(tmp.path()), &["bogus"]);
    assert_eq!(out.status.code(), Some(2));
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
