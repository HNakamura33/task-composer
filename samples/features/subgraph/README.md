# Subgraph

サブグラフ（入れ子DAG）のサンプル。

## Files

| File | Description |
|------|-------------|
| `basic.json` | 基本サブグラフ（ETLパイプライン） |
| `nested.json` | ネストしたサブグラフ（2階層） |
| `with_inputs.json` | サブグラフへの入力渡し |

## Basic Usage

```json
{
  "task_id": "subdag_task",
  "executor": "dag",
  "args": {
    "dag": {
      "tasks": [...]
    }
  }
}
```

## Referencing Subgraph Results

親DAGからサブグラフ内タスクの結果を参照:

```
$.{subdag_task}.output.{inner_task}.output.{field}
```

## Passing Inputs to Subgraph

```json
{
  "args": {
    "inputs": { "key": "value" },
    "dag": { "tasks": [...] }
  }
}
```

サブグラフ内では `$.inputs.key` で参照。
