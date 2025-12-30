//! Task Composer - DAGベースのタスク管理ライブラリ
//!
//! 有向非巡回グラフ(DAG)を使ってタスクの依存関係を管理します。

use std::collections::HashMap;
use serde::Deserialize;

/// タスクを表す構造体
///
/// DAG内の各ノードに対応し、タスクの詳細情報を保持します。
#[derive(Deserialize)]
struct Task {
    /// タスクの一意な識別子
    task_id: String,
    /// タスクの名前
    name: String,
    /// タスクの詳細説明
    description: String,
    /// タスクの優先度（0-255、数値が大きいほど高優先）
    priority: u8,
    /// タスクの現在の状態
    status: Status,
    /// タスク実行時のプロンプト
    prompt: String,
    /// タスクを実行するロール
    role: Role,
    /// このタスクが依存するタスクIDのリスト
    dependencies: Vec<String>,
}

/// Task のデフォルト値
impl Default for Task {
    fn default() -> Self {
        Task {
            task_id: String::new(),
            name: String::from("Untitled Task"),
            description: String::new(),
            priority: 0,
            status: Status::default(),
            prompt: String::new(),
            role: Role::default(),
            dependencies: vec![],
        }
    }
}

/// ロール（役割）を表す構造体
///
/// タスクを実行するエージェントの役割と権限を定義します。
#[derive(Deserialize)]
struct Role {
    /// ロールの一意な識別子
    role_id: String,
    /// ロールの名前
    name: String,
    /// 利用可能なサブエージェントのリスト
    subagents: Vec<String>,
    /// このロールが持つスキルのリスト
    skills: Vec<String>,
    /// ロールの詳細説明
    description: String,
    /// 許可されたツールのリスト
    tool_permissions: Vec<String>,
    /// 許可されたファイル操作のリスト
    file_permissions: Vec<String>,
}

/// Role のデフォルト値
impl Default for Role {
    fn default() -> Self {
        Role {
            role_id: String::new(),
            name: String::from("Default Role"),
            subagents: vec![],
            skills: vec![],
            description: String::new(),
            tool_permissions: vec![],
            file_permissions: vec![],
        }
    }
}
/// タスクの状態を表すenum
///
/// タスクのライフサイクルにおける現在の状態を示します。
#[derive(Deserialize)]
enum Status {
    /// 未着手: タスクがまだ開始されていない
    Pending,
    /// 進行中: タスクが現在実行中
    InProgress,
    /// 完了: タスクが正常に完了した
    Completed,
}

/// Status のデフォルト値
impl Default for Status {
    fn default() -> Self {
        Status::Pending
    }
}


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
struct DAG {
    /// ノード間のエッジを保持するHashMap
    /// - キー: 始点ノードID
    /// - 値: 終点ノードIDのリスト
    edges: HashMap<String, Vec<String>>,

