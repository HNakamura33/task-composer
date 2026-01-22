use super::*;
use crate::types::{Role, ToolPermission, FilePermission};

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
        name: Some("Sample Task".to_string()),
        description: Some("This is a sample task.".to_string()),
        priority: 1,
        prompt: Some("Execute sample task.".to_string()),
        role: Role {
            role_id: "role_1".to_string(),
            name: "Sample Role".to_string(),
            subagents: vec![],
            skills: vec![],
            description: "Role for sample task.".to_string(),
            tool_permissions: ToolPermission::default(),
            file_permissions: FilePermission::default(),
        },
        dependencies: vec![],
        executor: String::from("log"),
        args: serde_json::Value::Null,
        if_condition: None,
        else_condition: None,
        timeout_secs: None,
    };

    dag.add_task(task);
    assert_eq!(dag.nodes.len(), 1);
}

#[test]
fn test_add_edge() {
    let mut dag = DAG::new();
    let task1 = Task {
        task_id: "1".to_string(),
        name: Some("Task 1".to_string()),
        description: Some("First task.".to_string()),
        priority: 1,
        prompt: Some("Execute task 1.".to_string()),
        role: Role {
            role_id: "role_1".to_string(),
            name: "Role 1".to_string(),
            subagents: vec![],
            skills: vec![],
            description: "Role for task 1.".to_string(),
            tool_permissions: ToolPermission::default(),
            file_permissions: FilePermission::default(),
        },
        dependencies: vec![],
        executor: String::from("log"),
        args: serde_json::Value::Null,
        if_condition: None,
        else_condition: None,
        timeout_secs: None,
    };
    let task2 = Task {
        task_id: "2".to_string(),
        name: Some("Task 2".to_string()),
        description: Some("Second task.".to_string()),
        priority: 2,
        prompt: Some("Execute task 2.".to_string()),
        role: Role {
            role_id: "role_2".to_string(),
            name: "Role 2".to_string(),
            subagents: vec![],
            skills: vec![],
            description: "Role for task 2.".to_string(),
            tool_permissions: ToolPermission::default(),
            file_permissions: FilePermission::default(),
        },
        dependencies: vec![],
        executor: String::from("log"),
        args: serde_json::Value::Null,
        if_condition: None,
        else_condition: None,
        timeout_secs: None,
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
                "prompt": "Do task 1",
                "executor": "log",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "Role 1",
                    "tool_permissions": {
                        "bash": {
                            "allowed_commands": [],
                            "blocked_commands": [],
                            "require_confirmation": []
                        },
                        "write": {
                            "max_file_size_mb": null,
                            "allowed_extensions": []
                        }
                    },
                    "file_permissions": {
                        "allowed_paths": [],
                        "denied_paths": [],
                        "read_only_paths": []
                    }
                }
            },
            {
                "task_id": "2",
                "name": "Task 2",
                "description": "Second task",
                "priority": 2,
                "prompt": "Do task 2",
                "executor": "log",
                "args": {},
                "dependencies": ["1"],
                "role": {
                    "role_id": "r2",
                    "name": "Role 2",
                    "subagents": [],
                    "skills": [],
                    "description": "Role 2",
                    "tool_permissions": {
                        "bash": {
                            "allowed_commands": [],
                            "blocked_commands": [],
                            "require_confirmation": []
                        },
                        "write": {
                            "max_file_size_mb": null,
                            "allowed_extensions": []
                        }
                    },
                    "file_permissions": {
                        "allowed_paths": [],
                        "denied_paths": [],
                        "read_only_paths": []
                    }
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
    let json = std::fs::read_to_string("../samples/basics/simple_dag.json").unwrap();
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
    let json = std::fs::read_to_string("../samples/basics/simple_dag.json").unwrap();
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

#[test]
fn test_topological_sort() {
    let json = std::fs::read_to_string("../samples/basics/simple_dag.json").unwrap();
    let dag = DAG::from_json(&json).unwrap();

    let sorted = dag.topological_sort().unwrap();

    // 4つのノードがソートされている
    assert_eq!(sorted.len(), 4);

    // 順序の検証: 依存関係が正しく反映されているか
    let pos_1 = sorted.iter().position(|x| x == "1").unwrap();
    let pos_2 = sorted.iter().position(|x| x == "2").unwrap();
    let pos_3 = sorted.iter().position(|x| x == "3").unwrap();
    let pos_4 = sorted.iter().position(|x| x == "4").unwrap();

    // Task 1 は Task 2, 3 より前
    assert!(pos_1 < pos_2);
    assert!(pos_1 < pos_3);

    // Task 2, 3 は Task 4 より前
    assert!(pos_2 < pos_4);
    assert!(pos_3 < pos_4);
}

#[test]
fn test_topological_sort_cycle_detection() {
    // 循環のあるDAGを作成
    let mut dag = DAG::new();

    let task1 = Task {
        task_id: "A".to_string(),
        ..Default::default()
    };
    let task2 = Task {
        task_id: "B".to_string(),
        ..Default::default()
    };
    let task3 = Task {
        task_id: "C".to_string(),
        ..Default::default()
    };

    dag.add_task(task1);
    dag.add_task(task2);
    dag.add_task(task3);

    // A → B → C → A の循環を作成
    dag.add_edge("A", "B");
    dag.add_edge("B", "C");
    dag.add_edge("C", "A");

    // 循環があるのでエラーを返すべき
    let result = dag.topological_sort();
    assert!(result.is_err());
}

#[test]
fn test_compute_all_descendants() {
    let json = std::fs::read_to_string("../samples/basics/simple_dag.json").unwrap();
    let dag = DAG::from_json(&json).unwrap();

    let descendants = dag.compute_all_descendants().unwrap();

    // Task 1 の子孫: {2, 3, 4}
    let desc_1 = descendants.get("1").unwrap();
    assert!(desc_1.contains("2"));
    assert!(desc_1.contains("3"));
    assert!(desc_1.contains("4"));
    assert_eq!(desc_1.len(), 3);

    // Task 2 の子孫: {4}
    let desc_2 = descendants.get("2").unwrap();
    assert!(desc_2.contains("4"));
    assert_eq!(desc_2.len(), 1);

    // Task 3 の子孫: {4}
    let desc_3 = descendants.get("3").unwrap();
    assert!(desc_3.contains("4"));
    assert_eq!(desc_3.len(), 1);

    // Task 4 の子孫: {} (終端ノード)
    let desc_4 = descendants.get("4").unwrap();
    assert!(desc_4.is_empty());
}

#[test]
fn test_compute_all_descendants_large_graph() {
    // 大きなグラフを作成
    //
    //       1
    //      /|\
    //     2 3 4
    //     |X| |
    //     5 6 7
    //      \|/
    //       8
    //       |
    //       9
    //       |
    //      10
    //
    let mut dag = DAG::new();

    // 10個のタスクを作成
    for i in 1..=10 {
        let task = Task {
            task_id: i.to_string(),
            ..Default::default()
        };
        dag.add_task(task);
    }

    // エッジを追加
    // レベル1 → レベル2
    dag.add_edge("1", "2");
    dag.add_edge("1", "3");
    dag.add_edge("1", "4");

    // レベル2 → レベル3 (クロス)
    dag.add_edge("2", "5");
    dag.add_edge("2", "6");
    dag.add_edge("3", "5");
    dag.add_edge("3", "6");
    dag.add_edge("4", "7");

    // レベル3 → レベル4
    dag.add_edge("5", "8");
    dag.add_edge("6", "8");
    dag.add_edge("7", "8");

    // レベル4 → レベル5 → レベル6
    dag.add_edge("8", "9");
    dag.add_edge("9", "10");

    let descendants = dag.compute_all_descendants().unwrap();

    // Task 1 の子孫: {2,3,4,5,6,7,8,9,10} (全て)
    let desc_1 = descendants.get("1").unwrap();
    assert_eq!(desc_1.len(), 9);
    for i in 2..=10 {
        assert!(
            desc_1.contains(&i.to_string()),
            "1 should have {} as descendant",
            i
        );
    }

    // Task 2 の子孫: {5,6,8,9,10}
    let desc_2 = descendants.get("2").unwrap();
    assert_eq!(desc_2.len(), 5);
    assert!(desc_2.contains("5"));
    assert!(desc_2.contains("6"));
    assert!(desc_2.contains("8"));
    assert!(desc_2.contains("9"));
    assert!(desc_2.contains("10"));

    // Task 4 の子孫: {7,8,9,10}
    let desc_4 = descendants.get("4").unwrap();
    assert_eq!(desc_4.len(), 4);
    assert!(desc_4.contains("7"));
    assert!(desc_4.contains("8"));
    assert!(desc_4.contains("9"));
    assert!(desc_4.contains("10"));

    // Task 8 の子孫: {9,10}
    let desc_8 = descendants.get("8").unwrap();
    assert_eq!(desc_8.len(), 2);
    assert!(desc_8.contains("9"));
    assert!(desc_8.contains("10"));

    // Task 10 の子孫: {} (終端)
    let desc_10 = descendants.get("10").unwrap();
    assert!(desc_10.is_empty());
}

#[test]
fn test_compute_all_ancestors() {
    let json = std::fs::read_to_string("../samples/basics/simple_dag.json").unwrap();
    let dag = DAG::from_json(&json).unwrap();

    let ancestors = dag.compute_all_ancestors().unwrap();

    // Task 1 の祖先: {} (ルートノード)
    let anc_1 = ancestors.get("1").unwrap();
    assert!(anc_1.is_empty());

    // Task 2 の祖先: {1}
    let anc_2 = ancestors.get("2").unwrap();
    assert!(anc_2.contains("1"));
    assert_eq!(anc_2.len(), 1);

    // Task 3 の祖先: {1}
    let anc_3 = ancestors.get("3").unwrap();
    assert!(anc_3.contains("1"));
    assert_eq!(anc_3.len(), 1);

    // Task 4 の祖先: {1, 2, 3}
    let anc_4 = ancestors.get("4").unwrap();
    assert!(anc_4.contains("1"));
    assert!(anc_4.contains("2"));
    assert!(anc_4.contains("3"));
    assert_eq!(anc_4.len(), 3);
}

#[test]
fn test_compute_all_ancestors_large_graph() {
    // 大きなグラフを作成（test_compute_all_descendants_large_graphと同じ構造）
    let mut dag = DAG::new();

    for i in 1..=10 {
        let task = Task {
            task_id: i.to_string(),
            ..Default::default()
        };
        dag.add_task(task);
    }

    dag.add_edge("1", "2");
    dag.add_edge("1", "3");
    dag.add_edge("1", "4");
    dag.add_edge("2", "5");
    dag.add_edge("2", "6");
    dag.add_edge("3", "5");
    dag.add_edge("3", "6");
    dag.add_edge("4", "7");
    dag.add_edge("5", "8");
    dag.add_edge("6", "8");
    dag.add_edge("7", "8");
    dag.add_edge("8", "9");
    dag.add_edge("9", "10");

    let ancestors = dag.compute_all_ancestors().unwrap();

    // Task 1 の祖先: {} (ルート)
    let anc_1 = ancestors.get("1").unwrap();
    assert!(anc_1.is_empty());

    // Task 5 の祖先: {1, 2, 3}
    let anc_5 = ancestors.get("5").unwrap();
    assert!(anc_5.contains("1"));
    assert!(anc_5.contains("2"));
    assert!(anc_5.contains("3"));
    assert_eq!(anc_5.len(), 3);

    // Task 8 の祖先: {1,2,3,4,5,6,7}
    let anc_8 = ancestors.get("8").unwrap();
    assert_eq!(anc_8.len(), 7);
    for i in 1..=7 {
        assert!(
            anc_8.contains(&i.to_string()),
            "8 should have {} as ancestor",
            i
        );
    }

    // Task 10 の祖先: {1,2,3,4,5,6,7,8,9} (全て)
    let anc_10 = ancestors.get("10").unwrap();
    assert_eq!(anc_10.len(), 9);
    for i in 1..=9 {
        assert!(
            anc_10.contains(&i.to_string()),
            "10 should have {} as ancestor",
            i
        );
    }
}

#[test]
fn test_get_all_parallel_pairs() {
    // sample_dag.json: 1 → 2 → 4, 1 → 3 → 4
    // 並行ペア: (2, 3) のみ
    let json = std::fs::read_to_string("../samples/basics/simple_dag.json").unwrap();
    let dag = DAG::from_json(&json).unwrap();

    let pairs = dag.get_all_parallel_pairs().unwrap();

    // 重複なしで (2,3) または (3,2) が1つだけあるべき
    // ペアの数を確認
    println!("Parallel pairs: {:?}", pairs);

    // Task 2 と Task 3 のペアが存在することを確認
    let has_2_3 = pairs
        .iter()
        .any(|(a, b)| (a == "2" && b == "3") || (a == "3" && b == "2"));
    assert!(has_2_3, "Should have (2,3) as parallel pair");

    // 自己ペアがないことを確認
    let has_self_pair = pairs.iter().any(|(a, b)| a == b);
    assert!(!has_self_pair, "Should not have self pairs");
}

#[test]
fn test_get_all_parallel_pairs_large_graph() {
    // 大きなグラフ:
    //       1
    //      /|\
    //     2 3 4
    //     |X| |
    //     5 6 7
    //      \|/
    //       8
    //       |
    //       9
    //       |
    //      10
    let mut dag = DAG::new();

    for i in 1..=10 {
        let task = Task {
            task_id: i.to_string(),
            ..Default::default()
        };
        dag.add_task(task);
    }

    dag.add_edge("1", "2");
    dag.add_edge("1", "3");
    dag.add_edge("1", "4");
    dag.add_edge("2", "5");
    dag.add_edge("2", "6");
    dag.add_edge("3", "5");
    dag.add_edge("3", "6");
    dag.add_edge("4", "7");
    dag.add_edge("5", "8");
    dag.add_edge("6", "8");
    dag.add_edge("7", "8");
    dag.add_edge("8", "9");
    dag.add_edge("9", "10");

    let pairs = dag.get_all_parallel_pairs().unwrap();
    println!("Large graph parallel pairs: {:?}", pairs);

    // 並行ペアのヘルパー関数
    let has_pair = |a: &str, b: &str| {
        pairs
            .iter()
            .any(|(x, y)| (x == a && y == b) || (x == b && y == a))
    };

    // レベル2の並行ペア: (2,3), (2,4), (3,4)
    assert!(has_pair("2", "3"), "2 and 3 should be parallel");
    assert!(has_pair("2", "4"), "2 and 4 should be parallel");
    assert!(has_pair("3", "4"), "3 and 4 should be parallel");

    // 異なるブランチの並行ペア
    assert!(has_pair("5", "7"), "5 and 7 should be parallel");
    assert!(has_pair("6", "7"), "6 and 7 should be parallel");
    assert!(has_pair("4", "5"), "4 and 5 should be parallel");
    assert!(has_pair("4", "6"), "4 and 6 should be parallel");
    assert!(has_pair("2", "7"), "2 and 7 should be parallel");
    assert!(has_pair("3", "7"), "3 and 7 should be parallel");

    // 依存関係があるペアは含まれないことを確認
    assert!(!has_pair("1", "2"), "1 and 2 should NOT be parallel (1→2)");
    assert!(!has_pair("2", "5"), "2 and 5 should NOT be parallel (2→5)");
    assert!(!has_pair("8", "9"), "8 and 9 should NOT be parallel (8→9)");
    assert!(
        !has_pair("1", "10"),
        "1 and 10 should NOT be parallel (1→...→10)"
    );
    assert!(
        !has_pair("2", "10"),
        "2 and 10 should NOT be parallel (2→...→10)"
    );
    assert!(
        !has_pair("8", "10"),
        "8 and 10 should NOT be parallel (8→9→10)"
    );

    // 自己ペアがないことを確認
    let has_self_pair = pairs.iter().any(|(a, b)| a == b);
    assert!(!has_self_pair, "Should not have self pairs");

    // 重複ペアがないことを確認
    let mut sorted_pairs: Vec<(String, String)> = pairs
        .iter()
        .map(|(a, b)| {
            if a < b {
                (a.clone(), b.clone())
            } else {
                (b.clone(), a.clone())
            }
        })
        .collect();
    sorted_pairs.sort();
    let original_len = sorted_pairs.len();
    sorted_pairs.dedup();
    assert_eq!(
        sorted_pairs.len(),
        original_len,
        "Should not have duplicate pairs"
    );
}

// ============================================
// execute_async のテスト
// ============================================

use crate::task_executor::{TaskExecutor, ExecutionContext, ExecutionResult, LogExecutor};
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc as StdArc;

/// テスト用のシンプルなExecutor
struct TestExecutor {
    execution_count: StdArc<AtomicUsize>,
}

impl TestExecutor {
    fn new(counter: StdArc<AtomicUsize>) -> Self {
        TestExecutor {
            execution_count: counter,
        }
    }
}

#[async_trait]
impl TaskExecutor for TestExecutor {
    fn name(&self) -> &str {
        "test"
    }

    async fn execute_task(
        &self,
        task: &Task,
        _ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        self.execution_count.fetch_add(1, Ordering::SeqCst);
        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            status: ExecutionStatus::Success,
            output: serde_json::json!({
                "task_id": task.task_id,
                "message": format!("Task {} executed", task.task_id)
            }),
        })
    }
}

