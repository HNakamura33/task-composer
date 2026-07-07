//! DAG（有向非巡回グラフ）モジュール
//!
//! タスク間の依存関係を管理するためのDAGデータ構造を提供します。
//!
//! # 主な機能
//! - JSONファイルからDAGを読み込み
//! - タスクの依存関係の管理
//! - トポロジカルソートによる実行順序の決定
//! - 循環依存の検出
//!
//! # 使用例
//! ```ignore
//! let json = std::fs::read_to_string("sample_dag.json").unwrap();
//! let dag = DAG::from_json(&json).unwrap();
//!
//! // トポロジカルソートで実行順序を取得
//! let order = dag.topological_sort().unwrap();
//! ```

use crate::checkpoint::{Checkpoint, CheckpointState, CheckpointValidation, compute_dag_hash};
use crate::checkpoint::writer::{CheckpointWriter, JsonCheckpointWriter};
use crate::path_resolver::{ResolveContext, evaluate_condition, extract_referenced_tasks, resolve_inputs};
use crate::task_executor::{
    CheckpointInfo, ExecutionContext, ExecutionResult, ExecutionStatus, ExecutorRegistry,
    TaskExecutor,
};
use crate::types::{Config, LoopConfig, LoopContext, Task};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// JSON読み込み用のDAG構造体
///
/// JSONファイルからDAGを読み込む際の中間構造体です。
#[derive(Deserialize)]
struct DAGJson {
    /// タスクのリスト
    tasks: Vec<Task>,
    /// 設定（オプション）
    #[serde(default)]
    config: Config,
    /// ループ設定（オプション）
    #[serde(default)]
    loop_config: Option<LoopConfig>,
}

/// DAG（有向非巡回グラフ）を表す構造体
///
/// ノード間の依存関係をエッジとして保持し、
/// タスクの実行順序を決定するために使用します。
pub struct DAG {
    /// ノード間のエッジを保持するHashMap
    /// - キー: 始点ノードID
    /// - 値: 終点ノードIDのリスト
    pub edges: HashMap<String, Vec<String>>,

    /// ノード間のエッジを保持するHashMap
    /// - キー: 終点ノードID
    /// - 値: 始点ノードIDのリスト
    pub edges_rev: HashMap<String, Vec<String>>,

    /// ノードを保持するHashMap
    /// - キー: ノードID
    /// - 値: タスク情報
    pub nodes: HashMap<String, Task>,

    // pub task_manager: TaskManager,
    pub registry: Arc<ExecutorRegistry>,

    pub config: Config,

    /// ループ設定
    pub loop_config: Option<LoopConfig>,

    /// 外部入力（サブDAGで親から渡される値）
    pub inputs: Option<serde_json::Value>,
}

/// DAG のデフォルト値
impl Default for DAG {
    fn default() -> Self {
        DAG::new()
    }
}

impl DAG {
    /// 新しい空のDAGを作成する
    ///
    /// # Returns
    /// 空のDAGインスタンス
    ///
    /// # Example
    /// ```
    /// # use task_composer_core::DAG;
    /// let dag = DAG::new();
    /// ```
    pub fn new() -> Self {
        DAG {
            edges: HashMap::new(),
            edges_rev: HashMap::new(),
            nodes: HashMap::new(),
            // task_manager: TaskManager::new(Arc::clone(&registry)),
            registry: Arc::new(ExecutorRegistry::new()),
            config: Config::default(),
            loop_config: None,
            inputs: None,
        }
    }

    /// Executorを登録する
    ///
    /// # Arguments
    /// * `executor` - 登録するExecutor
    ///
    /// # Panics
    /// registryが既に共有されている場合はパニック
    pub fn register_executor(&mut self, executor: Box<dyn TaskExecutor + Send + Sync>) {
        Arc::get_mut(&mut self.registry)
            .expect("Cannot register executor: registry is already shared")
            .register(executor);
    }

    /// ExecutorRegistryを設定する
    ///
    /// サブグラフ実行時など、既存のregistryを共有したい場合に使用します。
    ///
    /// # Arguments
    /// * `registry` - 設定するExecutorRegistry
    pub fn set_registry(&mut self, registry: Arc<ExecutorRegistry>) {
        self.registry = registry;
    }

    /// サブDAGに外部入力を設定する
    ///
    /// 親DAGからサブDAGに値を渡す際に使用します。
    /// サブDAG内では `$.inputs.{field}` 形式で参照できます。
    ///
    /// # Arguments
    /// * `inputs` - 親から渡される入力値
    pub fn set_inputs(&mut self, inputs: serde_json::Value) {
        self.inputs = Some(inputs);
    }

    /// タスクをDAGに追加する
    ///
    /// # Arguments
    /// * `task` - 追加するタスク
    ///
    /// # Example
    /// ```
    /// # use task_composer_core::{DAG, Task};
    /// let mut dag = DAG::new();
    /// let task = Task::default();
    /// dag.add_task(task);
    /// ```
    pub fn add_task(&mut self, task: Task) {
        let task_id = task.task_id.clone();
        self.nodes.insert(task_id.clone(), task);
        self.edges.entry(task_id.clone()).or_insert(vec![]);
        self.edges_rev.entry(task_id).or_insert(vec![]);
    }

    /// 2つのノード間にエッジ（依存関係）を追加する
    ///
    /// # Arguments
    /// * `from` - 始点ノードID（依存元）
    /// * `to` - 終点ノードID（依存先）
    ///
    /// # Example
    /// ```ignore
    /// dag.add_edge("1", "2");  // Task 1 → Task 2
    /// ```
    pub fn add_edge(&mut self, from: &str, to: &str) {
        self.edges
            .entry(from.to_string())
            .or_insert(vec![])
            .push(to.to_string());

        self.edges_rev
            .entry(to.to_string())
            .or_insert(vec![])
            .push(from.to_string());
    }