    /// ノードを保持するHashMap
    /// - キー: ノードID
    /// - 値: タスク情報
    nodes: HashMap<String, Task>,
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
    fn new() -> Self {
        DAG {
            edges: HashMap::new(),
            nodes: HashMap::new(),  
        }
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
    fn add_task(&mut self, task: Task) {
        let task_id = task.task_id.clone();
        self.nodes.insert(task_id.clone(), task);
        self.edges.entry(task_id).or_insert(vec![]);
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
    fn add_edge(&mut self, from: &str, to: &str) {
        self.edges
            .entry(from.to_string())
            .or_insert(vec![])
            .push(to.to_string());
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
    fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        let dag_json: DAGJson = serde_json::from_str(json_str)?;
        let mut dag = DAG::new();

        for task in dag_json.tasks {
            let dependencies = task.dependencies.clone();
            let task_id = task.task_id.clone();

            dag.add_task(task);

            // 依存関係をエッジとして追加
            for dep in dependencies {
                dag.add_edge(&dep, &task_id);
            }
        }

        Ok(dag)
    }

    fn get_dependencies(&self, task_id: &str) -> Option<&Vec<String>> {
        self.edges.get(task_id)
    }
}

/// DAG のデフォルト値S
impl Default for DAG {
    fn default() -> Self {
        DAG::new()
    }
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn test_new_dag_is_empty() {
        let dag = DAG::new();
        assert!(dag.edges.is_empty());
        assert!(dag.nodes.is_empty());
    }

    #[test]
    fn test_add_task() {
        let mut dag = DAG::new();
        let task = Task {
            task_id: "1".to_string(),
            name: "Sample Task".to_string(),
            description: "This is a sample task.".to_string(),
            priority: 1,
            status: Status::Pending,
            prompt: "Execute sample task.".to_string(),
            role: Role {
                role_id: "role_1".to_string(),
                name: "Sample Role".to_string(),
                subagents: vec![],
                skills: vec![],
                description: "Role for sample task.".to_string(),
                tool_permissions: vec![],
                file_permissions: vec![],
            },
            dependencies: vec![],
        };

        dag.add_task(task);
        assert_eq!(dag.nodes.len(), 1);
    }

    #[test]
    fn test_add_edge() {
        let mut dag = DAG::new();
        let task1 = Task {
            task_id: "1".to_string(),
            name: "Task 1".to_string(),
            description: "First task.".to_string(),
            priority: 1,
            status: Status::Pending,
            prompt: "Execute task 1.".to_string(),
            role: Role {
                role_id: "role_1".to_string(),
                name: "Role 1".to_string(),
                subagents: vec![],
                skills: vec![],
                description: "Role for task 1.".to_string(),
                tool_permissions: vec![],
                file_permissions: vec![],
            },
            dependencies: vec![],
        };
        let task2 = Task {
            task_id: "2".to_string(),
            name: "Task 2".to_string(),
            description: "Second task.".to_string(),
            priority: 2,
            status: Status::Pending,
            prompt: "Execute task 2.".to_string(),
            role: Role {
                role_id: "role_2".to_string(),
                name: "Role 2".to_string(),
                subagents: vec![],
                skills: vec![],
                description: "Role for task 2.".to_string(),
                tool_permissions: vec![],
                file_permissions: vec![],
            },
            dependencies: vec![],
        };

        let id1 = task1.task_id.clone();
        let id2 = task2.task_id.clone();

        dag.add_task(task1);
        dag.add_task(task2);

        dag.add_edge(&id1, &id2);
        assert_eq!(dag.edges.get("1").unwrap().len(), 1);
    }

    #[test]
    fn test_from_json() {
        let json = r#"
        {
            "tasks": [
                {
                    "task_id": "1",
                    "name": "Task 1",
                    "description": "First task",
                    "priority": 1,
                    "status": "Pending",
                    "prompt": "Do task 1",
                    "dependencies": [],
                    "role": {
                        "role_id": "r1",
                        "name": "Role 1",
                        "subagents": [],
                        "skills": [],
                        "description": "Role 1",
                        "tool_permissions": [],
                        "file_permissions": []
                    }
                },
                {
                    "task_id": "2",
                    "name": "Task 2",
                    "description": "Second task",
                    "priority": 2,
                    "status": "Pending",
                    "prompt": "Do task 2",
                    "dependencies": ["1"],
                    "role": {
                        "role_id": "r2",
                        "name": "Role 2",
                        "subagents": [],
                        "skills": [],
                        "description": "Role 2",
                        "tool_permissions": [],
                        "file_permissions": []
                    }
                }
            ]
        }
        "#;

        let dag = DAG::from_json(json).unwrap();

        // 2つのタスクが読み込まれたことを確認
        assert_eq!(dag.nodes.len(), 2);

        // Task 1 → Task 2 のエッジが作成されたことを確認
        let edges_from_1 = dag.edges.get("1").unwrap();
        assert!(edges_from_1.contains(&"2".to_string()));
    }

    #[test]
    fn test_from_json_file() {
        // sample_dag.json を読み込むテスト
        let json = std::fs::read_to_string("sample_dag.json").unwrap();
        let dag = DAG::from_json(&json).unwrap();

        // 4つのタスクが読み込まれたことを確認
        assert_eq!(dag.nodes.len(), 4);

        // 依存関係の確認: Task 1 → Task 2, Task 3
        let edges_from_1 = dag.edges.get("1").unwrap();
        assert!(edges_from_1.contains(&"2".to_string()));
        assert!(edges_from_1.contains(&"3".to_string()));

        // 依存関係の確認: Task 2, Task 3 → Task 4
        let edges_from_2 = dag.edges.get("2").unwrap();
        let edges_from_3 = dag.edges.get("3").unwrap();
        assert!(edges_from_2.contains(&"4".to_string()));
        assert!(edges_from_3.contains(&"4".to_string()));
    }

    #[test]
    fn test_get_dependencies() {
        let json = std::fs::read_to_string("sample_dag.json").unwrap();
        let dag = DAG::from_json(&json).unwrap();

        // Task 1 は Task 2, 3 への依存を持つ
        let deps = dag.get_dependencies("1").unwrap();
        assert!(deps.contains(&"2".to_string()));
        assert!(deps.contains(&"3".to_string()));

        // Task 4 は依存先がない（終端ノード）
        let deps_4 = dag.get_dependencies("4").unwrap();
        assert!(deps_4.is_empty());

        // 存在しないタスクは None
        assert!(dag.get_dependencies("999").is_none());
    }
}

fn main() {
    // sample_dag.json を読み込む
    let json = std::fs::read_to_string("sample_dag.json")
        .expect("Failed to read sample_dag.json");

    // JSONからDAGを作成
    let dag = DAG::from_json(&json)
        .expect("Failed to parse JSON");

    // 読み込んだDAGの情報を表示
    println!("Loaded {} tasks", dag.nodes.len());

    for (task_id, task) in &dag.nodes {
        println!("  Task {}: {}", task_id, task.name);
    }

    println!("\nEdges:");
    for (from, to_list) in &dag.edges {
        for to in to_list {
            println!("  {} -> {}", from, to);
        }
    }
}
