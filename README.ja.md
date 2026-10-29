# aghandoff

Claude Codeのアカウントを替えて、同じ会話を続ける。

[English](README.md) · 日本語

アカウントごとのログインを分離し、作業領域の会話を共有するRust CLIです。起動時に使うアカウントを選び、Claudeを終了した後、別のアカウントで同じセッションを再開できます。

## クイックスタート

macOSまたはLinux、Rust 1.85以上、Claude Code 2.1.268以上のstable版が必要です。初回の確認には最新のstable版を使ってください。

```sh
cargo build --release
```

以下では、ビルドした `target/release/aghandoff` が `PATH` にあるものとします。

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

名前は小文字英数字・`-`・`_` を使い、先頭は英数字、長さは48文字以内です。

`continue` は、終了コード0かつ会話ファイルが保存された最後の起動を選びます。中断・異常終了した場合は、起動時に表示されたセッションIDを `resume` に指定してください。

## 仕組み

プロファイルごとに `CLAUDE_CONFIG_DIR` を分けます。登録した作業領域では、領域専用のリンクを通して会話ファイルとauto memoryを共有し、元のセッションIDで再開します。認証、ユーザー設定、MCP認証、拡張機能、プロンプトの入力履歴は分離されます。

ログイン、資格情報の保管と更新はClaude自身が処理します。aghandoffは資格情報ファイルやOSのKeychainを直接開かず、Claudeの認証ステータスを確認します。プロキシは追加せず、管理するforeground sessionの実行中はアカウントと作業領域のロックを保持します。

**共有した履歴は、再開先アカウントでClaudeに送られ得ます。** 同じプロジェクトへのアクセスが許可されたアカウント間で使ってください。プロファイルの保存先分離は、同じOSユーザーで動く別プロセスからのアクセスを防ぐ仕組みではありません。

## 設定

保存先は `~/.local/share/aghandoff` です。変更する場合は新規の非公開ディレクトリを指定し、その場所を固定して使ってください。

```sh
aghandoff --data-dir /private/path/aghandoff account list
aghandoff --claude-bin /path/to/claude status personal
```

## 現在の対応範囲

- Claudeサブスクリプションのログインと、aghandoffで作成したセッション。
- Claudeを終了した後のアカウント変更。管理対象はforeground sessionのみ。
- 既存履歴の取り込み、Codex対応、アカウント別の使用量表示は今後の対象です。

会話の切り替えにはaghandoffの `run` / `resume` を使ってください。Claude内の `/clear` / `/resume` によるセッション変更は追跡しません。使用量は当面Claudeの `/usage` で確認できます。

文脈の再開は、Claudeの履歴保存期間やアカウント・組織の制約に従います。実行中のツール・プロセスやサーバー側キャッシュは引き継ぎません。プロファイル間での保存先共有は独自の統合方式で、実際のCLIによる確認が必要です。

## 開発

```sh
cargo fmt
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

fake Claude CLIと合成データを使った統合テストを13件用意しています。プロファイルの分離、会話・メモリの共有、再開失敗、プロセスの排他を対象としています。

Rust 1.85.1を使い、Ubuntu 24.04とmacOS 14でリリースビルド・Clippy・13件すべての統合テストが通過しています。実アカウント間の引き継ぎは手動検証が必要です。

[設計メモ](docs/design.md) · [手動検証手順](docs/manual-test.md)（英語）

## ライセンス

[MIT](LICENSE)
