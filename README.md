# Task Composer

DAGベースのタスク管理ライブラリ。タスクの依存関係を管理し、非同期で並列実行します。

## 特徴

- **DAG（有向非巡回グラフ）によるタスク管理** - タスク間の依存関係を定義し、トポロジカル順序で実行
- **非同期並列実行** - 依存関係のないタスクを`tokio`で並列実行（同時実行数は設定可能）
- **プラグイン式Executor** - `TaskExecutor`トレイトを実装して独自のタスク実行ロジックを追加
- **タスク間データフロー** - `inputs`フィールドで前のタスクの出力を参照（`$.task_id.output.field`構文）
- **埋め込み参照** - 文字列内に`${...}`構文で値を埋め込み
- **自己参照** - `$.self`で現在のタスクのフィールドを参照
- **MCP連携** - Model Context Protocolを通じてClaude Codeと連携
- **ロールベースの権限管理** - ファイルアクセス権限、コマンド実行権限を定義

## インストール

```bash
git clone https://github.com/HNakamura33/task-composer.git
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
          "allowed_paths": ["src/"],
          "denied_paths": [".env"],
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
      "dependencies": ["1"]
    }
  ],
  "config": {
    "max_concurrent_tasks": 4
  }
}
```

### 主要フィールド

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `task_id` | String | タスクの一意な識別子 |
| `name` | String | タスク名 |
| `executor` | String | 使用するExecutorの名前（`log`, `mcp`） |
| `args` | JSON | タスクに渡す静的な引数 |
| `inputs` | JSON | 依存タスクの出力を参照するパス定義 |
| `dependencies` | Array | 依存するタスクIDのリスト |
| `role` | Object | ロール定義（権限を含む） |

## パス参照構文

### 基本構文（`$.{task_id}.output.{field}`）

依存タスクの出力を参照：

```
$.1.output.user_id           # 基本的なフィールドアクセス
$.1.output.config.host       # ネストしたフィールド
$.1.output.items[0]          # 配列インデックス
$.1.output.users[0].name     # 配列＋ネスト
$.001-101.output.data        # ハイフン付きタスクID
```

### 埋め込み参照（`${...}`）

文字列内に値を埋め込み：

```json
{
  "prompt": "Hello ${$.1.output.name}! Your ID is ${$.1.output.user_id}."
}
```

### 自己参照（`$.self`）

現在のタスクのフィールドを参照：

```json
{
  "args": {
    "current_task_name": "$.self.name",
    "current_role": "$.self.role",
    "role_skills": "$.self.role.skills"
  }
}
```

対応フィールド: `task_id`, `name`, `description`, `priority`, `status`, `prompt`, `executor`, `args`, `dependencies`, `role`

## Executor

### LogExecutor

デバッグ・テスト用のシンプルなExecutor。タスク情報をログ出力します。

```rust
dag.register_executor(Box::new(LogExecutor::new()));
```

### McpExecutor

Model Context Protocolを通じて外部MCPサーバーと連携します。

```json
{
  "executor": "mcp",
  "args": {
    "connection": {
      "type": "stdio",
      "command": "uv",
      "args": ["run", "--directory", "/path/to/mcp_server", "main.py", "serve"]
    },
    "tool": "claude_code_query",
    "arguments": {
      "prompt": "Review this code for security issues",
      "options": {
        "cwd": "/path/to/project",
        "max_turns": 5,
        "allowed_tools": ["Read", "Grep", "Glob"]
      },
      "extra_options": {
        "role": "$.self.role"
      }
    }
  }
}
```

### カスタムExecutorの作成

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

// 登録
dag.register_executor(Box::new(MyExecutor));
```

## MCP Server (Claude Code)

`mcp_servers/claude_code_mcp/`にClaude Code連携用のMCPサーバーが含まれています。

### セットアップ

```bash
cd mcp_servers/claude_code_mcp
uv sync
```

### 機能

- `claude_code_query` ツール: Claude Codeにクエリを送信
- `options`: Claude Agent SDK のオプション（`cwd`, `max_turns`, `allowed_tools`等）
- `extra_options.role`: ロール情報をシステムプロンプトとして注入

## プロジェクト構成

```
task-composer/
├── Cargo.toml
├── LICENSE                      # Apache 2.0
├── README.md
├── CLAUDE.md                    # Claude向け指示
├── sample_dag.json              # 基本サンプル
├── sample_mcp_dag.json          # MCP連携サンプル
├── sample_embedded_reference.json  # 埋め込み参照サンプル
├── sample_mcp_with_role.json    # Role付きMCPサンプル
├── src/
│   ├── main.rs                  # CLIエントリーポイント
│   ├── types.rs                 # 型定義（Task, Role, Config等）
│   ├── path_resolver.rs         # パス参照解決
│   ├── dag/
│   │   ├── mod.rs               # DAG実装
│   │   └── tests.rs             # テスト
│   ├── task_executor/
│   │   ├── mod.rs               # Executorトレイト・レジストリ
│   │   ├── log_executor.rs      # ログ出力Executor
│   │   └── mcp_executor.rs      # MCP Executor
│   └── conflict/
│       ├── mod.rs               # 競合検出
│       └── tests.rs
└── mcp_servers/
    └── claude_code_mcp/
        ├── main.py              # FastMCPサーバー
        └── pyproject.toml
```

## サンプルファイル

| ファイル | 説明 |
|----------|------|
| `sample_dag.json` | 基本的なDAG（LogExecutor使用） |
| `sample_mcp_dag.json` | MCP連携によるコード分析・README生成 |
| `sample_embedded_reference.json` | 埋め込み参照（`${...}`）のデモ |
| `sample_mcp_with_role.json` | Role情報をMCPに渡すデモ |

## ライセンス

Apache License 2.0

## 開発

このプロジェクトはRust学習を目的としています。詳細は[CLAUDE.md](./CLAUDE.md)を参照してください。
