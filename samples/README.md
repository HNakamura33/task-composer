# Sample DAG Files

Task Composerのサンプルファイル集です。

## Directory Structure

```
samples/
├── basics/          # 入門・基本サンプル
├── executors/       # Executor別サンプル
│   ├── mcp/         # MCP (Claude Code) 連携
│   ├── git/         # Git操作
│   └── github/      # GitHub API連携
├── features/        # 高度な機能サンプル
│   ├── loop/        # ループ実行
│   ├── condition/   # 条件分岐
│   └── subgraph/    # サブグラフ（入れ子DAG）
├── workflows/       # 実践的ワークフロー
└── _internal/       # 内部テスト用
```

## Getting Started

1. **最初に**: `basics/minimal.json` - 最小構成のDAG
2. **次に**: `basics/simple_dag.json` - 依存関係のある基本DAG
3. **参照機能**: `basics/embedded_reference.json` - タスク間のデータ参照

## Running Samples

```bash
# 特定のサンプルを実行
task-composer run samples/basics/minimal.json

# デフォルト（basics/simple_dag.json）を実行
cargo run -p task-composer-cli
```
