<p align="center">
  <b>English</b> · <a href="README.ja.md">日本語</a>
</p>

# gh-yard

A gh extension that picks a repository with fuzzy search and prints its path to stdout. Replaces ghq + fzf with a single binary.

## Installation

```
gh extension install 22mb/gh-yard
```

Requires only [gh](https://cli.github.com/) and git. Supported platforms: macOS / Linux (amd64 / arm64). A prebuilt binary is installed — no Rust toolchain needed.

## Usage

| Command | What it does |
|---|---|
| `gh yard [query]` | Open the selector and print the chosen repository's absolute path. With a query the selector starts filtered, and if exactly one repository matches, its path is printed without opening the selector |
| `gh yard list [-p]` | Print repositories, one per line. `-p` / `--full-path` for absolute paths |
| `gh yard get <spec>` | Clone and print the path |
| `gh yard create <spec>` | Create a local repository (`git init`) and print the path |
| `gh yard root` | Print the root directory |

`<spec>` takes three forms: `owner/repo` (host defaults to github.com), `host/owner/repo`, or a URL (`https://` / `ssh://` / `git@host:owner/repo`). Deep hierarchies such as GitLab subgroups work as-is.

gh-yard does not cd or launch an editor. It prints a path; you wire it up yourself.

```fish
# fish
function d
    set -l d (gh yard $argv); and cd $d
end
function c
    set -l d (gh yard $argv); and code $d
end
abbr gg 'gh yard get'
```

```zsh
# zsh / bash
d() { local d; d=$(gh yard "$@") && cd "$d"; }
c() { local d; d=$(gh yard "$@") && code "$d"; }
```

`d` alone opens the selector. `d zod` jumps straight to the only repository matching `zod`, or opens the selector filtered to `zod` when there are several.

Capturing into a variable first keeps an abort from doing anything. Writing `cd (gh yard)` directly would run a bare `cd` on abort — the output is empty — and drop you in your home directory.

## Selector keys

| Key | Action |
|---|---|
| `Enter` | Accept |
| `Esc` / `Ctrl-C` | Abort |
| `↑↓` / `Ctrl-P` / `Ctrl-N` / `Ctrl-K` / `Ctrl-J` | Move through candidates |
| `←→` / `Ctrl-B` / `Ctrl-F` | Move the input cursor |
| `Ctrl-A` / `Ctrl-E` | Start / end of line |
| `Backspace` | Delete the char before the cursor |
| `Del` / `Ctrl-D` | Delete the char at the cursor |
| `Ctrl-U` | Delete everything before the cursor |
| `Ctrl-W` | Delete the word before the cursor |

## Root directory

Resolved in the order `YARD_ROOT` → `git config yard.root` → `~/yard`. The layout is `root/host/owner/repo`.

To keep using an existing ghq tree, just point the root at it:

```
git config --global yard.root ~/ghq
```

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Selector aborted, or nothing found |
| 2 | Error |

## Development

CI runs these checks on every pull request; run them before pushing:

```
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
```

To use a local build as the gh extension, link the release binary into the repository root and install from there:

```
cargo build --release
ln -s target/release/gh-yard gh-yard
gh extension install .
```

After that, `cargo build --release` alone picks up changes: the link keeps pointing at the freshly built binary.
