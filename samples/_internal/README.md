# Internal

内部テスト・検証用サンプル。通常の使用では参照不要です。

## Files

| File | Description |
|------|-------------|
| `analysis_test.json` | 静的解析テスト（意図的なエラーケース） |
| `error_test.json` | エラー処理テスト（不明なexecutor、依存失敗時のスキップ） |
| `bash_worktree_test.json` | Git worktreeテスト |
| `large_dag.json` | 大規模DAGテスト（7タスク） |
| `huge_dag.json` | 超大規模DAGテスト（17タスク） |

## Purpose

これらのファイルは以下の目的で使用されます:

- 静的解析機能の検証
- エラーハンドリングの検証
- パフォーマンステスト
- CI/CDでの自動テスト
