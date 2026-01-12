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
- **サブグラフ実行** - `DagExecutor`でDAGをネストして実行（最大3レベル）、サブグラフ内の結果を親から参照可能
- **ループ実行** - `loop_config`でDAGを繰り返し実行、`$.loop.*`で前回イテレーションの結果を参照可能
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

### 最小構成

```json
{
  "tasks": [
    { "task_id": "build", "executor": "log" },
    { "task_id": "test", "executor": "log", "dependencies": ["build"] }
  ]
}
```

### 完全な例

```json
{
  "tasks": [
    {
      "task_id": "1",
      "name": "Setup Environment",
      "description": "開発環境のセットアップ",
      "priority": 1,
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
      "args": {
        "setup_result": "$.1.output.message"
      },
      "dependencies": ["1"]
    }
  ],
  "config": {
    "max_concurrent_tasks": 4,
    "default_task_timeout_secs": 300
  }
}
```

### 主要フィールド

#### 必須フィールド

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `task_id` | String | タスクの一意な識別子 |
| `executor` | String | 使用するExecutorの名前（`log`, `mcp`, `dag`） |

#### オプショナルフィールド

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `name` | String? | タスクの表示名（省略時はtask_idを使用） |
| `description` | String? | タスクの詳細説明 |
| `priority` | u8 | 優先度（0-255、デフォルト: 0） |
| `prompt` | String? | タスク実行時のプロンプト |
| `args` | JSON | タスクに渡す引数（パス参照 `$.task_id.output.*` を含む） |
| `dependencies` | Array | 依存するタスクIDのリスト |
| `if` | String? | 実行条件（trueなら実行、falseならスキップ） |
| `else` | String? | 実行条件（trueならスキップ、falseなら実行） |
| `role` | Object? | ロール定義（省略時は全権限許可） |
| `timeout_secs` | u64? | タスクのタイムアウト（秒）。省略時はconfigのデフォルト値を使用 |

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

### DagExecutor

サブグラフ（入れ子DAG）を実行します。最大3レベルまでネスト可能で、サブグラフ内の結果を親DAGから参照できます。

```json
{
  "task_id": "data_pipeline",
  "executor": "dag",
  "args": {
    "dag": {
      "tasks": [
        {"task_id": "extract", "executor": "log", "dependencies": []},
        {"task_id": "transform", "executor": "log", "dependencies": ["extract"]},
        {"task_id": "load", "executor": "log", "dependencies": ["transform"]}
      ],
      "config": {"max_concurrent_tasks": 1}
    }
  }
}
```

サブグラフの結果は親DAGから以下のように参照できます：

```json
{
  "inputs": {
    "load_result": "$.data_pipeline.output.load.output.task_id",
    "load_status": "$.data_pipeline.output.load.status"
  }
}
```

### BashExecutor

シェルコマンドを実行します。コマンドの出力がJSON形式の場合、自動的にパースして`parsed`フィールドに格納します。

```json
{
  "executor": "bash",
  "args": {
    "command": "echo '{\"status\": \"success\"}'",
    "cwd": "/path/to/workdir",
    "timeout_secs": 60,
    "shell": "sh"
  }
}
```

#### パラメータ

| パラメータ | 説明 | デフォルト |
|-----------|------|-----------|
| `command` | 実行するシェルコマンド（必須） | - |
| `cwd` | 作業ディレクトリ | カレントディレクトリ |
| `timeout_secs` | タイムアウト（秒） | 300 |
| `shell` | 使用するシェル | `sh` |

#### 出力形式

```json
{
  "exit_code": 0,
  "stdout": "{\"status\": \"success\"}",
  "stderr": "",
  "success": true,
  "parsed": { "status": "success" }
}
```

- `parsed`: stdoutがJSON形式の場合、パース結果が格納される（パース失敗時は`null`）
- 他のタスクから `$.task_id.output.parsed.field` で参照可能

#### 使用例：パターンマッチング

```json
{
  "task_id": "check",
  "executor": "bash",
  "args": {
    "command": "echo '${$.prev.output}' | grep -q 'SUCCESS' && echo '{\"matched\": true}' || echo '{\"matched\": false}'"
  }
}
```

### GitExecutor

ローカルGitリポジトリの操作を実行します。clone、commit、branch操作など基本的なGit操作をサポートします。

```json
{
  "executor": "git",
  "args": {
    "action": {
      "type": "clone",
      "url": "https://github.com/user/repo.git",
      "path": "/tmp/my-repo",
      "branch": "main"
    }
  }
}
```

#### サポートする操作