    /// JSON文字列からDAGを作成する
    ///
    /// タスクのフィールド（`args`, `prompt`, `if`, `else`）内に含まれる
    /// パス参照（`$.{task_id}.output.*`）から依存関係を自動的に解決します。
    /// 明示的に指定された`dependencies`と自動解決された依存関係はマージされます。
    ///
    /// # Arguments
    /// * `json_str` - DAGを定義したJSON文字列
    ///
    /// # Returns
    /// * `Ok(DAG)` - パース成功時
    /// * `Err(serde_json::Error)` - パース失敗時
    ///
    /// # Example
    /// ```ignore
    /// let json = r#"{"tasks": [...]}"#;
    /// let dag = DAG::from_json(json)?;
    /// ```
    ///
    /// # 依存関係の自動解決
    /// 以下のパターンからtask_idを抽出し、dependenciesに自動追加:
    /// - `$.task_id.output.field` - 直接パス参照
    /// - `${$.task_id.output.field}` - 埋め込み参照
    ///
    /// `$.self.*` と `$.loop.*` は自己参照・ループ参照のため除外されます。
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        let dag_json: DAGJson = serde_json::from_str(json_str)?;
        let mut dag = DAG::new();
        dag.config = dag_json.config;
        dag.loop_config = dag_json.loop_config;

        // 全タスクIDを収集（存在確認用）
        let all_task_ids: std::collections::HashSet<String> = dag_json
            .tasks
            .iter()
            .map(|t| t.task_id.clone())
            .collect();

        for mut task in dag_json.tasks {
            let task_id = task.task_id.clone();

            // 依存関係を自動解決
            let implicit_deps = Self::extract_implicit_dependencies(&task);

            // 明示的dependenciesと暗黙的dependenciesをマージ
            for dep in implicit_deps {
                // 存在するタスクIDのみ追加（外部参照や存在しないタスクは除外）
                if all_task_ids.contains(&dep) && !task.dependencies.contains(&dep) {
                    task.dependencies.push(dep);
                }
            }

            let dependencies = task.dependencies.clone();
            dag.add_task(task);

            // 依存関係をエッジとして追加
            for dep in dependencies {
                dag.add_edge(&dep, &task_id);
            }
        }

