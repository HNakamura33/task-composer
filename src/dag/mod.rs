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
//! ```
//! let json = std::fs::read_to_string("sample_dag.json").unwrap();
//! let dag = DAG::from_json(&json).unwrap();
//!
//! // トポロジカルソートで実行順序を取得
//! let order = dag.topological_sort().unwrap();
//! ```

use std::collections::{HashMap, HashSet};
use serde::Deserialize;
use crate::types::{Task, Config};
use crate::task_executor::{ExecutionContext, ExecutorRegistry, TaskExecutor, ExecutionResult};
use crate::path_resolver::{resolve_inputs, ResolveContext};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;


/// JSON読み込み用のDAG構造体
///
/// JSONファイルからDAGを読み込む際の中間構造体です。
#[derive(Deserialize)]
struct DAGJson {
    /// タスクのリスト
    tasks: Vec<Task>,
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

    config: Config,

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

    /// タスクをDAGに追加する
    ///
    /// # Arguments
    /// * `task` - 追加するタスク
    ///
    /// # Example
    /// ```
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
    /// ```
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
    /// # Arguments
    /// * `json_str` - DAGを定義したJSON文字列
    ///
    /// # Returns
    /// * `Ok(DAG)` - パース成功時
    /// * `Err(serde_json::Error)` - パース失敗時
    ///
    /// # Example
    /// ```
    /// let json = r#"{"tasks": [...]}"#;
    /// let dag = DAG::from_json(json)?;
    /// ```
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        let dag_json: DAGJson = serde_json::from_str(json_str)?;
        let mut dag = DAG::new();

        for task in dag_json.tasks {
            let dependencies = task.dependencies.clone();
            let task_id = task.task_id.clone();

            dag.add_task(task);

            // 依存関係をエッジとして追加
            for dep in dependencies.clone() {
                dag.add_edge(&dep, &task_id);
            }
        }

        Ok(dag)
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
    /// ```
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
    /// ```
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
        
        let results: Arc<Mutex<HashMap<String, ExecutionResult>>> = Arc::new(Mutex::new(HashMap::new()));
        let (tx, mut rx) = mpsc::channel::<(String, ExecutionResult)>(100);

        let mut running_tasks = 0;

        loop {
            while running_tasks < self.config.max_concurrent_tasks {
                let Some(task_id) = queue.pop() else { break; };
                let task = self.nodes.get(&task_id).unwrap().clone();
                let tx = tx.clone();
                let results_clone = Arc::clone(&results);

                let mut previous_results: HashMap<String, ExecutionResult> = HashMap::new();
                for dep_id in &task.dependencies {
                    if let Some(dep_result) = results_clone.lock().unwrap().get(dep_id) {
                        previous_results.insert(dep_id.clone(), dep_result.clone());
                    }
                }

                // inputsを解決してargsにマージ
                let resolve_ctx = ResolveContext {
                    previous_results: &previous_results,
                    current_task: Some(&task),
                };
                let resolved_inputs = resolve_inputs(&task.inputs, &resolve_ctx)
                    .map_err(|e| format!("Failed to resolve inputs for task {}: {}", task.task_id, e))?;

                let merged_args = merge_json_values(task.args.clone(), resolved_inputs);

                let ctx = ExecutionContext {
                    args: merged_args,
                    env_vars: HashMap::new(),
                };

                let registry = Arc::clone(&self.registry);

                tokio::spawn(async move {
                    let executor = registry.get(&task.executor).unwrap();
                    let result = executor.execute_task(&task, &ctx).await;
                    tx.send((task_id, result.unwrap())).await.unwrap();
                });
                // Note: この行はspawn直後に実行される（タスク完了を待たない）
                running_tasks += 1;
            }

            if running_tasks == 0 && queue.is_empty() {
                break;
            }

            
            if let Some((task_id, result)) = rx.recv().await {
                running_tasks -= 1;
                results.lock().unwrap().insert(task_id.clone(), result);
                
                if let Some(to_list) = self.edges.get(&task_id) {
                    for to in to_list {
                        *in_degree.get_mut(to).unwrap() -= 1;
                        if in_degree[to] == 0 {
                            queue.push(to.clone());
                        }
                    }
                }
            }
            

        }


        if results.lock().unwrap().len() != self.nodes.len() {
            Err("Graph has at least one cycle".to_string())
        } else {
            Ok(results.lock().unwrap().clone())
        }
        
    }



    /// 入次数が0のノード（ルートノード）をキューに追加する
    ///
    /// トポロジカルソートの初期化時に使用するヘルパーメソッド。
    /// 依存関係を持たない（入次数が0の）ノードを処理待ちキューに追加します。
    ///
    /// # Arguments
    /// * `in_degree` - 各ノードの入次数を保持するHashMap
    /// * `queue` - 処理待ちノードのキュー
    fn init_queue_with_roots(
        in_degree: &HashMap<String, usize>,
        queue: &mut Vec<String>,
    ) {
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
    /// ```
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
    /// ```
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
    /// ```
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

/// 2つのJSON値をマージする
///
/// - 両方がObjectの場合: キーをマージ（overrideで上書き）
/// - それ以外: overrideがNullでなければoverrideを返す、Nullならbaseを返す
fn merge_json_values(base: serde_json::Value, override_val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;

    match (base, override_val) {
        (Value::Object(mut base_map), Value::Object(override_map)) => {
            for (key, value) in override_map {
                base_map.insert(key, value);
            }
            Value::Object(base_map)
        }
        (base, Value::Null) => base,
        (_, override_val) => override_val,
    }
}

#[cfg(test)]
mod tests;
