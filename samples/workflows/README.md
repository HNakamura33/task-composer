# Workflows

実践的なワークフローサンプル。Issue駆動開発やCI/CDパイプラインなど。

## Files

| File | Description |
|------|-------------|
| `simple.json` | シンプルワークフロー（Issue取得 → ブランチ作成 → 実装 → PR作成） |
| `issue_bugfix.json` | バグ修正パターン（セキュリティパッチ → 認証修正 → リファクタ → 回帰テスト） |
| `issue_feature.json` | 新機能開発（設計 → 実装 → 統合テスト → ドキュメント） |
| `issue_microservices.json` | マイクロサービス更新（共通ライブラリ → 4サービス並列更新 → E2Eテスト） |
| `issue_tdd.json` | TDD開発ワークフロー（Issue取得 → 設計 → テスト生成 → worktree並列実装 → マージ → PR） |

## Workflow Patterns

### Simple (simple.json)
```
Issue取得 → ブランチ作成 → 実装 → PR作成
```

### Feature Development (issue_feature.json)
```
         ┌─ DB設計
Issue → ├─ API設計 → 実装 → 統合テスト → ドキュメント
         └─ UI設計
```

### Microservices (issue_microservices.json)
```
              ┌─ Service A
共通ライブラリ → ├─ Service B → E2Eテスト
              ├─ Service C
              └─ Service D
```
