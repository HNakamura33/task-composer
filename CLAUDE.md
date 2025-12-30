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
├── Cargo.toml          # 依存関係: serde, serde_json
├── src/
│   └── main.rs         # メインコード（構造体、impl、テスト）
├── sample_dag.json     # サンプルDAG定義ファイル
└── CLAUDE.md           # このファイル
```

## 主要な構造体

- `Task` - タスク情報（task_id, name, description, priority, status, prompt, role, dependencies）
- `Role` - ロール情報（role_id, name, subagents, skills, description, permissions）
- `Status` - タスク状態（Pending, InProgress, Completed）
- `DAG` - グラフ本体（nodes: HashMap, edges: HashMap）
- `DAGJson` - JSON読み込み用中間構造体

## 実装済み機能

- `DAG::new()` - 空のDAG作成
- `DAG::add_task()` - タスク追加
- `DAG::add_edge()` - エッジ追加
- `DAG::from_json()` - JSONからDAG作成
- `DAG::get_dependencies()` - タスクの依存先取得
- 各構造体の`Default`トレイト実装

## 開発中の機能

- `topological_sort()` - トポロジカルソート（循環検出含む）

## コマンド

```bash
cargo test      # テスト実行
cargo run       # sample_dag.json を読み込んで実行
cargo doc --open # ドキュメント生成
```

## コーディング規約

- ドキュメントコメント（`///`）を全ての公開関数・構造体に付ける
- テストは同じファイル内の `#[cfg(test)] mod tests` に記述
- エラーハンドリングは `Result<T, E>` を使用

## 学習メモ

- `&str` vs `String`: 関数引数は`&str`が柔軟
- `Option<T>` vs `Result<T, E>`: 値の有無 vs 成功/失敗
- セミコロンなし = 値を返す式
- `#[derive(Deserialize)]` でJSONから自動変換