#[tokio::test]
async fn test_execute_async_simple() {
    let mut dag = DAG::new();

    let task = Task {
        task_id: "1".to_string(),
        executor: "test".to_string(),
        ..Default::default()
    };
    dag.add_task(task);

    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results.get("1").unwrap().status, ExecutionStatus::Success);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_execute_async_with_dependencies() {
    let mut dag = DAG::new();

    // 1 → 2 → 3
    for i in 1..=3 {
        let task = Task {
            task_id: i.to_string(),
            executor: "test".to_string(),
            dependencies: if i > 1 {
                vec![(i - 1).to_string()]
            } else {
                vec![]
            },
            ..Default::default()
        };
        dag.add_task(task);
    }
    dag.add_edge("1", "2");
    dag.add_edge("2", "3");

    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    assert_eq!(results.len(), 3);
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_execute_async_parallel_execution() {
    // 並列実行のテスト: root → (a, b, c) → merge
    let mut dag = DAG::new();

    dag.add_task(Task {
        task_id: "root".to_string(),
        executor: "test".to_string(),
        ..Default::default()
    });

    for name in ["a", "b", "c"] {
        dag.add_task(Task {
            task_id: name.to_string(),
            executor: "test".to_string(),
            dependencies: vec!["root".to_string()],
            ..Default::default()
        });
        dag.add_edge("root", name);
    }

    dag.add_task(Task {
        task_id: "merge".to_string(),
        executor: "test".to_string(),
        dependencies: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        ..Default::default()
    });
    dag.add_edge("a", "merge");
    dag.add_edge("b", "merge");
    dag.add_edge("c", "merge");

    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    assert_eq!(results.len(), 5);
    assert_eq!(counter.load(Ordering::SeqCst), 5);

    // 全てのタスクが成功していることを確認
    for (_, result) in &results {
        assert_eq!(result.status, ExecutionStatus::Success);
    }
}

#[tokio::test]
async fn test_execute_async_with_log_executor() {
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "1",
                "name": "Task 1",
                "description": "First task",
                "priority": 1,
                "prompt": "Execute task 1",
                "executor": "log",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            },
            {
                "task_id": "2",
                "name": "Task 2",
                "description": "Second task",
                "priority": 2,
                "prompt": "Execute task 2",
                "executor": "log",
                "args": {},
                "dependencies": ["1"],
                "role": {
                    "role_id": "r2",
                    "name": "Role 2",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();
    dag.register_executor(Box::new(LogExecutor::new()));

    let results = dag.execute_async().await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results.get("1").unwrap().status, ExecutionStatus::Success);
    assert_eq!(results.get("2").unwrap().status, ExecutionStatus::Success);
}

#[tokio::test]
async fn test_execute_async_inputs_resolution() {
    // argsのパス参照解決をテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "producer",
                "name": "Producer",
                "description": "",
                "priority": 1,
                "prompt": "",
                "executor": "log",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r", "name": "R", "subagents": [], "skills": [], "description": "",
                    "tool_permissions": { "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] }, "write": { "max_file_size_mb": 10, "allowed_extensions": [] } },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            },
            {
                "task_id": "consumer",
                "name": "Consumer",
                "description": "",
                "priority": 2,
                "prompt": "",
                "executor": "log",
                "args": {
                    "producer_id": "$.producer.output.task_id"
                },
                "dependencies": ["producer"],
                "role": {
                    "role_id": "r", "name": "R", "subagents": [], "skills": [], "description": "",
                    "tool_permissions": { "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] }, "write": { "max_file_size_mb": 10, "allowed_extensions": [] } },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();
    dag.register_executor(Box::new(LogExecutor::new()));

    let results = dag.execute_async().await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results.get("producer").unwrap().status, ExecutionStatus::Success);
    assert_eq!(results.get("consumer").unwrap().status, ExecutionStatus::Success);
}

// ============================================
// if/else条件付き実行のテスト
// ============================================

#[tokio::test]
async fn test_execute_async_if_condition_true() {
    // if条件がtrueの場合、タスクは実行される
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "validate",
                "name": "Validate",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            },
            {
                "task_id": "on_success",
                "name": "On Success",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": ["validate"],
                "if": "$.validate.output.task_id == \"validate\"",
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();
    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results.get("validate").unwrap().status, ExecutionStatus::Success);
    assert_eq!(results.get("on_success").unwrap().status, ExecutionStatus::Success);
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_execute_async_if_condition_false() {
    // if条件がfalseの場合、タスクはスキップされる
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "validate",
                "name": "Validate",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            },
            {
                "task_id": "on_success",
                "name": "On Success",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": ["validate"],
                "if": "$.validate.output.task_id == \"nonexistent\"",
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();
    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results.get("validate").unwrap().status, ExecutionStatus::Success);
    assert_eq!(results.get("on_success").unwrap().status, ExecutionStatus::Skipped);
    assert_eq!(counter.load(Ordering::SeqCst), 1); // validateのみ実行
}

#[tokio::test]
async fn test_execute_async_else_condition() {
    // else条件のテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "validate",
                "name": "Validate",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            },
            {
                "task_id": "on_failure",
                "name": "On Failure",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": ["validate"],
                "else": "$.validate.output.task_id == \"validate\"",
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();
    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results.get("validate").unwrap().status, ExecutionStatus::Success);
    // else条件がtrueなのでスキップされる
    assert_eq!(results.get("on_failure").unwrap().status, ExecutionStatus::Skipped);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_execute_async_skip_propagation() {
    // スキップ伝播のテスト: 依存先がスキップされたら依存元もスキップ
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "first",
                "name": "First",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            },
            {
                "task_id": "second",
                "name": "Second",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": ["first"],
                "if": "false",
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            },
            {
                "task_id": "third",
                "name": "Third",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": ["second"],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();
    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    assert_eq!(results.len(), 3);
    assert_eq!(results.get("first").unwrap().status, ExecutionStatus::Success);
    assert_eq!(results.get("second").unwrap().status, ExecutionStatus::Skipped);
    // thirdはsecondがスキップされたので伝播でスキップ
    assert_eq!(results.get("third").unwrap().status, ExecutionStatus::Skipped);
    assert_eq!(counter.load(Ordering::SeqCst), 1); // firstのみ実行
}

// ============================================
// ループ実行のテスト
// ============================================

#[tokio::test]
async fn test_execute_async_loop_max_iterations() {
    // max_iterationsでループが終了することをテスト
    let json = r#"
    {
        "loop_config": {
            "max_iterations": 3
        },
        "tasks": [
            {
                "task_id": "counter",
                "name": "Counter",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();
    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    // 1タスク × 3イテレーション = 3回実行
    assert_eq!(counter.load(Ordering::SeqCst), 3);
    // 最終イテレーションの結果が返される
    assert_eq!(results.len(), 1);
    assert_eq!(results.get("counter").unwrap().status, ExecutionStatus::Success);
}

#[tokio::test]
async fn test_execute_async_loop_until_condition() {
    // until_conditionでループが終了することをテスト
    // カウンターが2になったら終了
    let json = r#"
    {
        "loop_config": {
            "max_iterations": 10,
            "until_condition": "$.loop.iteration >= 2"
        },
        "tasks": [
            {
                "task_id": "task1",
                "name": "Task 1",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();
    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    // iteration 0, 1, 2 で実行され、iteration 2 >= 2 でuntilが発動して終了
    // つまり 3回実行される
    assert_eq!(counter.load(Ordering::SeqCst), 3);
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_execute_async_loop_while_condition() {
    // while_conditionでループが継続することをテスト
    let json = r#"
    {
        "loop_config": {
            "max_iterations": 10,
            "while_condition": "$.loop.iteration < 2"
        },
        "tasks": [
            {
                "task_id": "task1",
                "name": "Task 1",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();
    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    // iteration 0, 1, 2 で実行、実行後に iteration 2 < 2 が false なので終了
    // つまり3回実行される（条件は実行後に評価される）
    assert_eq!(counter.load(Ordering::SeqCst), 3);
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_execute_async_loop_with_multiple_tasks() {
    // 複数タスクを持つDAGのループテスト
    let json = r#"
    {
        "loop_config": {
            "max_iterations": 2
        },
        "tasks": [
            {
                "task_id": "task1",
                "name": "Task 1",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            },
            {
                "task_id": "task2",
                "name": "Task 2",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": ["task1"],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();
    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    // 2タスク × 2イテレーション = 4回実行
    assert_eq!(counter.load(Ordering::SeqCst), 4);
    // 最終イテレーションの両タスクの結果が返される
    assert_eq!(results.len(), 2);
    assert_eq!(results.get("task1").unwrap().status, ExecutionStatus::Success);
    assert_eq!(results.get("task2").unwrap().status, ExecutionStatus::Success);
}

#[tokio::test]
async fn test_execute_async_no_loop_config() {
    // loop_configがない場合は通常実行
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "task1",
                "name": "Task 1",
                "description": "",
                "priority": 1,
                                "prompt": "",
                "executor": "test",
                "args": {},
                "dependencies": [],
                "role": {
                    "role_id": "r1",
                    "name": "Role 1",
                    "subagents": [],
                    "skills": [],
                    "description": "",
                    "tool_permissions": {
                        "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] },
                        "write": { "max_file_size_mb": 10, "allowed_extensions": [] }
                    },
                    "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] }
                }
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();
    assert!(dag.loop_config.is_none());

    let counter = StdArc::new(AtomicUsize::new(0));
    dag.register_executor(Box::new(TestExecutor::new(counter.clone())));

    let results = dag.execute_async().await.unwrap();

    // 1回だけ実行される
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert_eq!(results.len(), 1);
}

// ============================================
// 依存関係自動解決のテスト
// ============================================

#[test]
fn test_from_json_auto_dependency_from_args() {
    // argsのパス参照から依存関係が自動的に解決されることをテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "task_a",
                "executor": "log",
                "args": {}
            },
            {
                "task_id": "task_b",
                "executor": "log",
                "args": {
                    "value": "$.task_a.output.result"
                }
            }
        ]
    }
    "#;

    let dag = DAG::from_json(json).unwrap();

    // task_b は task_a を参照しているので、自動的に依存関係が追加される
    let task_b = dag.nodes.get("task_b").unwrap();
    assert!(task_b.dependencies.contains(&"task_a".to_string()));

    // エッジも追加されているはず
    let edges_from_a = dag.edges.get("task_a").unwrap();
    assert!(edges_from_a.contains(&"task_b".to_string()));
}

#[test]
fn test_from_json_auto_dependency_from_prompt() {
    // promptのパス参照から依存関係が自動的に解決されることをテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "prepare",
                "executor": "log",
                "args": {}
            },
            {
                "task_id": "process",
                "executor": "log",
                "prompt": "Process data: ${$.prepare.output.data}",
                "args": {}
            }
        ]
    }
    "#;

    let dag = DAG::from_json(json).unwrap();

    let task_process = dag.nodes.get("process").unwrap();
    assert!(task_process.dependencies.contains(&"prepare".to_string()));
}

#[test]
fn test_from_json_auto_dependency_from_if_condition() {
    // if条件のパス参照から依存関係が自動的に解決されることをテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "validate",
                "executor": "log",
                "args": {}
            },
            {
                "task_id": "on_success",
                "executor": "log",
                "args": {},
                "if": "$.validate.output.ok == true"
            }
        ]
    }
    "#;

    let dag = DAG::from_json(json).unwrap();

    let task_on_success = dag.nodes.get("on_success").unwrap();
    assert!(task_on_success.dependencies.contains(&"validate".to_string()));
}

#[test]
fn test_from_json_auto_dependency_from_else_condition() {
    // else条件のパス参照から依存関係が自動的に解決されることをテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "validate",
                "executor": "log",
                "args": {}
            },
            {
                "task_id": "on_failure",
                "executor": "log",
                "args": {},
                "else": "$.validate.output.ok == true"
            }
        ]
    }
    "#;

    let dag = DAG::from_json(json).unwrap();

    let task_on_failure = dag.nodes.get("on_failure").unwrap();
    assert!(task_on_failure.dependencies.contains(&"validate".to_string()));
}

#[test]
fn test_from_json_auto_dependency_multiple_references() {
    // 複数のタスクを参照している場合、全ての依存関係が追加されることをテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "task_a",
                "executor": "log",
                "args": {}
            },
            {
                "task_id": "task_b",
                "executor": "log",
                "args": {}
            },
            {
                "task_id": "task_c",
                "executor": "log",
                "args": {
                    "a_value": "$.task_a.output.x",
                    "b_value": "$.task_b.output.y"
                }
            }
        ]
    }
    "#;

    let dag = DAG::from_json(json).unwrap();

    let task_c = dag.nodes.get("task_c").unwrap();
    assert!(task_c.dependencies.contains(&"task_a".to_string()));
    assert!(task_c.dependencies.contains(&"task_b".to_string()));
}

#[test]
fn test_from_json_auto_dependency_merge_with_explicit() {
    // 明示的な依存関係と自動解決された依存関係がマージされることをテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "explicit_dep",
                "executor": "log",
                "args": {}
            },
            {
                "task_id": "implicit_dep",
                "executor": "log",
                "args": {}
            },
            {
                "task_id": "consumer",
                "executor": "log",
                "dependencies": ["explicit_dep"],
                "args": {
                    "value": "$.implicit_dep.output.x"
                }
            }
        ]
    }
    "#;

    let dag = DAG::from_json(json).unwrap();

    let task_consumer = dag.nodes.get("consumer").unwrap();
    // 明示的な依存関係
    assert!(task_consumer.dependencies.contains(&"explicit_dep".to_string()));
    // 自動解決された依存関係
    assert!(task_consumer.dependencies.contains(&"implicit_dep".to_string()));
}