| 操作 | 説明 | 必須パラメータ |
|------|------|---------------|
| `clone` | リポジトリをクローン | `url`, `path` |
| `open` | 既存リポジトリを開く | `path` |
| `init` | 新規リポジトリを初期化 | `path` |
| `status` | ステータス確認 | `path` |
| `diff` | 変更差分を表示 | `path` |
| `log` | コミット履歴を表示 | `path` |
| `commit` | コミット作成 | `path`, `message` |
| `create_branch` | ブランチ作成 | `path`, `name` |
| `checkout` | ブランチ切り替え | `path`, `branch` |
| `list_branches` | ブランチ一覧 | `path` |
| `delete_branch` | ブランチ削除 | `path`, `name` |
| `fetch` | リモートから取得 | `path` |
| `push` | リモートへプッシュ | `path` |

#### 認証オプション

```json
{
  "action": {
    "type": "clone",
    "url": "git@github.com:user/repo.git",
    "path": "/tmp/repo",
    "auth": {
      "type": "ssh_agent"
    }
  }
}
```

| 認証タイプ | 説明 |
|-----------|------|
| `ssh_agent` | SSH Agentを使用 |
| `ssh_key` | SSHキーファイルを指定（`private_key_path`, `passphrase`） |
| `user_password` | ユーザー名/パスワード認証（`username`, `password`） |

### GitHubExecutor

GitHub APIを通じてIssueやPull Requestを操作します。環境変数`GITHUB_TOKEN`で認証するか、argsで`token`を指定します。

```json
{
  "executor": "github",
  "args": {
    "owner": "user",
    "repo": "repository",
    "action": {
      "type": "create_issue",
      "title": "Bug report",
      "body": "Description of the issue"
    }
  }
}
```

#### Issue操作

| 操作 | 説明 | 必須パラメータ |
|------|------|---------------|
| `create_issue` | Issue作成 | `title` |
| `get_issue` | Issue詳細取得 | `number` |
| `list_issues` | Issue一覧 | - |
| `update_issue` | Issue更新 | `number` |
| `close_issue` | Issueクローズ | `number` |
| `create_comment` | コメント追加 | `number`, `body` |
| `delete_comment` | コメント削除 | `comment_id` |

#### Pull Request操作

| 操作 | 説明 | 必須パラメータ |
|------|------|---------------|
| `create_pr` | PR作成 | `title`, `head`, `base` |
| `get_pr` | PR詳細取得 | `number` |
| `list_prs` | PR一覧 | - |
| `merge_pr` | PRマージ | `number` |
| `request_review` | レビュー依頼 | `number` |

### ループ実行

`loop_config`を使用してDAGを繰り返し実行できます。

```json
{
  "loop_config": {
    "max_iterations": 5,
    "until_condition": "$.loop.iteration >= 3"
  },
  "tasks": [...]
}
```

#### ループ設定（loop_config）

| フィールド | 説明 |
|-----------|------|
| `max_iterations` | 最大繰り返し回数（必須） |
| `while_condition` | 継続条件（trueの間ループ継続） |
| `until_condition` | 終了条件（trueになったらループ終了） |

#### ループ参照（$.loop.*）

| 参照 | 説明 | 例 |
|------|------|---|
| `$.loop.iteration` | 現在のイテレーション番号（0始まり） | `0`, `1`, `2`... |
| `$.loop.first` | 初回かどうか | `true` / `false` |
| `$.loop.previous.{task_id}.output` | 前回イテレーションの結果 | `$.loop.previous.counter.output.value` |

```json
{
  "args": {
    "iteration": "$.loop.iteration",
    "is_first": "$.loop.first",
    "previous_value": "$.loop.previous.counter.output.value"
  }
}
```

初回イテレーション（`$.loop.first == true`）では、`$.loop.previous.*`は`null`を返します。

### Ralph Loopパターン

