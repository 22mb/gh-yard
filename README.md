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
| `gh yard` | Open the selector and print the chosen repository's absolute path |
| `gh yard list [-p]` | Print repositories, one per line. `-p` / `--full-path` for absolute paths |
| `gh yard get <spec>` | Clone and print the path |
| `gh yard create <spec>` | Create a local repository (`git init`) and print the path |
| `gh yard root` | Print the root directory |

`<spec>` takes three forms: `owner/repo` (host defaults to github.com), `host/owner/repo`, or a URL (`https://` / `ssh://` / `git@host:owner/repo`). Deep hierarchies such as GitLab subgroups work as-is.

gh-yard does not cd or launch an editor. It prints a path; you wire it up yourself.

```fish
# fish
abbr d 'set -l d (gh yard); and cd $d'
abbr c 'set -l d (gh yard); and code $d'
abbr gg 'gh yard get'
```

```zsh
# zsh / bash
alias d='d=$(gh yard) && cd "$d"'
alias c='d=$(gh yard) && code "$d"'
```

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

```
cargo test
cargo build --release
```

To use a local build as the gh extension, place the binary at the repository root and install from there:

```
cargo build --release && cp target/release/gh-yard .
gh extension install .
```

After that, `cargo build --release && cp target/release/gh-yard .` is all it takes to pick up changes.
