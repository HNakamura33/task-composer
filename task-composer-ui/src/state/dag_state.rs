//! DAG状態管理

use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use task_composer_core::types::{Task, Role, Config};

/// UI用のタスクステータス
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UiStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl Default for UiStatus {
    fn default() -> Self {
        UiStatus::Pending
    }
}

impl From<task_composer_core::types::Status> for UiStatus {
    fn from(status: task_composer_core::types::Status) -> Self {
        match status {
            task_composer_core::types::Status::Pending => UiStatus::Pending,
            task_composer_core::types::Status::InProgress => UiStatus::InProgress,
            task_composer_core::types::Status::Completed => UiStatus::Completed,
        }
    }
}

impl From<UiStatus> for task_composer_core::types::Status {
    fn from(status: UiStatus) -> Self {
        match status {
            UiStatus::Pending => task_composer_core::types::Status::Pending,
            UiStatus::InProgress => task_composer_core::types::Status::InProgress,
            UiStatus::Completed => task_composer_core::types::Status::Completed,
            UiStatus::Failed => task_composer_core::types::Status::Pending, // Failedはcoreにないのでデフォルト
        }
    }
}

/// UI用のタスク構造体
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiTask {
    pub task_id: String,
    pub name: String,
    pub description: String,
    pub priority: u8,
    pub status: UiStatus,
    pub prompt: String,
    pub executor: String,
    pub dependencies: Vec<String>,
    pub role: Role,
    pub inputs: serde_json::Value,
    pub args: serde_json::Value,
    /// グラフ上の位置（UI専用）
    pub position: Option<(f32, f32)>,
}

impl From<&Task> for UiTask {
    fn from(task: &Task) -> Self {
        UiTask {
            task_id: task.task_id.clone(),
            name: task.name.clone(),
            description: task.description.clone(),
            priority: task.priority,
            status: task.status.clone().into(),
            prompt: task.prompt.clone(),
            executor: task.executor.clone(),
            dependencies: task.dependencies.clone(),
            role: task.role.clone(),
            inputs: task.inputs.clone(),
            args: task.args.clone(),
            position: None,
        }
    }
}

impl From<&UiTask> for Task {
    fn from(ui_task: &UiTask) -> Self {
        use task_composer_core::types::Status;
        Task {
            task_id: ui_task.task_id.clone(),
            name: ui_task.name.clone(),
            description: ui_task.description.clone(),
            priority: ui_task.priority,
            status: match ui_task.status {
                UiStatus::Pending => Status::Pending,
                UiStatus::InProgress => Status::InProgress,
                UiStatus::Completed | UiStatus::Failed => Status::Completed,
            },
            prompt: ui_task.prompt.clone(),
            executor: ui_task.executor.clone(),
            dependencies: ui_task.dependencies.clone(),
            role: ui_task.role.clone(),
            inputs: ui_task.inputs.clone(),
            args: ui_task.args.clone(),
            if_condition: None,
            else_condition: None,
        }
    }
}

/// DAG全体の状態
#[derive(Debug, Clone, Default)]
pub struct DagState {
    /// タスクマップ
    pub tasks: HashMap<String, UiTask>,
    /// 依存エッジ（from -> [to, ...]）
    pub edges: HashMap<String, Vec<String>>,
    /// 設定
    pub config: Config,
    /// 読み込んだファイルパス
    pub file_path: Option<PathBuf>,
    /// 未保存の変更があるか
    pub is_dirty: bool,
}

/// ノードの寸法（レイアウト計算用）
const NODE_WIDTH: f32 = 160.0;
const NODE_HEIGHT: f32 = 70.0;

impl DagState {
    /// JSONからDAG状態を作成
    pub fn from_json(json: &str) -> Result<Self, String> {
        let dag = task_composer_core::DAG::from_json(json)
            .map_err(|e| e.to_string())?;

        let mut tasks: HashMap<String, UiTask> = dag.nodes
            .iter()
            .map(|(id, task)| (id.clone(), UiTask::from(task)))
            .collect();

        // 初期レイアウトを計算
        let positions = Self::calculate_layout(&tasks, &dag.edges);
        for (task_id, pos) in positions {
            if let Some(task) = tasks.get_mut(&task_id) {
                task.position = Some(pos);
            }
        }

        Ok(DagState {
            tasks,
            edges: dag.edges.clone(),
            config: dag.config.clone(),
            file_path: None,
            is_dirty: false,
        })
    }

