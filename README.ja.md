<p align="center">
  <a href="README.md">English</a> · <b>日本語</b>
</p>

# gh-yard

リポジトリをファジー検索で選択し、そのパスを標準出力に書き出す gh 拡張機能です。ghq と fzf を組み合わせたワークフローを単一のバイナリにまとめています。

## インストール

```
gh extension install 22mb/gh-yard
```

必要なのは [gh](https://cli.github.com/) と git だけです。対応プラットフォームは macOS と Linux (amd64 / arm64) であり、ビルド済みのバイナリがインストールされるため、Rust のツールチェーンは必要ありません。

## 使い方

| コマンド | 動作 |
|---|---|
| `gh yard [-q <query>]` | セレクタを開き、選択したリポジトリの絶対パスを出力。`-q` / `--query` でクエリを渡すと絞り込んだ状態でセレクタを開き、一致が 1 件だけのときはセレクタを開かずにそのパスを出力 |
| `gh yard list [-p]` | リポジトリを 1 行に 1 件ずつ出力。`-p` / `--full-path` を指定すると絶対パスで出力 |
| `gh yard get <spec>` | リポジトリをクローンし、そのパスを出力 |
| `gh yard create <spec>` | ローカルリポジトリを作成 (`git init`) し、そのパスを出力 |
| `gh yard root` | ルートディレクトリを出力 |

`<spec>` には、`owner/repo` (ホストの既定は github.com)、`host/owner/repo`、URL (`https://`、`ssh://`、`git@host:owner/repo`) の 3 形式を指定できます。GitLab のサブグループのような深い階層もそのまま扱えます。

gh-yard 自体は `cd` の実行やエディタの起動を行わないため、出力されたパスを受け取ってユーザー側で次のように組み立てます。

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

引数なしで `d` を実行するとセレクタが開きます。一方、`d zod` を実行すると、`zod` に一致するリポジトリが 1 件のときはそのディレクトリへ直接移動し、複数あるときは `zod` で絞り込んだ状態のセレクタが開きます。

出力をいったん変数で受け取るのは、セレクタを中止した際に何も起きないようにするためです。`cd (gh yard)` と直接記述すると、中止して出力が空になった際に引数なしの `cd` が実行され、ホームディレクトリへ移動してしまいます。

## セレクタのキー操作

| キー | 動作 |
|---|---|
| `Enter` | 選択の確定 |
| `Esc` / `Ctrl-C` | 中止 |
| `↑↓` / `Ctrl-P` / `Ctrl-N` / `Ctrl-K` / `Ctrl-J` | 候補間の移動 |
| `←→` / `Ctrl-B` / `Ctrl-F` | 入力カーソルの移動 |
| `Ctrl-A` / `Ctrl-E` | 行頭・行末への移動 |
| `Backspace` | カーソル直前の 1 文字の削除 |
| `Del` / `Ctrl-D` | カーソル位置の 1 文字の削除 |
| `Ctrl-U` | カーソルより前の全文字の削除 |
| `Ctrl-W` | カーソル直前の単語の削除 |

## ルートディレクトリ

ルートディレクトリは「`YARD_ROOT` 環境変数 → `git config yard.root` → `~/yard`」の順で解決され、リポジトリは `root/host/owner/repo` の構成で配置されます。

ghq で管理している既存のディレクトリツリーを引き続き使うときは、次のようにルートをそのディレクトリに設定するだけで済みます。

```
git config --global yard.root ~/ghq
```

## 終了コード

| コード | 意味 |
|---|---|
| 0 | 正常終了 |
| 1 | セレクタの中止、または一致する対象が 0 件 |
| 2 | エラー |

## 開発

CI ではプルリクエストごとに次のチェックが実行されるため、プッシュする前に手元の環境でもこれらのチェックを実行してください。

```
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
```

ローカルでビルドしたバイナリを gh 拡張機能として使うには、リリースビルドのバイナリへのシンボリックリンクをリポジトリのルートに作成し、そこからインストールします。

```
cargo build --release
ln -s target/release/gh-yard gh-yard
gh extension install .
```

シンボリックリンクは新しくビルドされたバイナリを指し続けるため、以降は `cargo build --release` を実行するだけで変更が反映されます。

## 謝辞

gh-yard は、リポジトリを一定の構成で配置してファジー検索で選択するという、[ghq](https://github.com/x-motemen/ghq) と [fzf](https://github.com/junegunn/fzf) が確立したワークフローを受け継いでいます。長年にわたり日々愛用してきたこの 2 つのプロジェクトに感謝します。
