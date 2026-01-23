# Basics

入門・基本サンプル集。Task Composerの基礎を学ぶのに最適です。

## Files

| File | Description |
|------|-------------|
| `minimal.json` | 最小構成（2タスク: build → test） |
| `simple_dag.json` | 基本DAG（4タスク: 環境設定 → DB/API並列 → 統合） |
| `embedded_reference.json` | 埋め込み参照（`${$.task.output.field}`形式） |
| `auto_dependency.json` | 依存関係の自動解決 |

## Recommended Order

1. `minimal.json` - DAGの最小構成を理解
2. `simple_dag.json` - 依存関係と並列実行を理解
3. `embedded_reference.json` - タスク間のデータ受け渡しを理解
4. `auto_dependency.json` - 参照から自動で依存関係を推論