        Ok(dag)
    }

    /// タスクのフィールドから暗黙的な依存関係を抽出する
    ///
    /// 以下のフィールドからパス参照を検索:
    /// - `args` - タスクの引数
    /// - `prompt` - プロンプト文字列
    /// - `if` - 実行条件
    /// - `else` - else条件
    fn extract_implicit_dependencies(task: &Task) -> std::collections::HashSet<String> {
        let mut deps = std::collections::HashSet::new();

        // argsから抽出
        deps.extend(extract_referenced_tasks(&task.args));

        // promptから抽出
        if let Some(ref prompt) = task.prompt {
            deps.extend(extract_referenced_tasks(&serde_json::Value::String(
                prompt.clone(),
            )));
        }

        // if条件から抽出
        if let Some(ref if_cond) = task.if_condition {
            deps.extend(extract_referenced_tasks(&serde_json::Value::String(
                if_cond.clone(),
            )));
        }

        // else条件から抽出
        if let Some(ref else_cond) = task.else_condition {
            deps.extend(extract_referenced_tasks(&serde_json::Value::String(
                else_cond.clone(),
            )));
        }

        deps
    }

    /// タスクの依存先を取得する
    ///
    /// # Arguments
    /// * `task_id` - タスクID
    ///
    /// # Returns
    /// * `Some(&Vec<String>)` - 依存先のリスト
    /// * `None` - タスクが存在しない場合
    ///
    /// # Example
    /// ```ignore
    /// let deps = dag.get_dependencies("1");
    /// if let Some(dep_list) = deps {
    ///     println!("Task 1 depends on: {:?}", dep_list);
    /// }
    /// ```
    pub fn get_dependencies(&self, task_id: &str) -> Option<&Vec<String>> {
        self.edges.get(task_id)
    }

    /// トポロジカルソートを実行する
    ///
    /// Kahnのアルゴリズムを使用して、依存関係を考慮した
    /// タスクの実行順序を決定します。
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - ソートされたタスクIDのリスト（依存元が先）
    /// * `Err(String)` - 循環が検出された場合
    ///
    /// # Example
    /// ```ignore
    /// let order = dag.topological_sort()?;
    /// for task_id in order {
    ///     println!("Execute: {}", task_id);
    /// }
    /// ```
    ///
    /// # Errors
    /// グラフに循環が含まれている場合、エラーを返します。
    pub fn topological_sort(&self) -> Result<Vec<String>, String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        for node in self.nodes.keys() {
            in_degree.insert(node.clone(), 0);
        }

        for (_from, to_list) in &self.edges {
            for to in to_list {
                *in_degree.get_mut(to).unwrap() += 1;
            }
        }

        let mut queue: Vec<String> = Vec::new();
        Self::init_queue_with_roots(&in_degree, &mut queue);

        let mut result: Vec<String> = Vec::new();

        while !queue.is_empty() {
            let current: String = queue.remove(0);
            result.push(current.clone());

            if let Some(to_list) = self.edges.get(&current) {
                Self::update_in_degrees_and_enqueue(to_list, &mut in_degree, &mut queue);
            }
        }

        if result.len() != self.nodes.len() {
            Err("Graph has at least one cycle".to_string())
        } else {
            Ok(result)
        }
    }

    // pub fn execute(&mut self) -> Result<HashMap<String, ExecutionResult>, String> {

    //     let mut in_degree: HashMap<String, usize> = HashMap::new();
    //     let mut results: HashMap<String, ExecutionResult> = HashMap::new();

    //     for node in self.nodes.keys() {
    //         in_degree.insert(node.clone(), 0);
    //     }

    //     for (_from, to_list) in &self.edges {
    //         for to in to_list {
    //             *in_degree.get_mut(to).unwrap() += 1;
    //         }
    //     }

    //     let mut queue: Vec<String> = Vec::new();

    //     // 入次数が0のノード（ルートノード）をキューに追加
    //     for (node, &degree) in &in_degree {
    //         if degree == 0 {
    //             queue.push(node.clone());
    //         }
    //     }

    //     // キューからノードを取り出して実行
    //     while !queue.is_empty() {
    //         let current: String = queue.remove(0);
    //         let task = self.nodes.get(&current).unwrap().clone();

    //         // 依存タスクの結果を収集して ExecutionContext を作成
    //         let mut previous_results: HashMap<String, ExecutionResult> = HashMap::new();
    //         for dep_id in &task.dependencies {
    //             if let Some(dep_result) = results.get(dep_id) {
    //                 previous_results.insert(dep_id.clone(), dep_result.clone());
    //             }
    //         }

    //         // inputsを解決してargsにマージ
    //         let resolved_inputs = resolve_inputs(&task.inputs, &previous_results)
    //             .map_err(|e| format!("Failed to resolve inputs for task {}: {}", task.task_id, e))?;

    //         let merged_args = merge_json_values(task.args.clone(), resolved_inputs);

    //         let ctx = ExecutionContext {
    //             args: merged_args,
    //             env_vars: HashMap::new(),
    //         };

    //         // タスクを実行
    //         let result = self.task_manager.add_task(task, ctx)?;
    //         results.insert(current.clone(), result);

    //         // 後続ノードの入次数を更新し、0になったらキューに追加
    //         if let Some(to_list) = self.edges.get(&current) {
    //             for to in to_list {
    //                 *in_degree.get_mut(to).unwrap() -= 1;
    //                 if in_degree[to] == 0 {
    //                     queue.push(to.clone());
    //                 }
    //             }
    //         }
    //     }

    //     if results.len() != self.nodes.len() {
    //         Err("Graph has at least one cycle".to_string())
    //     } else {
    //         Ok( results )
    //     }
    //     // 実行ロジックをここに実装
    // }

    /// DAGを非同期で並列実行する
    ///
    /// トポロジカル順序に従ってタスクを実行します。
    /// 依存関係のないタスクは`config.max_concurrent_tasks`の上限まで並列実行されます。
    /// `loop_config`が設定されている場合は、ループ実行を行います。
    ///
    /// # Returns
    /// - `Ok(HashMap<String, ExecutionResult>)`: 全タスクの実行結果
    /// - `Err(String)`: 循環依存がある場合やタスク実行に失敗した場合
    ///
    /// # Example
    /// ```ignore
    /// let mut dag = DAG::from_json(&json)?;
    /// dag.register_executor(Box::new(LogExecutor::new()));
    /// let results = dag.execute_async().await?;
    /// ```
    ///
    /// # Algorithm
    /// カーンのアルゴリズムを使用したトポロジカルソートに基づいて実行:
    /// 1. 入次数が0のタスクをキューに追加
    /// 2. キューからタスクを取り出し、`tokio::spawn`で並列実行
    /// 3. タスク完了時に後続タスクの入次数を減らし、0になったらキューに追加
    /// 4. 全タスク完了まで繰り返す
    pub async fn execute_async(&mut self) -> Result<HashMap<String, ExecutionResult>, String> {
        // ループ設定がある場合はループ実行
        if let Some(loop_config) = self.loop_config.clone() {
            return self.execute_with_loop(loop_config, None, None).await;
        }

        // 通常実行（ループなし）
        self.execute_once(None, None, None, None).await
    }

    /// サブグラフとしてチェックポイント付きでDAGを実行する
    ///
    /// DagExecutorから呼ばれ、サブグラフのループイテレーション履歴を
    /// 親のチェックポイントに保存します。
    ///
    /// # Arguments
    ///
    /// * `checkpoint` - 親DAGのチェックポイント
    /// * `writer` - チェックポイントライター
    /// * `subgraph_task_id` - サブグラフの親タスクID
    pub async fn execute_as_subgraph(
        &mut self,
        checkpoint: Arc<Mutex<Checkpoint>>,
        writer: Arc<Box<dyn CheckpointWriter>>,
        subgraph_task_id: &str,
    ) -> Result<HashMap<String, ExecutionResult>, String> {
        if let Some(loop_config) = self.loop_config.clone() {
            return self.execute_with_loop(
                loop_config,
                Some((checkpoint, writer, None)),
                Some(subgraph_task_id),
            ).await;
        }

        // ループなしの場合は通常実行（サブグラフとして実行）
        self.execute_once(None, None, None, Some(subgraph_task_id)).await
    }

    /// チェックポイント付きでDAGを実行する
    ///
    /// 実行中の状態をチェックポイントファイルに保存し、
    /// 中断・失敗時から再開可能にします。
    ///
    /// # Arguments
    ///
    /// * `dag_file` - DAGファイルのパス（チェックポイント検証用）
    /// * `dag_json` - DAG JSONの内容（ハッシュ計算用）
    /// * `checkpoint_path` - チェックポイントファイルのパス（Noneの場合は`{dag_file}.checkpoint.json`）
    /// * `shutdown_signal` - シャットダウンシグナル（Ctrl+C等で設定）
    ///
    /// # Returns
    ///
    /// 実行結果と最終チェックポイント状態のタプル
    pub async fn execute_with_checkpoint(
        &mut self,
        dag_file: &str,
        dag_json: &str,
        checkpoint_path: Option<&Path>,
        shutdown_signal: Option<Arc<AtomicBool>>,
    ) -> Result<(HashMap<String, ExecutionResult>, CheckpointState), String> {
        let dag_hash = compute_dag_hash(dag_json);

        // チェックポイントライターを作成
        let writer: Box<dyn CheckpointWriter> = match checkpoint_path {
            Some(path) => Box::new(JsonCheckpointWriter::new(path)),
            None => Box::new(JsonCheckpointWriter::from_dag_file(dag_file)),
        };

        // 既存のチェックポイントを読み込み
        let existing_checkpoint = writer.load().map_err(|e| {
            format!("Failed to load checkpoint: {}", e)
        })?;

        // チェックポイントがあれば検証
        let (mut checkpoint, initial_results) = if let Some(cp) = existing_checkpoint {
            let task_ids: Vec<&str> = self.nodes.keys().map(|s| s.as_str()).collect();
            match cp.validate(&dag_hash, &task_ids) {
                CheckpointValidation::Valid => {
                    eprintln!("[Checkpoint] Resuming from checkpoint: {} completed tasks", cp.completed_count());
                    let results = cp.to_previous_results();
                    (cp, Some(results))
                }
                CheckpointValidation::DagModified => {
                    eprintln!("[Checkpoint] Warning: DAG has been modified since checkpoint was created");
                    eprintln!("[Checkpoint] Starting fresh execution");
                    (Checkpoint::new(dag_file, &dag_hash), None)
                }
                CheckpointValidation::TaskRemoved(task_id) => {
                    eprintln!("[Checkpoint] Warning: Task '{}' was removed from DAG", task_id);
                    eprintln!("[Checkpoint] Starting fresh execution");
                    (Checkpoint::new(dag_file, &dag_hash), None)
                }
                CheckpointValidation::VersionMismatch { expected, actual } => {
                    eprintln!("[Checkpoint] Warning: Version mismatch (expected {}, got {})", expected, actual);
                    eprintln!("[Checkpoint] Starting fresh execution");
                    (Checkpoint::new(dag_file, &dag_hash), None)
                }
            }
        } else {
            (Checkpoint::new(dag_file, &dag_hash), None)
        };

        // ループ設定がある場合はループ実行（チェックポイント付き）
        if let Some(loop_config) = self.loop_config.clone() {
            let checkpoint_wrapper = Arc::new(Mutex::new(checkpoint));
            let writer_wrapper: Arc<Box<dyn CheckpointWriter>> = Arc::new(writer);
            let results = self.execute_with_loop(
                loop_config,
                Some((Arc::clone(&checkpoint_wrapper), Arc::clone(&writer_wrapper), shutdown_signal)),
                None,
            ).await?;
            let mut cp = checkpoint_wrapper.lock().unwrap();
            cp.set_state(CheckpointState::Completed);
            writer_wrapper.save(&cp).map_err(|e| format!("Failed to save checkpoint: {}", e))?;
            return Ok((results, CheckpointState::Completed));
        }

        // チェックポイントラッパーを作成
        let checkpoint_wrapper = Arc::new(Mutex::new(checkpoint));
        let writer_wrapper = Arc::new(writer);

        // 実行
        let result = self.execute_once(
            None,
            initial_results,
            Some((Arc::clone(&checkpoint_wrapper), Arc::clone(&writer_wrapper), shutdown_signal)),
            None,
        ).await;

        // 最終状態を取得
        let final_checkpoint = checkpoint_wrapper.lock().unwrap().clone();

        match result {
            Ok(results) => {
                // 成功時は完了状態を保存
                let mut cp = checkpoint_wrapper.lock().unwrap();
                cp.set_state(CheckpointState::Completed);
                writer_wrapper.save(&cp).map_err(|e| format!("Failed to save checkpoint: {}", e))?;
                Ok((results, CheckpointState::Completed))
            }
            Err(e) => {
                // 失敗時の状態を保存
                let current_state = final_checkpoint.state.clone();
                Ok((final_checkpoint.to_previous_results(), current_state))
            }
        }
    }

    /// DAGを1回実行する（内部メソッド）
    ///
    /// loop_contextがSomeの場合、$.loop.*参照が利用可能になります。
    ///
    /// # Arguments
    ///
    /// * `loop_context` - ループコンテキスト（ループ実行時）
    /// * `initial_results` - 初期結果（再開時に使用）
    /// * `checkpoint_info` - チェックポイント情報（チェックポイント、ライター、シャットダウンシグナル）
    /// * `subgraph_task_id` - サブグラフの親タスクID（サブグラフ実行時のみ）
    async fn execute_once(
        &mut self,
        loop_context: Option<&LoopContext>,
        initial_results: Option<HashMap<String, ExecutionResult>>,
        checkpoint_info: Option<(
            Arc<Mutex<Checkpoint>>,
            Arc<Box<dyn CheckpointWriter>>,
            Option<Arc<AtomicBool>>,
        )>,
        subgraph_task_id: Option<&str>,
    ) -> Result<HashMap<String, ExecutionResult>, String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        for node in self.nodes.keys() {
            in_degree.insert(node.clone(), 0);
        }

        for (_from, to_list) in &self.edges {
            for to in to_list {
                *in_degree.get_mut(to).unwrap() += 1;
            }
        }

        let mut queue: Vec<String> = Vec::new();

        // 入次数が0のノード（ルートノード）をキューに追加
        for (node, &degree) in &in_degree {
            if degree == 0 {
                queue.push(node.clone());
            }
        }

        // DEBUG: 初期状態を出力
        eprintln!("DEBUG: Initial in_degree: {:?}", in_degree);
        eprintln!("DEBUG: Initial queue: {:?}", queue);

        // 結果マップを初期化（再開時は完了済み結果を設定）
        let results: Arc<Mutex<HashMap<String, ExecutionResult>>> = Arc::new(Mutex::new(
            initial_results.unwrap_or_default()
        ));

        // 再開時: 完了済みタスクの入次数を調整し、後続タスクをキューに追加
        if !results.lock().unwrap().is_empty() {
            let completed_tasks: Vec<String> = results.lock().unwrap().keys().cloned().collect();
            eprintln!("[Resume] Skipping {} completed tasks", completed_tasks.len());

            for task_id in &completed_tasks {
                // 完了済みタスクの後続タスクの入次数を減らす
                if let Some(to_list) = self.edges.get(task_id) {
                    for to in to_list {
                        if let Some(degree) = in_degree.get_mut(to) {
                            if *degree > 0 {
                                *degree -= 1;
                            }
                        }
                    }
                }
            }

            // キューを再構築（入次数0かつ未完了のタスク）
            queue.clear();
            for (node, &degree) in &in_degree {
                if degree == 0 && !completed_tasks.contains(node) {
                    queue.push(node.clone());
                }
            }
            eprintln!("[Resume] Tasks ready to execute: {:?}", queue);
        }

        let (tx, mut rx) = mpsc::channel::<(String, Result<ExecutionResult, String>)>(100);

        let mut running_tasks = 0;
        let mut failed_count: usize = 0;

        // チェックポイント保存用のヘルパー関数
        let subgraph_id = subgraph_task_id.map(|s| s.to_string());
        let save_checkpoint = |cp: &Arc<Mutex<Checkpoint>>, writer: &Arc<Box<dyn CheckpointWriter>>, result: &ExecutionResult| {
            let mut checkpoint = cp.lock().unwrap();
            // サブグラフ実行時は親タスクのcurrent_iteration.tasksに保存
            if let Some(ref sg_id) = subgraph_id {
                checkpoint.update_subgraph_task(sg_id, &result.task_id, result);
            } else {
                checkpoint.update_task(&result.task_id, result);
            }
            if let Err(e) = writer.save(&checkpoint) {
                eprintln!("[Checkpoint] Warning: Failed to save checkpoint: {}", e);
            }
        };

        loop {
            // シャットダウンシグナルのチェック
            if let Some((ref checkpoint, ref writer, ref shutdown_signal)) = checkpoint_info {
                if let Some(signal) = shutdown_signal {
                    if signal.load(Ordering::SeqCst) {
                        eprintln!("[Checkpoint] Shutdown signal received, saving checkpoint...");
                        let mut cp = checkpoint.lock().unwrap();
                        cp.set_state(CheckpointState::Interrupted);
                        if let Err(e) = writer.save(&cp) {
                            eprintln!("[Checkpoint] Warning: Failed to save checkpoint: {}", e);
                        }
                        return Err("Execution interrupted by shutdown signal".to_string());
                    }
                }
            }

            while running_tasks < self.config.max_concurrent_tasks {
                let Some(task_id) = queue.pop() else {
                    break;
                };
                eprintln!("DEBUG: Processing task from queue: {}", task_id);
                let task = self.nodes.get(&task_id).unwrap().clone();
                let tx = tx.clone();
                let results_clone = Arc::clone(&results);

                let mut previous_results: HashMap<String, ExecutionResult> = HashMap::new();
                for dep_id in &task.dependencies {
                    if let Some(dep_result) = results_clone.lock().unwrap().get(dep_id) {
                        previous_results.insert(dep_id.clone(), dep_result.clone());
                    }
                }

                // スキップ判定: 依存先がスキップされていたらこのタスクもスキップ
                let dependency_skipped = task.dependencies.iter().any(|dep_id| {
                    previous_results
                        .get(dep_id)
                        .map(|r| r.status == ExecutionStatus::Skipped)
                        .unwrap_or(false)
                });

                if dependency_skipped {
                    println!("  [Task {} skipped due to dependency skip]", task_id);
                    let skip_result = ExecutionResult {
                        task_id: task_id.clone(),
                        status: ExecutionStatus::Skipped,
                        output: serde_json::json!({"reason": "dependency_skipped"}),
                    };
                    results.lock().unwrap().insert(task_id.clone(), skip_result);

                    // 後続タスクの入次数を更新
                    if let Some(to_list) = self.edges.get(&task_id) {
                        for to in to_list {
                            *in_degree.get_mut(to).unwrap() -= 1;
                            if in_degree[to] == 0 {
                                queue.push(to.clone());
                            }
                        }
                    }
                    continue;
                }

                // argsとinputsの参照を解決してマージ
                let resolve_ctx = ResolveContext {
                    previous_results: &previous_results,
                    current_task: Some(&task),
                    loop_context,
                    inputs: self.inputs.as_ref(),
                    map_context: None,
                    reduce_context: None,
                };

                // if/else条件の評価
                let should_skip = self.evaluate_skip_condition(&task, &resolve_ctx)?;

                if should_skip {
                    println!("  [Task {} skipped due to condition]", task_id);
                    let skip_result = ExecutionResult {
                        task_id: task_id.clone(),
                        status: ExecutionStatus::Skipped,
                        output: serde_json::json!({"reason": "condition_not_met"}),
                    };
                    results.lock().unwrap().insert(task_id.clone(), skip_result);

                    // 後続タスクの入次数を更新
                    if let Some(to_list) = self.edges.get(&task_id) {
                        for to in to_list {
                            *in_degree.get_mut(to).unwrap() -= 1;
                            if in_degree[to] == 0 {
                                queue.push(to.clone());
                            }
                        }
                    }
                    continue;
                }

                // argsを解決（パス参照 $.task.output.* 等を解決）
                let resolved_args = resolve_inputs(&task.args, &resolve_ctx).map_err(|e| {
                    format!("Failed to resolve args for task {}: {}", task.task_id, e)
                })?;

                // DagExecutor用にチェックポイント情報を準備
                let ctx_checkpoint_info = if task.executor == "dag" {
                    checkpoint_info.as_ref().map(|(cp, w, _)| {
                        CheckpointInfo {
                            checkpoint: Arc::clone(cp),
                            writer: Arc::clone(w),
                        }
                    })
                } else {
                    None
                };

                let ctx = ExecutionContext {
                    args: resolved_args,
                    env_vars: HashMap::new(),
                    previous_results: Some(previous_results.clone()),
                    checkpoint_info: ctx_checkpoint_info,
                };

                let registry = Arc::clone(&self.registry);

                // タイムアウト値を決定（タスク個別 > Config デフォルト）
                let timeout_secs = task
                    .timeout_secs
                    .or(self.config.default_task_timeout_secs);

                tokio::spawn(async move {
                    let executor = match registry.get(&task.executor) {
                        Some(exec) => exec,
                        None => {
                            let error = format!("Executor not found: {}", task.executor);
                            eprintln!("Task {} failed: {}", task_id, error);
                            let _ = tx.send((task_id, Err(error))).await;
                            return;
                        }
                    };

                    // タイムアウト付きで実行
                    let result = if let Some(secs) = timeout_secs {
                        match timeout(Duration::from_secs(secs), executor.execute_task(&task, &ctx))
                            .await
                        {
                            Ok(inner_result) => inner_result,
                            Err(_) => Err(format!(
                                "Task '{}' timed out after {} seconds",
                                task_id, secs
                            )),
                        }
                    } else {
                        executor.execute_task(&task, &ctx).await
                    };

                    match result {
                        Ok(exec_result) => {
                            let _ = tx.send((task_id, Ok(exec_result))).await;
                        }
                        Err(e) => {
                            eprintln!("Task {} failed: {}", task_id, e);
                            let _ = tx.send((task_id, Err(e))).await;
                        }
                    }
                });
                // Note: この行はspawn直後に実行される（タスク完了を待たない）
                running_tasks += 1;
            }

            if running_tasks == 0 && queue.is_empty() {
                break;
            }

            if let Some((task_id, result)) = rx.recv().await {
                running_tasks -= 1;

                match result {
                    Ok(exec_result) => {
                        eprintln!("DEBUG: Task {} completed successfully", task_id);
                        results.lock().unwrap().insert(task_id.clone(), exec_result.clone());

                        // チェックポイント保存（成功時）
                        if let Some((ref checkpoint, ref writer, _)) = checkpoint_info {
                            save_checkpoint(checkpoint, writer, &exec_result);
                        }

                        if let Some(to_list) = self.edges.get(&task_id) {
                            eprintln!("DEBUG: Task {} has successors: {:?}", task_id, to_list);
                            for to in to_list {
                                *in_degree.get_mut(to).unwrap() -= 1;
                                eprintln!("DEBUG: {} in_degree now = {}", to, in_degree[to]);
                                if in_degree[to] == 0 {
                                    eprintln!("DEBUG: Adding {} to queue", to);
                                    queue.push(to.clone());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Task {} failed: {}", task_id, e);
                        // 失敗したタスクもresultsに追加（二重カウント防止）
                        let failed_result = ExecutionResult {
                            task_id: task_id.clone(),
                            status: ExecutionStatus::Failed,
                            output: serde_json::json!({"error": e.clone()}),
                        };
                        results.lock().unwrap().insert(task_id.clone(), failed_result.clone());
                        failed_count += 1;

                        // チェックポイント保存（失敗時）
                        if let Some((ref checkpoint, ref writer, _)) = checkpoint_info {
                            let mut cp = checkpoint.lock().unwrap();
                            // サブグラフ実行時は親タスクのcurrent_iteration.tasksに保存
                            if let Some(ref sg_id) = subgraph_id {
                                cp.update_subgraph_task(sg_id, &task_id, &failed_result);
                            } else {
                                cp.update_task(&task_id, &failed_result);
                            }
                            cp.set_state(CheckpointState::Failed {
                                failed_task: task_id,
                                error: e,
                            });
                            if let Err(err) = writer.save(&cp) {
                                eprintln!("[Checkpoint] Warning: Failed to save checkpoint: {}", err);
                            }
                        }
                    }
                }
            }
        }

        let results_map = results.lock().unwrap().clone();
        let success_count = results_map
            .values()
            .filter(|r| r.status == ExecutionStatus::Success)
            .count();
        let skipped_count = results_map
            .values()
            .filter(|r| r.status == ExecutionStatus::Skipped)
            .count();

        // failedはresultsに含まれないので、ノード総数から差し引く
        let actual_failed = self.nodes.len() - results_map.len();

        if actual_failed > 0 || failed_count > 0 {
            Err(format!(
                "{} task(s) failed, {} task(s) skipped",
                failed_count + actual_failed,
                skipped_count
            ))
        } else {
            println!(
                "  [Execution complete: {} succeeded, {} skipped]",
                success_count, skipped_count
            );
            Ok(results_map)
        }
    }

    /// ループ実行を行う
    ///
    /// DAGを繰り返し実行し、条件に基づいてループを制御します。
    /// 前回イテレーションの結果は`$.loop.previous.*`で参照可能です。
    ///
    /// チェックポイントが有効な場合、各イテレーション完了時に結果を保存し、
    /// 中断時に完了済みイテレーションからの再開が可能です。
    ///
    /// # Arguments
    /// * `config` - ループ設定
    /// * `checkpoint_info` - チェックポイント情報（チェックポイント、ライター、シャットダウンシグナル）
    /// * `subgraph_task_id` - サブグラフの親タスクID（トップレベルループの場合はNone）
    ///
    /// # Returns
    /// - `Ok(HashMap<String, ExecutionResult>)`: 最後のイテレーションの実行結果
    /// - `Err(String)`: 実行エラー
    async fn execute_with_loop(
        &mut self,
        config: LoopConfig,
        checkpoint_info: Option<(
            Arc<Mutex<Checkpoint>>,
            Arc<Box<dyn CheckpointWriter>>,
            Option<Arc<AtomicBool>>,
        )>,
        subgraph_task_id: Option<&str>,
    ) -> Result<HashMap<String, ExecutionResult>, String> {
        let mut iteration = 0;
        let mut previous_results: Option<HashMap<String, serde_json::Value>> = None;
        let mut results: HashMap<String, ExecutionResult>;
        let mut in_progress_tasks: Option<HashMap<String, ExecutionResult>> = None;

        // チェックポイントからの再開チェック
        if let Some((ref cp_arc, _, _)) = checkpoint_info {
            let cp = cp_arc.lock().unwrap();

            // 完了済みイテレーション数を確認
            let completed = match subgraph_task_id {
                Some(task_id) => cp.completed_subgraph_iterations(task_id),
                None => cp.completed_loop_iterations(),
            };

            // 進行中イテレーションがあるかチェック（サブグラフのみ）
            let in_progress = if let Some(task_id) = subgraph_task_id {
                cp.get_in_progress_iteration(task_id)
            } else {
                None
            };

            if let Some(in_prog) = in_progress {
                // 進行中イテレーションから再開
                eprintln!(
                    "[Checkpoint] Resuming from in-progress iteration {} ({} tasks completed)",
                    in_prog.iteration, in_prog.tasks.len()
                );
                iteration = in_prog.iteration;

                // 完了済みタスクをExecutionResultに変換
                in_progress_tasks = Some(
                    in_prog.tasks.iter()
                        .filter(|(_, tc)| tc.status == ExecutionStatus::Success)
                        .map(|(k, tc)| {
                            (k.clone(), ExecutionResult {
                                task_id: k.clone(),
                                status: tc.status.clone(),
                                output: tc.output.clone(),
                            })
                        })
                        .collect()
                );

                // 前回イテレーションの結果を取得（iteration > 0の場合）
                if iteration > 0 {
                    previous_results = match subgraph_task_id {
                        Some(task_id) => cp.last_subgraph_previous_results(task_id),
                        None => cp.last_loop_previous_results(),
                    };
                }
            } else if completed > 0 {
                // 完了済みイテレーションから再開（次のイテレーションを開始）
                eprintln!(
                    "[Checkpoint] Resuming loop from iteration {} ({} completed)",
                    completed, completed
                );
                iteration = completed;
                previous_results = match subgraph_task_id {
                    Some(task_id) => cp.last_subgraph_previous_results(task_id),
                    None => cp.last_loop_previous_results(),
                };
            }
        }

        println!(
            "  [Loop execution started: max_iterations={}, starting_at={}]",
            config.max_iterations, iteration
        );

        loop {
            // 最大回数チェック（再開時にすでに到達している可能性）
            if iteration >= config.max_iterations {
                // 再開時にすでに完了している場合、最後のイテレーション結果を返す
                if let Some(prev) = &previous_results {
                    results = prev
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                ExecutionResult {
                                    task_id: k.clone(),
                                    status: ExecutionStatus::Success,
                                    output: v.clone(),
                                },
                            )
                        })
                        .collect();
                    break;
                }
                println!("  [Loop terminated: max_iterations already reached]");
                return Ok(HashMap::new());
            }

            // LoopContextを構築
            let loop_context = LoopContext {
                iteration,
                first: iteration == 0,
                previous_results: previous_results.clone(),
            };

            println!(
                "  [Loop iteration {} (first={})]",
                iteration, loop_context.first
            );

            // 進行中タスクからの再開でない場合のみイテレーション開始を記録
            if in_progress_tasks.is_none() {
                if let Some((ref cp_arc, ref writer, _)) = checkpoint_info {
                    let mut cp = cp_arc.lock().unwrap();
                    if let Some(task_id) = subgraph_task_id {
                        cp.start_subgraph_iteration(task_id, iteration);
                    } else {
                        cp.tasks.clear();
                    }
                    if let Err(e) = writer.save(&cp) {
                        eprintln!("[Checkpoint] Warning: Failed to save iteration start: {}", e);
                    }
                }
            }

            // LoopContextを渡して実行
            // 進行中タスクがあれば初期結果として渡し、使用後はクリア
            let initial_results_for_iteration = in_progress_tasks.take();
            results = self.execute_once(
                Some(&loop_context),
                initial_results_for_iteration,
                checkpoint_info.as_ref().map(|(cp, w, s)| {
                    (Arc::clone(cp), Arc::clone(w), s.clone())
                }),
                subgraph_task_id,
            ).await?;

            // イテレーション結果をチェックポイントに保存
            if let Some((ref cp_arc, ref writer, _)) = checkpoint_info {
                let mut cp = cp_arc.lock().unwrap();
                match subgraph_task_id {
                    Some(task_id) => {
                        cp.complete_subgraph_iteration(task_id, &results);
                    }
                    None => cp.add_loop_iteration(&results),
                }
                if let Err(e) = writer.save(&cp) {
                    eprintln!("[Checkpoint] Warning: Failed to save loop iteration: {}", e);
                }
            }

            iteration += 1;

            // 最大回数チェック
            if iteration >= config.max_iterations {
                println!("  [Loop terminated: max_iterations reached]");
                break;
            }

            // 条件評価用のコンテキストを作成
            let eval_ctx = ResolveContext {
                previous_results: &results,
                current_task: None,
                loop_context: Some(&loop_context),
                inputs: self.inputs.as_ref(),
                map_context: None,
                reduce_context: None,
            };

            // while条件チェック（falseなら終了）
            if let Some(ref cond) = config.while_condition {
                let should_continue = evaluate_condition(cond, &eval_ctx)
                    .map_err(|e| format!("Failed to evaluate while_condition: {}", e))?;
                if !should_continue {
                    println!("  [Loop terminated: while_condition became false]");
                    break;
                }
            }

            // until条件チェック（trueなら終了）
            if let Some(ref cond) = config.until_condition {
                let should_stop = evaluate_condition(cond, &eval_ctx)
                    .map_err(|e| format!("Failed to evaluate until_condition: {}", e))?;
                if should_stop {
                    println!("  [Loop terminated: until_condition became true]");
                    break;
                }
            }

            // 今回の結果を次回の previous_results として保存
            previous_results = Some(
                results
                    .iter()
                    .map(|(k, v)| (k.clone(), v.output.clone()))
                    .collect(),
            );
        }

        println!("  [Loop execution complete: {} iterations]", iteration);
        Ok(results)
    }

    /// if/else条件を評価してスキップするかどうかを判定する
    ///
    /// # 実行ルール
    /// | フィールド | 条件結果 | タスク実行 |
    /// |-----------|---------|-----------|
    /// | なし | - | 実行 |
    /// | `if` | true | 実行 |
    /// | `if` | false | スキップ |
    /// | `else` | true | スキップ |
    /// | `else` | false | 実行 |
    fn evaluate_skip_condition(&self, task: &Task, ctx: &ResolveContext) -> Result<bool, String> {
        // if条件がある場合
        if let Some(ref if_cond) = task.if_condition {
            let result = evaluate_condition(if_cond, ctx).map_err(|e| {
                format!(
                    "Failed to evaluate if condition for task {}: {}",
                    task.task_id, e
                )
            })?;
            // ifがfalseならスキップ
            return Ok(!result);
        }

        // else条件がある場合
        if let Some(ref else_cond) = task.else_condition {
            let result = evaluate_condition(else_cond, ctx).map_err(|e| {
                format!(
                    "Failed to evaluate else condition for task {}: {}",
                    task.task_id, e
                )
            })?;
            // elseがtrueならスキップ
            return Ok(result);
        }

        // 条件なし → 実行する（スキップしない）
        Ok(false)
    }

    /// 入次数が0のノード（ルートノード）をキューに追加する
    ///
    /// トポロジカルソートの初期化時に使用するヘルパーメソッド。
    /// 依存関係を持たない（入次数が0の）ノードを処理待ちキューに追加します。
    ///
    /// # Arguments
    /// * `in_degree` - 各ノードの入次数を保持するHashMap
    /// * `queue` - 処理待ちノードのキュー
    fn init_queue_with_roots(in_degree: &HashMap<String, usize>, queue: &mut Vec<String>) {
        for (node, &degree) in in_degree {
            if degree == 0 {
                queue.push(node.clone());
            }
        }
    }

    /// 後続ノードの入次数を更新し、入次数が0になったノードをキューに追加する
    ///
    /// トポロジカルソートのKahnアルゴリズムで使用するヘルパーメソッド。
    /// 処理済みノードの後続ノードに対して、入次数を1減らし、
    /// 入次数が0になったノードを処理待ちキューに追加します。
    ///
    /// # Arguments
    /// * `to_list` - 後続ノードIDのリスト
    /// * `in_degree` - 各ノードの入次数を保持するHashMap
    /// * `queue` - 処理待ちノードのキュー
    fn update_in_degrees_and_enqueue(
        to_list: &[String],
        in_degree: &mut HashMap<String, usize>,
        queue: &mut Vec<String>,
    ) {
        for to in to_list {
            *in_degree.get_mut(to).unwrap() -= 1;
            if in_degree[to] == 0 {
                queue.push(to.clone());
            }
        }
    }

    /// 全ノードの子孫集合を計算する
    ///
    /// トポロジカル順序の逆順で処理することで、
    /// 動的計画法により効率的に全ノードの子孫を計算します。
    ///
    /// # Returns
    /// * `Ok(HashMap)` - 各ノードIDをキーとし、その子孫のノードID集合を値とするHashMap
    /// * `Err(String)` - グラフに循環が含まれている場合
    ///
    /// # Example
    /// ```ignore
    /// let descendants = dag.compute_all_descendants()?;
    /// let desc_1 = descendants.get("1").unwrap();
    /// // desc_1 には "1" から到達可能な全ノードIDが含まれる
    /// ```
    ///
    /// # Errors
    /// グラフに循環が含まれている場合、エラーを返します。
    pub fn compute_all_descendants(&self) -> Result<HashMap<String, HashSet<String>>, String> {
        let sorted_nodes = self.topological_sort()?;
        let mut descendants: HashMap<String, HashSet<String>> = HashMap::new();

        for node in sorted_nodes.iter().rev() {
            let mut node_descendants: HashSet<String> = HashSet::new();
            if let Some(children) = self.edges.get(node) {
                for child in children {
                    node_descendants.insert(child.clone());
                    if let Some(child_descendants) = descendants.get(child) {
                        for desc in child_descendants {
                            node_descendants.insert(desc.clone());
                        }
                    }
                }
            }
            descendants.insert(node.clone(), node_descendants);
        }
        Ok(descendants)
    }

    /// 全ノードの祖先集合を計算する
    ///
    /// トポロジカル順序の正順で処理することで、
    /// 動的計画法により効率的に全ノードの祖先を計算します。
    ///
    /// # Returns
    /// * `Ok(HashMap)` - 各ノードIDをキーとし、その祖先のノードID集合を値とするHashMap
    /// * `Err(String)` - グラフに循環が含まれている場合
    ///
    /// # Example
    /// ```ignore
    /// let ancestors = dag.compute_all_ancestors()?;
    /// let anc_4 = ancestors.get("4").unwrap();
    /// // anc_4 には "4" に到達可能な全ノードIDが含まれる
    /// ```
    ///
    /// # Errors
    /// グラフに循環が含まれている場合、エラーを返します。
    pub fn compute_all_ancestors(&self) -> Result<HashMap<String, HashSet<String>>, String> {
        let sorted_nodes: Vec<String> = self.topological_sort()?;
        let mut ancestors: HashMap<String, HashSet<String>> = HashMap::new();

        for node in sorted_nodes.iter() {
            let mut node_ancestors: HashSet<String> = HashSet::new();
            if let Some(parents) = self.edges_rev.get(node) {
                for parent in parents {
                    node_ancestors.insert(parent.clone());
                    if let Some(parent_ancestors) = ancestors.get(parent) {
                        for anc in parent_ancestors {
                            node_ancestors.insert(anc.clone());
                        }
                    }
                }
            }
            ancestors.insert(node.clone(), node_ancestors);
        }

        Ok(ancestors)
    }

    /// 全ての並行実行可能なノードペアを取得する
    ///
    /// 2つのノードが並行実行可能とは、互いに依存関係がないことを意味します。
    /// つまり、どちらも他方の祖先でも子孫でもない場合、並行実行可能です。
    ///
    /// # Returns
    /// * `Ok(Vec)` - 並行実行可能なノードペアのリスト（重複なし、A < B の辞書順）
    /// * `Err(String)` - グラフに循環が含まれている場合
    ///
    /// # Example
    /// ```ignore
    /// let pairs = dag.get_all_parallel_pairs()?;
    /// for (a, b) in pairs {
    ///     println!("{} と {} は並行実行可能", a, b);
    /// }
    /// ```
    ///
    /// # Errors
    /// グラフに循環が含まれている場合、エラーを返します。
    pub fn get_all_parallel_pairs(&self) -> Result<Vec<(String, String)>, String> {
        let descendants: HashMap<String, HashSet<String>> = self.compute_all_descendants()?;
        let ancestors: HashMap<String, HashSet<String>> = self.compute_all_ancestors()?;

        let mut pairs: Vec<(String, String)> = Vec::new();

        for (node_id_desc, node_desc) in descendants {
            if let Some(node_anc) = ancestors.get(&node_id_desc) {
                for node_id_anc in ancestors.keys() {
                    if node_id_desc >= node_id_anc.clone() {
                        continue;
                    }
                    if !node_desc.contains(&node_id_anc.clone())
                        && !node_anc.contains(&node_id_anc.clone())
                    {
                        pairs.push((node_id_desc.clone(), node_id_anc.clone()));
                    }
                }
            }
        }

        Ok(pairs)
    }
}

#[cfg(test)]
mod tests;
