# Task Composer

DAGベースのタスク管理ライブラリ（Rust学習プロジェクト）

## Claudeへの指示

このプロジェクトはRust学習を目的としています。以下の点を意識してサポートしてください：

- **コードを直接書かない**: ヒントやシグネチャを提示し、ユーザーが自分で実装できるようガイドする
- **概念を説明する**: 新しいRustの概念が出てきたら、図や例を使って分かりやすく説明する
- **エラーを学習機会に**: コンパイルエラーが出たら、なぜそのエラーが起きるのかを丁寧に解説する
- **段階的に進める**: 一度に大量のコードを提示せず、小さなステップで進める
- **質問を促す**: ユーザーが理解を深められるよう、適宜質問を投げかける
- **テストを重視**: 実装後は必ずテストを書くよう促す

## プロジェクト概要

有向非巡回グラフ(DAG)を使ってタスクの依存関係を管理するRustライブラリ。

## 構造

```
task-composer/
├── Cargo.toml                   # ワークスペース定義
├── LICENSE                      # Apache 2.0
├── README.md
├── CLAUDE.md                    # このファイル
├── task-composer-core/          # コアライブラリ
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs             # 型定義（Task, Role, Config等）
│       ├── path_resolver.rs     # パス参照・埋め込み参照の解決
│       ├── dag/                 # DAG実装
│       │   ├── mod.rs
│       │   └── tests.rs
│       ├── task_executor/       # Executor実装
│       │   ├── mod.rs
│       │   ├── log_executor.rs
│       │   ├── mcp_executor.rs
│       │   └── dag_executor.rs
│       └── analysis/            # 静的解析
│           ├── mod.rs
│           ├── dag_analysis.rs
│           ├── task_validation.rs
│           └── conflict/
├── task-composer-cli/           # CLIツール
│   ├── Cargo.toml
│   └── src/main.rs
├── task-composer-ui/            # Dioxus UI（Desktop/Web/TUI）
│   ├── Cargo.toml
│   ├── Dioxus.toml
│   └── src/
├── mcp_servers/
│   └── claude_code_mcp/
│       ├── main.py              # FastMCPサーバー
│       └── pyproject.toml
└── samples/                     # サンプルDAGファイル
    ├── sample_minimal.json      # 最小構成サンプル
    ├── sample_dag.json          # 基本サンプル
    ├── sample_mcp_dag.json      # MCP連携サンプル
    ├── sample_loop.json         # ループ実行サンプル
    └── ...
```

## 主要な構造体

- `Task` - タスク情報（必須: task_id, executor / オプショナル: name, description, priority, prompt, role, dependencies, args, if_condition, else_condition, timeout_secs）
- `Role` - ロール情報（role_id, name, subagents, skills, description, tool_permissions, file_permissions）
- `FilePermission` - ファイル権限（allowed_paths, denied_paths, read_only_paths）
- `ToolPermission` - ツール権限（bash, write）
- `BashPermission` - Bash権限（allowed_commands, blocked_commands, require_confirmation）
- `WritePermission` - 書き込み権限（max_file_size_mb, allowed_extensions）
- `Status` - タスク状態（Pending, InProgress, Completed）
- `DAG` - グラフ本体（nodes, edges, edges_rev, registry, config, loop_config）
- `Config` - 設定（max_concurrent_tasks, default_task_timeout_secs）
- `LoopConfig` - ループ設定（max_iterations, while_condition, until_condition）
- `LoopContext` - ループ実行コンテキスト（iteration, first, previous_results）
- `ExecutionResult` - 実行結果（task_id, status, output）
- `ExecutionStatus` - 実行ステータス（Success, Failed, Skipped）
- `ResolveContext` - パス解決コンテキスト（previous_results, current_task, loop_context）

## 実装済み機能

### DAG基本機能
- `DAG::new()` - 空のDAG作成
- `DAG::add_task()` - タスク追加
- `DAG::add_edge()` - エッジ追加
- `DAG::from_json()` - JSONからDAG作成
- `DAG::get_dependencies()` - タスクの依存先取得
- `DAG::topological_sort()` - トポロジカルソート（Kahnアルゴリズム、循環検出含む）
- `DAG::execute_async()` - 非同期並列実行

### パス参照機能
- `$.{task_id}.output.{field}` - 依存タスクの出力参照
- `$.self.{field}` - 現在のタスクのフィールド参照
- `${...}` - 文字列内への埋め込み参照
- ネストしたフィールド、配列インデックス対応

### 条件付き実行（if/else）
- `if` フィールド - 条件がtrueなら実行、falseならスキップ
- `else` フィールド - 条件がtrueならスキップ、falseなら実行
- スキップ伝播 - 依存先がスキップされると依存元もスキップ
- 条件式: 比較演算（`==`, `!=`, `>`, `<`, `>=`, `<=`）、論理演算（`&&`, `||`, `!`）

### Executor
- `LogExecutor` - デバッグ・テスト用ログ出力
- `McpExecutor` - MCP (Model Context Protocol) 連携
- `DagExecutor` - サブグラフ（入れ子DAG）実行
- `ExecutorRegistry` - Executor管理

### サブグラフ実行
- `args.dag` でサブDAG定義を指定
- ネストしたサブグラフのサポート（最大3レベル）
- サブグラフ内でのパス参照（`$.{task_id}.output.{field}`）
- 親DAGからサブグラフ結果を参照（`$.{subdag_task}.output.{inner_task}.output.{field}`）

### ループ実行
- `loop_config.max_iterations` - 最大繰り返し回数
- `loop_config.while_condition` - 継続条件（trueの間ループ継続）
- `loop_config.until_condition` - 終了条件（trueになったらループ終了）
- `$.loop.iteration` - 現在のイテレーション番号（0始まり）
- `$.loop.first` - 初回かどうか（true/false）
- `$.loop.previous.{task_id}.output.{field}` - 前回イテレーションの結果参照

### タイムアウト機能
- `config.default_task_timeout_secs` - 全タスク共通のデフォルトタイムアウト（秒）
- `task.timeout_secs` - タスク個別のタイムアウト（秒）、Configより優先
- タイムアウト時はタスクが失敗扱いになる

### MCP Server (Python)
- `claude_code_query` - Claude Codeへのクエリ実行
- `options` - Claude Agent SDKオプション
- `extra_options.role` - ロール情報をシステムプロンプトに変換

## コマンド

```bash
# 開発時
cargo test                                  # テスト実行
cargo run -p task-composer-cli              # samples/sample_dag.json を読み込んで実行
cargo run -p task-composer-cli -- file.json # 指定したJSONを実行
cargo doc --open                            # ドキュメント生成

# CLIインストール後
task-composer analyze file.json             # 静的解析のみ
task-composer run file.json                 # 静的解析+実行
task-composer exec file.json                # 実行のみ（静的解析なし）
```

## コーディング規約

- ドキュメントコメント（`///`）を全ての公開関数・構造体に付ける
- テストは同じファイル内の `#[cfg(test)] mod tests` に記述
- エラーハンドリングは `Result<T, E>` を使用
- 非同期関数には `async_trait` を使用

## 学習メモ

- `&str` vs `String`: 関数引数は`&str`が柔軟
- `Option<T>` vs `Result<T, E>`: 値の有無 vs 成功/失敗
- セミコロンなし = 値を返す式
- `#[derive(Deserialize, Serialize)]` でJSON変換
- `async/await` と `tokio::spawn` で並列実行
- `Arc<Mutex<T>>` で共有状態の管理
- `regex::Regex` で文字列パターンマッチ
