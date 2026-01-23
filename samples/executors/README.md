# Executors

各Executorの使用例。

## Files

| File | Executor | Description |
|------|----------|-------------|
| `bash.json` | BashExecutor | シェルコマンド実行、JSON出力の自動パース |
| `data.json` | DataExecutor | 定数データの出力 |

## Subdirectories

- `mcp/` - MCP (Model Context Protocol) 連携サンプル
- `git/` - Git操作サンプル
- `github/` - GitHub API連携サンプル

## Executor Types

| Executor | 用途 |
|----------|------|
| `log` | デバッグ・テスト用ログ出力 |
| `mcp` | Claude Code連携 |
| `dag` | サブグラフ（入れ子DAG）実行 |
| `git` | Gitリポジトリ操作 |
| `github` | GitHub API操作 |
| `bash` | シェルコマンド実行 |
| `data` | 定数データ出力 |
