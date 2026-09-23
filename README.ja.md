<p align="center">
  <a href="README.md">English</a> · <b>日本語</b>
</p>

# gh-yard

リポジトリを fuzzy 検索で選択し、そのパスを標準出力に書き出す gh extension です。ghq と fzf を組み合わせた使い方を、単一のバイナリにまとめています。

## インストール

```
gh extension install 22mb/gh-yard
```

前提となるのは [gh](https://cli.github.com/) と git だけです。対応プラットフォームは macOS と Linux (amd64 / arm64) です。ビルド済みのバイナリがインストールされるため、Rust の環境は必要ありません。

## 使い方

| コマンド | 動作 |
|---|---|
| `gh yard [-q <query>]` | セレクタを開き、選択したリポジトリの絶対パスを出力。`-q` / `--query` でクエリを渡すと絞り込んだ状態でセレクタを開き、一致が 1 件だけのときはセレクタを開かずにそのパスを出力 |
| `gh yard list [-p]` | リポジトリを 1 行に 1 件ずつ出力。`-p` / `--full-path` を指定すると絶対パスで出力 |
| `gh yard get <spec>` | リポジトリをクローンし、そのパスを出力 |
| `gh yard create <spec>` | ローカルにリポジトリを作成 (`git init`) し、そのパスを出力 |
| `gh yard root` | ルートディレクトリを出力 |

`<spec>` には、`owner/repo` (ホストは github.com)、`host/owner/repo`、URL (`https://` / `ssh://` / `git@host:owner/repo`) の 3 形式を指定できます。GitLab のサブグループのような深い階層も、そのまま扱えます。

gh yard 自体は cd やエディタ起動の機能を持たないため、出力されたパスを受け取り、ユーザー側で次のように組み立てます。

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

引数なしで `d` を実行すると、セレクタが開きます。`d zod` を実行すると、`zod` に一致するリポジトリが 1 件のときはそのディレクトリへ直接移動し、複数あるときは `zod` で絞り込んだ状態のセレクタが開きます。

出力をいったん変数で受け取るのは、セレクタを中止したときに何も起きないようにするためです。`cd (gh yard)` と直接書くと、中止して出力が空になった際に引数なしの `cd` と同じ扱いになり、ホームディレクトリへ移動してしまいます。

## セレクタのキー操作

| キー | 動作 |
|---|---|
| `Enter` | 選択の確定 |
| `Esc` / `Ctrl-C` | 中止 |
| `↑↓` / `Ctrl-P` / `Ctrl-N` / `Ctrl-K` / `Ctrl-J` | 候補の移動 |
| `←→` / `Ctrl-B` / `Ctrl-F` | 入力カーソルの移動 |
| `Ctrl-A` / `Ctrl-E` | 行頭・行末への移動 |
| `Backspace` | カーソルの直前の 1 文字を削除 |
| `Del` / `Ctrl-D` | カーソル位置の 1 文字を削除 |
| `Ctrl-U` | カーソルより前の文字をすべて削除 |
| `Ctrl-W` | カーソルの直前の単語を削除 |

## ルートディレクトリ

ルートディレクトリは、「`YARD_ROOT` 環境変数 → `git config yard.root` → `~/yard`」の順で決定されます。リポジトリは `root/host/owner/repo` の構成で配置されます。

ghq で管理している既存のディレクトリをそのまま使うときは、次のようにルートをそのディレクトリに設定するだけで済みます。

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

CI では、pull request ごとに次のチェックを実行します。push する前に、手元の環境でもこれらのチェックを通しておいてください。

```
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
```

手元でビルドしたバイナリを gh extension として使うには、release ビルドのバイナリへのシンボリックリンクをリポジトリの直下に作成し、そのディレクトリからローカルインストールします。

```
cargo build --release
ln -s target/release/gh-yard gh-yard
gh extension install .
```

シンボリックリンクは再ビルドしたバイナリを指し続けるため、以降は `cargo build --release` を実行するだけで変更が反映されます。

## 謝辞

gh-yard は、[ghq](https://github.com/x-motemen/ghq) と [fzf](https://github.com/junegunn/fzf) が確立した使い方をそのまま受け継いでいます。それは、リポジトリを決まった構成で配置し、fuzzy 検索で選択するという使い方です。毎日使ってきたこの 2 つのプロジェクトに感謝します。