[Ralph Loop](https://github.com/anthropics/claude-plugins-official/tree/main/plugins/ralph-loop)は、AIエージェントに反復的な自己改善ループを実行させるパターンです。task-composerの`loop_config`と`McpExecutor`を組み合わせることで実現できます。

#### 特徴

| Ralph Loopの特徴 | task-composerでの実現方法 |
|-----------------|-------------------------|
| プロンプト不変 | 同じtask定義がループで繰り返し実行される |
| ファイル永続性 | Claude Codeがファイルに書き込み、次のイテレーションで参照 |
| 自己参照 | `$.loop.previous.*` で前回の結果を参照 |
| 完了条件 | `until_condition` で特定の出力を検出して終了 |
| 最大反復数 | `max_iterations` で無限ループを防止 |

#### 使用例

```json
{
  "loop_config": {
    "max_iterations": 10,
    "until_condition": "$.improve.output.all_tests_passed == true"
  },
  "tasks": [
    {
      "task_id": "improve",
      "executor": "mcp",
      "args": {
        "connection": { "type": "stdio", "command": "uv", "args": ["..."] },
        "tool": "claude_code_query",
        "arguments": {
          "prompt": "Iteration: ${$.loop.iteration}\nPrevious result: ${$.loop.previous.improve.output}\n\n1. Run tests\n2. Fix failures\n3. Output { \"all_tests_passed\": true } when done",
          "options": {
            "max_turns": 50,
            "allowed_tools": ["Read", "Write", "Edit", "Bash"]
          }
        }
      }
    }
  ]
}
```

#### 適切なユースケース

**推奨：**
- テスト駆動開発（テスト実行→失敗修正→再実行）
- コード品質改善（lint→修正→再lint）
- 段階的な機能実装（実装→テスト→改善）
- 自動検証可能なタスク

**非推奨：**
- 人間の判断が必要なタスク
- 明確な完了条件がないタスク
- 本番環境でのデバッグ

#### サンプル

`samples/sample_ralph_loop.json` を参照してください。

### タイムアウト機能

タスクの実行時間を制限してハングアップを防ぐことができます。

#### 設定方法

```json
{
  "config": {
    "max_concurrent_tasks": 4,
    "default_task_timeout_secs": 300
  },
  "tasks": [
    {
      "task_id": "quick_task",
      "executor": "log"
    },
    {
      "task_id": "long_task",
      "executor": "mcp",
      "timeout_secs": 900
    }
  ]
}
```

#### タイムアウト設定

| 設定場所 | フィールド | 説明 |
|---------|-----------|------|
| Config | `default_task_timeout_secs` | 全タスク共通のデフォルトタイムアウト（秒） |
| Task | `timeout_secs` | タスク個別のタイムアウト（秒）、Configより優先 |

- タスク個別の`timeout_secs`が設定されている場合、そちらが優先される
- どちらも設定されていない場合、タイムアウトなしで実行される
- タイムアウト時はタスクが失敗（Failed）として扱われる

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
├── samples/                     # サンプルDAGファイル
│   ├── sample_dag.json
│   ├── sample_mcp_dag.json
│   ├── sample_loop.json
│   └── ...
├── task-composer-core/          # コアライブラリ
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs             # 型定義（Task, Role, Config等）
│       ├── path_resolver.rs     # パス参照解決
│       ├── dag/                 # DAG実装
│       ├── task_executor/       # Executor実装
│       └── analysis/            # 静的解析
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

サンプルファイルは `samples/` ディレクトリに配置されています。

| ファイル | 説明 |
|----------|------|
| `samples/sample_minimal.json` | 最小構成（task_idとexecutorのみ） |
| `samples/sample_dag.json` | 基本的なDAG（LogExecutor使用） |
| `samples/sample_mcp_dag.json` | MCP連携によるコード分析・README生成 |
| `samples/sample_embedded_reference.json` | 埋め込み参照（`${...}`）のデモ |
| `samples/sample_mcp_with_role.json` | Role情報をMCPに渡すデモ |
| `samples/sample_mcp_with_hungup_timeout.json` | タイムアウト機能のデモ |
| `samples/sample_if_else.json` | if/else条件付き実行のデモ |
| `samples/sample_subgraph.json` | サブグラフ実行のデモ |
| `samples/sample_nested_subgraph.json` | ネストしたサブグラフのデモ（2レベル） |
| `samples/sample_loop.json` | ループ実行のデモ |
| `samples/sample_ralph_loop.json` | Ralph Loopパターンのデモ（MCP連携） |
| `samples/sample_bash.json` | BashExecutorの基本操作デモ |
| `samples/sample_git.json` | GitExecutorの基本操作デモ |
| `samples/sample_git_all_operations.json` | GitExecutorの全操作デモ |
| `samples/sample_github.json` | GitHubExecutorの基本操作デモ |
| `samples/sample_github_all_operations.json` | GitHubExecutorの全操作デモ |
| `samples/sample_analysis_test.json` | 静的解析のエラー検出テスト用 |
| `samples/sample_error_test.json` | エラー検出テスト用 |
| `samples/large_dag.json` | パフォーマンステスト用（大規模DAG） |
| `samples/huge_dag.json` | パフォーマンステスト用（超大規模DAG） |

## ライセンス

Apache License 2.0

## 開発

このプロジェクトはRust学習を目的としています。詳細は[CLAUDE.md](./CLAUDE.md)を参照してください。
