# Loop

ループ実行（繰り返し処理）のサンプル。

## Files

| File | Description |
|------|-------------|
| `basic.json` | 基本ループ（カウンター、max_iterations: 5） |
| `ralph_loop.json` | TDD自己改善ループ（AIによる反復的実装改善） |

## Loop Configuration

```json
{
  "loop_config": {
    "max_iterations": 5,
    "while_condition": "$.task.output.continue == true",
    "until_condition": "$.task.output.done == true"
  }
}
```

## Loop Context References

| Reference | Description |
|-----------|-------------|
| `$.loop.iteration` | 現在のイテレーション番号（0始まり） |
| `$.loop.first` | 初回かどうか（true/false） |
| `$.loop.previous.{task_id}.output.{field}` | 前回イテレーションの結果 |
