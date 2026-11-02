# aghandoff

Claude Codeのアカウントを替えて、同じ会話を続ける。

[English](README.md) · 日本語

アカウントごとのログインを分離し、作業領域の会話を共有するRust CLIです。起動時に使うアカウントを選び、Claudeを終了した後、別のアカウントで同じセッションを再開できます。

## 必要なもの

- macOSまたはLinux
- Git
- Rust 1.85以上（Cargoを含む）
- Claude Code 2.1.268以上のstable版と、利用可能なClaudeサブスクリプション

Rustはaghandoffのビルド・インストール時に必要です。インストール済みの実行ファイルを使う際には不要です。Claude Codeは初回の確認に最新のstable版を使ってください。

<details>
<summary>rustupでRustをインストールする</summary>

[rustup](https://rust-lang.org/tools/install/)はRust公式が推奨するインストーラーです。

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

案内に従ってインストールした後、ターミナルを開き直して確認します。

```sh
rustc --version
cargo --version
```

macOSでビルド時にリンカーが見つからない場合は、`xcode-select --install`で[Command Line Tools](https://developer.apple.com/documentation/xcode/installing-the-command-line-tools/)を導入してから再実行してください。

</details>

<details>
<summary>miseを使う場合（任意）</summary>

すでに[mise](https://mise.jdx.dev/lang/rust.html)を使っている場合は、Rustもmiseで管理できます。miseは内部でrustupを使用し、必要に応じて導入するため、rustupを別途インストールする必要はありません。

```sh
mise use -g rust@stable
mise exec -- rustc --version
mise exec -- cargo --version
```

`-g`は、このユーザーの既定のRustバージョンを設定します。以下の手順でリポジトリをcloneし、ディレクトリへ移動した後は、次のコマンドでaghandoffをインストール・確認できます。

```sh
mise exec -- cargo install --path . --locked
mise exec -- aghandoff --help
```

シェルでmiseを有効化していない場合は、この後の`aghandoff`コマンドにも`mise exec --`を付けてください。miseは必須ではなく、rustupのみでも利用できます。

</details>

## クイックスタート

```sh
git clone https://github.com/rrhaya/aghandoff.git
cd aghandoff
cargo install --path . --locked
```

Cargoのバイナリ用ディレクトリ（通常は`~/.cargo/bin`）に `aghandoff` をインストールします。そのディレクトリが `PATH` に含まれていることを確認するか、上記の`mise exec --`を使ってください。

```sh
aghandoff --help
```

```sh
# 2つのプロファイルを作り、Claudeのログイン画面から認証します。
aghandoff account add personal
aghandoff account add second
aghandoff login personal
aghandoff login second

# 会話を共有するプロジェクトを登録します。
aghandoff workspace add myapp /path/to/myapp

# 最初のアカウントで会話を始めます。
aghandoff run myapp personal

# /exit で終了した後、別のアカウントで同じ会話を再開します。
aghandoff continue myapp second
```

送信前にClaude内の `/status` でアカウントを確認してください。プロファイル名はローカルのラベルです。同じ実アカウントに別のラベルでログインしても、別アカウントにはなりません。

## 既存の環境を取り込む

aghandoffを使い始める前の会話をプレビューし、`--apply` でコピーできます。

```sh
aghandoff import sessions myapp personal --from /path/to/old/project-history
aghandoff import sessions myapp personal --from /path/to/old/project-history --apply
```

取り込み元には、通常の `~/.claude/projects/` 内の対象プロジェクトのディレクトリを指定します。`--session <uuid>` で会話を選択でき、複数回指定も可能です。`--with-memory` を付けると、そのプロジェクトのauto memoryも取り込みます。適用後に再開用のコマンドが表示されます。

個人の指示・スキルも、アカウントプロファイルごとにコピーできます。

```sh
aghandoff import config personal --instructions ~/.claude/CLAUDE.md --skill ~/.claude/skills/review
# プレビューを確認した後、--apply を付けて再実行します。
```

`--instructions` には個人の `AGENTS.md` なども指定でき、プロファイルの `CLAUDE.md` として保存します。`--skill` は任意のローカルディレクトリのスキルを選択でき、複数回指定できます。`--rules <dir>` は個人のルール、`--settings <file>` は基本設定の取り込みです。

設定から取り込むのは `model`、`language`、`outputStyle`、`alwaysThinkingEnabled`、`effortLevel` の5項目だけです。認証情報、hooks、環境変数、MCP設定、権限設定は移しません。既存ファイルは上書きせず、元のファイルも変更しません。コピー後の自動同期は行わないため、同じ個人設定を使いたいプロファイルごとに実行してください。

プロジェクト内の指示ファイルや `.claude/skills/` は、従来どおりClaude自身のルールで読み込まれます。取り込み前に元のClaudeセッションを終了してください。明示した取り込み元のシンボリックリンクは解決しますが、内部のリンクや既知の秘密ファイル名は拒否します。取り込み対象の指示やスキル自体に秘密情報が含まれないことも確認してください。

取り込みは一度に512 MiB・10,000ファイルまでです。既存ファイルとの衝突や登録済みIDがあれば、選択全体を中止します。新しい作業領域では、更新日時が最新の取り込み済み会話を `continue` の対象にします。既に対象がある場合は変更しません。JSONL以外への書き出し、外部のファイル復元用チェックポイント、実行中のプロセス、サーバー側状態は移行しません。[詳細な取り込み手順](docs/import.md)（英語）を参照してください。

## コマンド

| コマンド | 用途 |
| --- | --- |
| `account add <name>` / `account list` | アカウントプロファイルの作成・一覧 |
| `login <account>` | 指定したプロファイルでログイン |
| `status <account>` | サブスク認証と設定ディレクトリの確認 |
| `workspace add <name> <path>` / `workspace list` | 共有する作業領域の登録・一覧 |
| `run <workspace> <account>` | 新しい会話を開始 |
| `continue <workspace> <account>` | 最後に正常保存されたセッションを再開 |
| `resume <workspace> <account> <session-id>` | 登録済みのセッションを指定して再開 |
| `sessions <workspace>` | セッションIDと起動情報を表示 |
| `import sessions <workspace> <account> --from <dir>` | 既存の会話をプレビュー。`--apply` でコピー |
| `import config <account> [options]` | 個人の指示・スキル・ルール・基本設定を選択して取り込み |

名前は小文字英数字・`-`・`_` を使い、先頭は英数字、長さは48文字以内です。

`continue` は、終了コード0かつ会話ファイルが保存された最後の起動を選びます。中断・異常終了した場合は、起動時に表示されたセッションIDを `resume` に指定してください。

## 仕組み

プロファイルごとに `CLAUDE_CONFIG_DIR` を分けます。登録した作業領域では、領域専用のリンクを通して会話ファイルとauto memoryを共有し、元のセッションIDで再開します。認証、MCP認証、拡張機能、プロンプトの入力履歴は分離されます。個人設定も分離され、`import config` で選んだものだけコピーできます。

ログイン、資格情報の保管と更新はClaude自身が処理します。aghandoffは資格情報ファイルやOSのKeychainを直接開かず、Claudeの認証ステータスを確認します。プロキシは追加せず、管理するforeground sessionの実行中はアカウントと作業領域のロックを保持します。

**共有した履歴は、再開先アカウントでClaudeに送られ得ます。** 同じプロジェクトへのアクセスが許可されたアカウント間で使ってください。プロファイルの保存先分離は、同じOSユーザーで動く別プロセスからのアクセスを防ぐ仕組みではありません。

## 設定

保存先は `~/.local/share/aghandoff` です。変更する場合は新規の非公開ディレクトリを指定し、その場所を固定して使ってください。

```sh
aghandoff --data-dir /private/path/aghandoff account list
aghandoff --claude-bin /path/to/claude status personal
```

## 現在の対応範囲

- Claudeサブスクリプションのログイン、aghandoffで作成したセッション、取り込んだネイティブ形式の会話履歴。
- Claudeを終了した後のアカウント変更。管理対象はforeground sessionのみ。
- 選択した既存履歴と個人設定の取り込み。Codex対応とアカウント別の使用量表示は今後の対象です。

会話の切り替えにはaghandoffの `run` / `resume` を使ってください。Claude内の `/clear` / `/resume` によるセッション変更は追跡しません。使用量は当面Claudeの `/usage` で確認できます。

文脈の再開は、Claudeの履歴保存期間やアカウント・組織の制約に従います。実行中のツール・プロセスやサーバー側キャッシュは引き継ぎません。プロファイル間での保存先共有は独自の統合方式で、実際のCLIによる確認が必要です。

## 開発

```sh
cargo fmt
cargo fmt --check
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
```

fake Claude CLIと合成データを使った統合テストを22件用意しています。プロファイルの分離、会話・メモリの共有、再開失敗、プロセスの排他、履歴移行、個人設定の選択、取り込み先との衝突を対象としています。

`Cargo.lock`でCIに使用する依存バージョンを固定しています。

CIはRust 1.85.1を使い、Ubuntu 24.04とmacOS 14でリリースビルド・整形・Clippy・すべての統合テストを確認します。実アカウント間の引き継ぎと、実際のClaudeでの取り込み履歴の再開は手動検証が必要です。

[設計メモ](docs/design.md) · [手動検証手順](docs/manual-test.md)（英語）

## ライセンス

[MIT](LICENSE)
