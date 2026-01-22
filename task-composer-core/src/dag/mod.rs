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

use crate::path_resolver::{ResolveContext, evaluate_condition, extract_referenced_tasks, resolve_inputs};
use crate::task_executor::{
    ExecutionContext, ExecutionResult, ExecutionStatus, ExecutorRegistry, TaskExecutor,
};
use crate::types::{Config, LoopConfig, LoopContext, Task};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
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
            return self.execute_with_loop(loop_config).await;
        }

        // 通常実行（ループなし）
        self.execute_once(None).await
    }

    /// DAGを1回実行する（内部メソッド）
    ///
    /// loop_contextがSomeの場合、$.loop.*参照が利用可能になります。
    async fn execute_once(
        &mut self,
        loop_context: Option<&LoopContext>,
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

        let results: Arc<Mutex<HashMap<String, ExecutionResult>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (tx, mut rx) = mpsc::channel::<(String, Result<ExecutionResult, String>)>(100);

        let mut running_tasks = 0;
        let mut failed_count: usize = 0;

        loop {
            while running_tasks < self.config.max_concurrent_tasks {
                let Some(task_id) = queue.pop() else {
                    break;
                };
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

                let ctx = ExecutionContext {
                    args: resolved_args,
                    env_vars: HashMap::new(),
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
                        results.lock().unwrap().insert(task_id.clone(), exec_result);

                        if let Some(to_list) = self.edges.get(&task_id) {
                            for to in to_list {
                                *in_degree.get_mut(to).unwrap() -= 1;
                                if in_degree[to] == 0 {
                                    queue.push(to.clone());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Task {} failed: {}", task_id, e);
                        failed_count += 1;
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
    /// # Arguments
    /// * `config` - ループ設定
    ///
    /// # Returns
    /// - `Ok(HashMap<String, ExecutionResult>)`: 最後のイテレーションの実行結果
    /// - `Err(String)`: 実行エラー
    async fn execute_with_loop(
        &mut self,
        config: LoopConfig,
    ) -> Result<HashMap<String, ExecutionResult>, String> {
        let mut iteration = 0;
        let mut previous_results: Option<HashMap<String, serde_json::Value>> = None;
        let mut results: HashMap<String, ExecutionResult>;

        println!(
            "  [Loop execution started: max_iterations={}]",
            config.max_iterations
        );

        loop {
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

            // LoopContextを渡して実行
            results = self.execute_once(Some(&loop_context)).await?;
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
            // ExecutionResult.output をserde_json::Valueとして保存
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
