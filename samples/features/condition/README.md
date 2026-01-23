# Condition

条件分岐（if/else）のサンプル。

## Files

| File | Description |
|------|-------------|
| `if_else.json` | if/else条件実行（validate → 成功/失敗で分岐） |

## Condition Syntax

```json
{
  "task_id": "on_success",
  "if": "$.validate.output.valid == true",
  "executor": "log"
}
```

```json
{
  "task_id": "on_failure",
  "else": "$.validate.output.valid == true",
  "executor": "log"
}
```

## Supported Operators

### Comparison
`==`, `!=`, `>`, `<`, `>=`, `<=`

### Logical
`&&`, `||`, `!`

## Skip Propagation

依存先がスキップされた場合、依存元も自動的にスキップされます。