#[test]
fn test_from_json_auto_dependency_no_duplicate() {
    // 明示的に指定された依存関係と同じタスクを参照している場合、重複しないことをテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "task_a",
                "executor": "log",
                "args": {}
            },
            {
                "task_id": "task_b",
                "executor": "log",
                "dependencies": ["task_a"],
                "args": {
                    "value": "$.task_a.output.x"
                }
            }
        ]
    }
    "#;

    let dag = DAG::from_json(json).unwrap();

    let task_b = dag.nodes.get("task_b").unwrap();
    // task_a は1回だけ含まれる（重複なし）
    let count = task_b.dependencies.iter().filter(|d| *d == "task_a").count();
    assert_eq!(count, 1);
}

#[test]
fn test_from_json_auto_dependency_excludes_self() {
    // $.self参照は依存関係に追加されないことをテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "task_a",
                "executor": "log",
                "args": {
                    "my_id": "$.self.task_id"
                }
            }
        ]
    }
    "#;

    let dag = DAG::from_json(json).unwrap();

    let task_a = dag.nodes.get("task_a").unwrap();
    // selfは依存関係に含まれない
    assert!(!task_a.dependencies.contains(&"self".to_string()));
    assert!(task_a.dependencies.is_empty());
}

#[test]
fn test_from_json_auto_dependency_excludes_loop() {
    // $.loop参照は依存関係に追加されないことをテスト
    let json = r#"
    {
        "loop_config": {
            "max_iterations": 3
        },
        "tasks": [
            {
                "task_id": "counter",
                "executor": "log",
                "args": {
                    "iteration": "$.loop.iteration"
                }
            }
        ]
    }
    "#;

    let dag = DAG::from_json(json).unwrap();

    let task_counter = dag.nodes.get("counter").unwrap();
    // loopは依存関係に含まれない
    assert!(!task_counter.dependencies.contains(&"loop".to_string()));
    assert!(task_counter.dependencies.is_empty());
}

