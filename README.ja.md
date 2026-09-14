<p align="center">
  <a href="README.md">English</a> · <b>日本語</b>
</p>

# gh-yard

リポジトリを fuzzy 検索で選び、そのパスを標準出力に出す gh extension。ghq + fzf の使い方を単一バイナリにまとめたものです。

## インストール

```
gh extension install 22mb/gh-yard
```

前提は [gh](https://cli.github.com/) と git のみ。対応プラットフォームは macOS / Linux (amd64 / arm64)。ビルド済みバイナリが入るので、Rust 環境は不要です。

## 使い方

| コマンド | 動作 |
|---|---|
| `gh yard [-q <query>]` | セレクタを開き、選んだリポジトリの絶対パスを出力する。`-q` / `--query` でクエリを渡すと絞り込んだ状態で開き、一致が 1 件だけならセレクタを開かずにそのパスを出力する |
| `gh yard list [-p]` | リポジトリを 1 行 1 件で出力する。`-p` / `--full-path` で絶対パス |
| `gh yard get <spec>` | クローンし、そのパスを出力する |
| `gh yard create <spec>` | ローカルにリポジトリを作成 (`git init`) し、そのパスを出力する |
| `gh yard root` | ルートディレクトリを出力する |

`<spec>` は `owner/repo`（ホストは github.com）、`host/owner/repo`、URL (`https://` / `ssh://` / `git@host:owner/repo`) の 3 形式。GitLab のサブグループのような深い階層もそのまま扱えます。

cd やエディタ起動は持ちません。パスを受け取って自分で組みます。

```fish
# fish
function d
    set -l d (gh yard -q "$argv"); and cd $d
end
function c
    set -l d (gh yard -q "$argv"); and code $d
end
abbr gg 'gh yard get'
```

```zsh
# zsh / bash
d() { local d; d=$(gh yard -q "$*") && cd "$d"; }
c() { local d; d=$(gh yard -q "$*") && code "$d"; }
```

`d` だけならセレクタが開きます。`d zod` は `zod` に一致するリポジトリが 1 件ならそこへ直接移動し、複数あれば `zod` で絞り込んだセレクタが開きます。

いったん変数で受けるのは、中止したときに何も起きないようにするためです。`cd (gh yard)` と直接書くと、中止して出力が空になったときに引数なしの `cd` と同じ扱いになり、ホームディレクトリへ移動してしまいます。

## セレクタのキー操作

| キー | 動作 |
|---|---|
| `Enter` | 確定 |
| `Esc` / `Ctrl-C` | 中止 |
| `↑↓` / `Ctrl-P` / `Ctrl-N` / `Ctrl-K` / `Ctrl-J` | 候補の移動 |
| `←→` / `Ctrl-B` / `Ctrl-F` | 入力カーソルの移動 |
| `Ctrl-A` / `Ctrl-E` | 行頭 / 行末 |
| `Backspace` | カーソル前の 1 文字を削除 |
| `Del` / `Ctrl-D` | カーソル位置の 1 文字を削除 |
| `Ctrl-U` | カーソルより前をすべて削除 |
| `Ctrl-W` | カーソル前の単語を削除 |

## ルートディレクトリ

`YARD_ROOT` → `git config yard.root` → `~/yard` の順で解決します。レイアウトは `root/host/owner/repo`。

既存の ghq の配下をそのまま使う場合は、ルートを指すだけで済みます。

```
git config --global yard.root ~/ghq
```

## 終了コード

| コード | 意味 |
|---|---|
| 0 | 正常終了 |
| 1 | セレクタの中止、または対象が 0 件 |
| 2 | エラー |

## 開発

CI は pull request ごとに次のチェックを実行します。push する前に手元でも通しておきます。

```
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
```

手元のビルドを gh extension として使うには、release バイナリへのシンボリックリンクをリポジトリ直下に置き、そこからローカルインストールします。

```
cargo build --release
ln -s target/release/gh-yard gh-yard
gh extension install .
```

以降は `cargo build --release` だけで反映されます。リンクはビルドし直したバイナリを指し続けます。

## 謝辞

gh-yard は、[ghq](https://github.com/x-motemen/ghq) と [fzf](https://github.com/junegunn/fzf) が確立した使い方をそのまま受け継いでいます。リポジトリを決まった配置に置き、fuzzy 検索で選ぶ使い方です。毎日使ってきた 2 つのプロジェクトに感謝します。
