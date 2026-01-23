# Features

高度な機能のサンプル集。

## Subdirectories

| Directory | Description |
|-----------|-------------|
| `loop/` | ループ実行（繰り返し処理） |
| `condition/` | 条件分岐（if/else） |
| `subgraph/` | サブグラフ（入れ子DAG） |

## Feature Overview

### Loop
- `max_iterations` で最大繰り返し回数を指定
- `while_condition` / `until_condition` で継続・終了条件を指定
- `$.loop.iteration` で現在のイテレーション番号を参照

### Condition
- `if` フィールドで条件がtrueなら実行
- `else` フィールドで条件がfalseなら実行
- スキップされたタスクに依存するタスクも自動スキップ

### Subgraph
- `executor: "dag"` でサブDAGを実行
- `args.dag` でサブDAG定義を指定
- 最大3レベルまでネスト可能
