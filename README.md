# Task Composer

DAGベースのタスク管理ライブラリ。タスクの依存関係を管理し、非同期で並列実行します。

## 特徴

- **DAG（有向非巡回グラフ）によるタスク管理** - タスク間の依存関係を定義し、トポロジカル順序で実行
- **非同期並列実行** - 依存関係のないタスクを`tokio`で並列実行（同時実行数は設定可能）
- **プラグイン式Executor** - `TaskExecutor`トレイトを実装して独自のタスク実行ロジックを追加
- **タスク間データフロー** - `inputs`フィールドで前のタスクの出力を参照（`$.task_id.output.field`構文）
- **ロールベースの権限管理** - ファイルアクセス権限、コマンド実行権限を定義

## インストール

```bash
git clone https://github.com/your-repo/task-composer.git
cd task-composer
cargo build
```

## クイックスタート

### 基本的な使い方

```bash
# サンプルDAGを実行
cargo run

# カスタムDAGファイルを指定
cargo run -- path/to/your_dag.json

# テスト実行
cargo test

# ドキュメント生成
cargo doc --open
```

### プログラムから使用

```rust
use task_composer::dag::DAG;
use task_composer::task_executor::LogExecutor;

#[tokio::main]
async fn main() {
    // JSONからDAGを作成
    let json = std::fs::read_to_string("sample_dag.json").unwrap();
    let mut dag = DAG::from_json(&json).unwrap();

    // Executorを登録
    dag.register_executor(Box::new(LogExecutor::new()));

    // 非同期で実行
    let results = dag.execute_async().await.unwrap();

    println!("Executed {} tasks", results.len());
}
```

## DAG JSON形式

```json
{
  "tasks": [
    {
      "task_id": "1",
      "name": "Setup Environment",
      "description": "開発環境のセットアップ",
      "priority": 1,
      "status": "Pending",
      "prompt": "プロジェクトを初期化して依存関係をインストール",
      "executor": "log",
      "args": {"env": "development"},
      "dependencies": [],
      "role": {
        "role_id": "role_setup",
        "name": "Setup Role",
        "subagents": [],
        "skills": ["environment"],
        "description": "環境構築用ロール",
        "tool_permissions": {
          "bash": {
            "allowed_commands": ["git", "npm"],
            "blocked_commands": ["rm -rf /"],
            "require_confirmation": []
          },
          "write": {
            "max_file_size_mb": 10,
            "allowed_extensions": [".rs", ".json"]
          }
        },
        "file_permissions": {
          "allowed_paths": ["${project_root}/src"],
          "denied_paths": ["${project_root}/.env"],
          "read_only_paths": []
        }
      }
    },
    {
      "task_id": "2",
      "name": "Build",
      "executor": "log",
      "args": {},
      "inputs": {
        "setup_result": "$.1.output.message"
      },
      "dependencies": ["1"],
      "role": { "..." : "..." }
    }
  ]
}
```

### 主要フィールド

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `task_id` | String | タスクの一意な識別子 |
| `name` | String | タスク名 |
| `executor` | String | 使用するExecutorの名前 |
| `args` | JSON | タスクに渡す静的な引数 |
| `inputs` | JSON | 依存タスクの出力を参照するパス定義 |
| `dependencies` | Array | 依存するタスクIDのリスト |
| `role` | Object | ロール定義（権限を含む） |

### Input Path構文

依存タスクの出力を参照するための構文：

```
$.{task_id}.output.{field}           # 基本的なフィールドアクセス
$.{task_id}.output.config.host       # ネストしたフィールド
$.{task_id}.output.items[0]          # 配列インデックス
$.{task_id}.output.users[0].name     # 配列＋ネスト
```

## カスタムExecutorの作成

```rust
use async_trait::async_trait;
use task_composer::task_executor::{TaskExecutor, ExecutionContext, ExecutionResult};
use task_composer::types::Task;

struct MyExecutor;

#[async_trait]
impl TaskExecutor for MyExecutor {
    fn name(&self) -> &str {
        "my_executor"
    }

    async fn execute_task(
        &self,
        task: &Task,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        // カスタム実行ロジック
        println!("Executing: {}", task.name);

        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            success: true,
            output: serde_json::json!({
                "result": "completed",
                "data": ctx.args
            }),
        })
    }
}

// 使用方法
let mut dag = DAG::from_json(&json)?;
dag.register_executor(Box::new(MyExecutor));
```

## 設定

`Config`構造体で実行時の設定を制御できます：

```rust
pub struct Config {
    /// 同時に実行できるタスクの最大数（デフォルト: 4）
    pub max_concurrent_tasks: usize,
}
```

## プロジェクト構成

```
task-composer/
├── Cargo.toml
├── sample_dag.json          # サンプルDAG
├── large_dag.json           # 大規模DAGサンプル
└── src/
    ├── main.rs              # CLIエントリーポイント
    ├── types.rs             # 型定義（Task, Role, Config等）
    ├── dag/
    │   ├── mod.rs           # DAG実装
    │   └── tests.rs         # テスト
    ├── task_executor/
    │   ├── mod.rs           # Executorトレイト・レジストリ
    │   └── log_executor.rs  # ログ出力Executor
    ├── path_resolver.rs     # Input Path解決
    └── conflict/
        ├── mod.rs           # 競合検出
        └── tests.rs
```

## 実行例

```
Loaded 4 tasks
  Task 1: Setup Environment
  Task 2: Design Database
  Task 3: Implement API
  Task 4: Integration

Edges:
  1 -> 2
  1 -> 3
  2 -> 4
  3 -> 4

=== Executing DAG ===

========================================
Executing Task: Setup Environment
========================================
  ID:          1
  Args:        {"env":"development"}
========================================

  [Task 1 completed]
  [Task 2 started]  <- 並列実行
  [Task 3 started]  <-
  [Task 2 completed]
  [Task 3 completed]
  [Task 4 started]
  [Task 4 completed]

=== Execution Complete ===
Executed 4 tasks in 1.52s
```

## ライセンス

MIT License

## 開発

このプロジェクトはRust学習を目的としています。詳細は[CLAUDE.md](./CLAUDE.md)を参照してください。
