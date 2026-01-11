# Task Composer

DAGベースのタスク管理ライブラリ。タスクの依存関係を管理し、非同期で並列実行します。

## 特徴

- **DAG（有向非巡回グラフ）によるタスク管理** - タスク間の依存関係を定義し、トポロジカル順序で実行
- **非同期並列実行** - 依存関係のないタスクを`tokio`で並列実行（同時実行数は設定可能）
- **静的解析** - 実行前にDAG構造とタスク定義を検証（循環検出、依存関係チェック、権限コンフリクト検出）
- **プラグイン式Executor** - `TaskExecutor`トレイトを実装して独自のタスク実行ロジックを追加
- **タスク間データフロー** - `inputs`フィールドで前のタスクの出力を参照（`$.task_id.output.field`構文）
- **埋め込み参照** - 文字列内に`${...}`構文で値を埋め込み
- **自己参照** - `$.self`で現在のタスクのフィールドを参照
- **条件付き実行** - `if`/`else`フィールドで条件に基づいてタスクをスキップ
- **MCP連携** - Model Context Protocolを通じてClaude Codeと連携
- **ロールベースの権限管理** - ファイルアクセス権限、コマンド実行権限を定義
- **マルチプラットフォームUI** - Dioxusによるデスクトップ/Web/TUI対応GUI

## インストール

```bash
git clone https://github.com/HNakamura33/task-composer.git
cd task-composer
cargo build --release

# CLIをインストール
cargo install --path task-composer-cli
```

## CLIコマンド

```bash
# ヘルプ表示
task-composer --help

# 静的解析のみ
task-composer analyze <FILE>

# 静的解析 + 実行（エラーがあれば中止）
task-composer run <FILE>
task-composer run --force <FILE>  # エラーがあっても続行

# 実行のみ（静的解析なし）
task-composer exec <FILE>

# 後方互換性（実行のみ）
task-composer <FILE>
```

### 静的解析の出力例

```
$ task-composer analyze sample_dag.json
=== Static Analysis: sample_dag.json ===

DAG Structure:
  Root nodes:  ["1"]
  Leaf nodes:  ["4"]
  Topological order: ["1", "2", "3", "4"]
  Parallel pairs: 1 pair(s)
  Critical path: ["1", "3", "4"]

Validation Results:
  [WARN] タスク 2 と 3 が '${project_root}/src' で WriteWrite 競合しています

=== Summary ===
  Warnings: 1
```

### 静的解析で検出される問題

| レベル | 検出内容 |
|--------|----------|
| Error | 循環依存、存在しない依存先、自己依存、空のタスク名、権限の矛盾 |
| Warning | 空のプロンプト、未知のExecutor、重複依存、孤立ノード、ファイルコンフリクト |

## クイックスタート

### 基本的な使い方

```bash
# サンプルDAGを実行
cargo run -p task-composer-cli

# カスタムDAGファイルを指定
cargo run -p task-composer-cli -- path/to/your_dag.json

# 静的解析
cargo run -p task-composer-cli -- analyze sample_dag.json

# テスト実行
cargo test

# ドキュメント生成
cargo doc --open
```

### プログラムから使用

```rust
use task_composer_core::dag::DAG;
use task_composer_core::analysis::StaticAnalyzer;
use task_composer_core::task_executor::LogExecutor;

#[tokio::main]
async fn main() {
    // JSONからDAGを作成
    let json = std::fs::read_to_string("sample_dag.json").unwrap();
    let mut dag = DAG::from_json(&json).unwrap();

    // 静的解析を実行
    let analyzer = StaticAnalyzer::new(&dag);
    let result = analyzer.analyze();

    if result.has_errors() {
        eprintln!("Errors found: {}", result.error_count());
        return;
    }

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
| `if` | String? | 実行条件（trueなら実行、falseならスキップ） |
| `else` | String? | 実行条件（trueならスキップ、falseなら実行） |
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

## 条件付き実行（if/else）

タスクに`if`または`else`フィールドを追加することで、条件に基づいて実行をスキップできます。

### 実行ルール

| フィールド | 条件結果 | タスク |
|-----------|---------|--------|
| なし | - | 実行 |
| `if` | true | 実行 |
| `if` | false | スキップ |
| `else` | true | スキップ |
| `else` | false | 実行 |

### 使用例

```json
{
  "tasks": [
    { "task_id": "validate", "executor": "log" },
    {
      "task_id": "on_success",
      "if": "$.validate.output.task_id == \"validate\"",
      "dependencies": ["validate"],
      "executor": "log"
    },
    {
      "task_id": "on_failure",
      "else": "$.validate.output.task_id == \"validate\"",
      "dependencies": ["validate"],
      "executor": "log"
    }
  ]
}
```

### 条件式の構文

| 種類 | 例 |
|------|---|
| パス参照 | `$.task_id.output.field` |
| 比較演算 | `==`, `!=`, `>`, `<`, `>=`, `<=` |
| 論理演算 | `&&`, `\|\|`, `!` |
| リテラル | `true`, `false`, `"string"`, `123`, `null` |

### スキップ伝播

依存先がスキップされると、依存元も自動的にスキップされます。

```
validate → on_success (スキップ) → finalize (伝播でスキップ)
```

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
use task_composer_core::task_executor::{TaskExecutor, ExecutionContext, ExecutionResult, ExecutionStatus};
use task_composer_core::types::Task;

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
            status: ExecutionStatus::Success,
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
├── Cargo.toml                   # ワークスペース定義
├── LICENSE                      # Apache 2.0
├── README.md
├── CLAUDE.md                    # Claude向け指示
├── sample_dag.json              # 基本サンプル
├── sample_mcp_dag.json          # MCP連携サンプル
├── sample_embedded_reference.json  # 埋め込み参照サンプル
├── sample_mcp_with_role.json    # Role付きMCPサンプル
├── sample_if_else.json          # if/else条件付き実行サンプル
├── sample_analysis_test.json    # 静的解析テスト用
├── task-composer-core/          # コアライブラリ
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs             # 型定義（Task, Role, Config等）
│       ├── path_resolver.rs     # パス参照解決
│       ├── dag/                 # DAG実装
│       ├── task_executor/       # Executor実装
│       └── analysis/            # 静的解析
│           ├── mod.rs           # StaticAnalyzer
│           ├── dag_analysis.rs  # DAG構造解析
│           ├── task_validation.rs # タスク検証
│           └── conflict/        # コンフリクト検出
├── task-composer-cli/           # CLIツール
│   ├── Cargo.toml
│   └── src/main.rs
├── task-composer-ui/            # Dioxus UI（Desktop/Web/TUI）
│   ├── Cargo.toml
│   ├── Dioxus.toml
│   └── src/
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
| `sample_if_else.json` | if/else条件付き実行のデモ |
| `sample_analysis_test.json` | 静的解析のエラー検出テスト用 |

## ライセンス

Apache License 2.0

## 開発

このプロジェクトはRust学習を目的としています。詳細は[CLAUDE.md](./CLAUDE.md)を参照してください。