#[test]
fn test_from_json_auto_dependency_excludes_nonexistent_task() {
    // 存在しないタスクへの参照は依存関係に追加されないことをテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "task_a",
                "executor": "log",
                "args": {
                    "value": "$.nonexistent_task.output.x"
                }
            }
        ]
    }
    "#;

    let dag = DAG::from_json(json).unwrap();

    let task_a = dag.nodes.get("task_a").unwrap();
    // 存在しないタスクは依存関係に含まれない
    assert!(!task_a.dependencies.contains(&"nonexistent_task".to_string()));
    assert!(task_a.dependencies.is_empty());
}

#[tokio::test]
async fn test_execute_async_with_auto_dependency() {
    // 自動解決された依存関係でも正しく実行されることをテスト
    let json = r#"
    {
        "tasks": [
            {
                "task_id": "producer",
                "name": "Producer",
                "executor": "log",
                "args": {}
            },
            {
                "task_id": "consumer",
                "name": "Consumer",
                "executor": "log",
                "prompt": "Consume ${$.producer.output.task_id}",
                "args": {}
            }
        ]
    }
    "#;

    let mut dag = DAG::from_json(json).unwrap();

    // consumerがproducerに依存していることを確認
    let consumer = dag.nodes.get("consumer").unwrap();
    assert!(consumer.dependencies.contains(&"producer".to_string()));

    dag.register_executor(Box::new(LogExecutor::new()));

    let results = dag.execute_async().await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results.get("producer").unwrap().status, ExecutionStatus::Success);
    assert_eq!(results.get("consumer").unwrap().status, ExecutionStatus::Success);
}