    /// トポロジカルソートを使用してレイヤーベースのレイアウトを計算
    fn calculate_layout(
        tasks: &HashMap<String, UiTask>,
        edges: &HashMap<String, Vec<String>>,
    ) -> HashMap<String, (f32, f32)> {
        let mut positions = HashMap::new();

        if tasks.is_empty() {
            return positions;
        }

        let mut layers: HashMap<String, usize> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        for task_id in tasks.keys() {
            in_degree.insert(task_id.clone(), 0);
        }
        for to_ids in edges.values() {
            for to_id in to_ids {
                *in_degree.entry(to_id.clone()).or_insert(0) += 1;
            }
        }

        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        for id in &queue {
            layers.insert(id.clone(), 0);
        }

        let mut i = 0;
        while i < queue.len() {
            let current = queue[i].clone();
            let current_layer = *layers.get(&current).unwrap_or(&0);
            i += 1;

            if let Some(children) = edges.get(&current) {
                for child in children {
                    let child_layer = layers.entry(child.clone()).or_insert(0);
                    *child_layer = (*child_layer).max(current_layer + 1);

                    if let Some(deg) = in_degree.get_mut(child) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 && !queue.contains(child) {
                            queue.push(child.clone());
                        }
                    }
                }
            }
        }

        let mut layer_nodes: HashMap<usize, Vec<String>> = HashMap::new();
        for (task_id, layer) in &layers {
            layer_nodes.entry(*layer).or_default().push(task_id.clone());
        }

        let padding = 100.0;
        let h_spacing = NODE_WIDTH + 60.0;  // 同一レイヤー内の横間隔
        let v_spacing = NODE_HEIGHT + 80.0; // レイヤー間の縦間隔

        // 縦方向のフロー（上から下へ）
        for (layer, nodes) in &layer_nodes {
            let y = padding + (*layer as f32) * v_spacing;
            let total_width = (nodes.len() as f32) * h_spacing - 60.0;
            let start_x = padding + (400.0 - total_width / 2.0).max(0.0); // 中央揃え
            for (idx, task_id) in nodes.iter().enumerate() {
                let x = start_x + (idx as f32) * h_spacing;
                positions.insert(task_id.clone(), (x, y));
            }
        }

        positions
    }

    /// JSONに変換
    pub fn to_json(&self) -> Result<String, String> {
        #[derive(Serialize)]
        struct JsonDag {
            tasks: Vec<Task>,
            config: Config,
        }

        let tasks: Vec<Task> = self.tasks.values().map(Task::from).collect();
        let json_dag = JsonDag {
            tasks,
            config: self.config.clone(),
        };

        serde_json::to_string_pretty(&json_dag)
            .map_err(|e| e.to_string())
    }

    /// タスクを追加
    pub fn add_task(&mut self, task: UiTask) {
        let task_id = task.task_id.clone();
        self.tasks.insert(task_id.clone(), task);
        self.edges.entry(task_id).or_insert_with(Vec::new);
        self.is_dirty = true;
    }

    /// タスクを削除
    pub fn remove_task(&mut self, task_id: &str) {
        self.tasks.remove(task_id);
        self.edges.remove(task_id);
        // 他のタスクからの参照も削除
        for deps in self.edges.values_mut() {
            deps.retain(|id| id != task_id);
        }
        self.is_dirty = true;
    }

    /// 依存関係を追加
    pub fn add_edge(&mut self, from: &str, to: &str) {
        self.edges
            .entry(from.to_string())
            .or_insert_with(Vec::new)
            .push(to.to_string());

        // タスクのdependenciesも更新
        if let Some(task) = self.tasks.get_mut(to) {
            if !task.dependencies.contains(&from.to_string()) {
                task.dependencies.push(from.to_string());
            }
        }
        self.is_dirty = true;
    }

    /// 依存関係を削除
    pub fn remove_edge(&mut self, from: &str, to: &str) {
        if let Some(deps) = self.edges.get_mut(from) {
            deps.retain(|id| id != to);
        }
        if let Some(task) = self.tasks.get_mut(to) {
            task.dependencies.retain(|id| id != from);
        }
        self.is_dirty = true;
    }
}
