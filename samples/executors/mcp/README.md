# MCP Executor

MCP (Model Context Protocol) を使ったClaude Code連携サンプル。

## Files

| File | Description |
|------|-------------|
| `basic.json` | 基本的なMCP連携（コード分析、README生成） |
| `with_role.json` | ロール付きMCP（セキュリティレビュー + ドキュメント作成） |
| `with_timeout.json` | タイムアウト設定付きMCP |

## Prerequisites

MCP Serverが起動している必要があります:

```bash
cd mcp_servers/claude_code_mcp
uv run main.py
```

## Key Features

- `claude_code_query` ツールでClaude Codeにクエリ送信
- `extra_options.role` でロール情報をシステムプロンプトに変換
- `timeout_secs` でタスク単位のタイムアウト設定
