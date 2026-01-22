# GitHub Executor

GitHubExecutorを使ったGitHub API連携サンプル。

## Files

| File | Description |
|------|-------------|
| `basic.json` | 基本GitHub操作（Issue作成・取得・クローズ） |
| `all_operations.json` | GitHub全操作網羅 |

## Supported Operations

### Issues
| Operation | Description |
|-----------|-------------|
| `create_issue` | Issue作成 |
| `get_issue` | Issue取得 |
| `list_issues` | Issue一覧 |
| `update_issue` | Issue更新 |
| `close_issue` | Issueクローズ |
| `add_comment` | コメント追加 |

### Pull Requests
| Operation | Description |
|-----------|-------------|
| `list_prs` | PR一覧 |
| `get_pr` | PR取得 |

## Prerequisites

GitHub認証トークンが必要です（環境変数 `GITHUB_TOKEN`）。
