//! DAG構造解析

use std::collections::HashMap;
use crate::dag::DAG;
use super::DagStructureAnalysis;

/// DAG構造を解析
pub fn analyze_dag_structure(dag: &DAG) -> DagStructureAnalysis {
    let mut result = DagStructureAnalysis::default();

    // トポロジカルソート（循環検出）
    match dag.topological_sort() {
        Ok(order) => {
            result.topological_order = Some(order);
            result.has_cycle = false;
        }
        Err(_) => {
            result.has_cycle = true;
        }
    }

    // 並列実行可能ペア
    if let Ok(pairs) = dag.get_all_parallel_pairs() {
        result.parallel_pairs = pairs;
    }

    // ルートノード（依存がない = edges_revに存在しないか空）
    for task_id in dag.nodes.keys() {
        let has_dependencies = dag.edges_rev
            .get(task_id)
            .map(|deps| !deps.is_empty())
            .unwrap_or(false);

        if !has_dependencies {
            result.root_nodes.push(task_id.clone());
        }
    }

    // リーフノード（誰からも依存されていない = edgesに存在しないか空）
    for task_id in dag.nodes.keys() {
        let has_dependents = dag.edges
            .get(task_id)
            .map(|deps| !deps.is_empty())
            .unwrap_or(false);

        if !has_dependents {
            result.leaf_nodes.push(task_id.clone());
        }
    }

    // 孤立ノード（ルートかつリーフ、ただしタスクが1つの場合は除外）
    if dag.nodes.len() > 1 {
        for task_id in &result.root_nodes {
            if result.leaf_nodes.contains(task_id) {
                result.orphan_nodes.push(task_id.clone());
            }
        }
    }

    // ノード深度計算
    if let Some(ref order) = result.topological_order {
        result.node_depths = compute_node_depths(dag, order);
    }

    // クリティカルパス計算
    if !result.node_depths.is_empty() {
        result.critical_path = compute_critical_path(dag, &result.node_depths, &result.leaf_nodes);
    }

    result
}

/// 各ノードの深度を計算（ルートからの最大距離）
fn compute_node_depths(dag: &DAG, order: &[String]) -> HashMap<String, usize> {
    let mut depths: HashMap<String, usize> = HashMap::new();

    for task_id in order {
        // 依存先の最大深度 + 1
        let depth = dag.edges_rev
            .get(task_id)
            .map(|deps| {
                deps.iter()
                    .filter_map(|d| depths.get(d))
                    .max()
                    .map(|m| m + 1)
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        depths.insert(task_id.clone(), depth);
    }

    depths
}

/// クリティカルパスを計算（最長パスをバックトラック）
fn compute_critical_path(
    dag: &DAG,
    depths: &HashMap<String, usize>,
    leaf_nodes: &[String],
) -> Vec<String> {
    // 最大深度のリーフノードを見つける
    let max_depth_leaf = leaf_nodes
        .iter()
        .filter_map(|id| depths.get(id).map(|d| (id, d)))
        .max_by_key(|(_, d)| *d)
        .map(|(id, _)| id.clone());

    let Some(start) = max_depth_leaf else {
        return Vec::new();
    };

    // バックトラックしてパスを構築
    let mut path = vec![start.clone()];
    let mut current = start;

    loop {
        let deps = dag.edges_rev.get(&current);
        if deps.is_none() || deps.unwrap().is_empty() {
            break;
        }

        // 最大深度の依存先を選択
        let next = deps
            .unwrap()
            .iter()
            .filter_map(|d| depths.get(d).map(|depth| (d, depth)))
            .max_by_key(|(_, d)| *d)
            .map(|(id, _)| id.clone());

        if let Some(next_id) = next {
            path.push(next_id.clone());
            current = next_id;
        } else {
            break;
        }
    }

    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Task, Status};

    fn create_test_dag() -> DAG {
        let mut dag = DAG::new();

        // Task 1 (root)
        dag.add_task(Task {
            task_id: "1".to_string(),
            name: "Task 1".to_string(),
            description: "".to_string(),
            priority: 1,
            status: Status::Pending,
            prompt: "".to_string(),
            executor: "log".to_string(),
            dependencies: vec![],
            role: Default::default(),
            args: serde_json::Value::Null,
            inputs: serde_json::Value::Null,
        });

        // Task 2 depends on 1
        dag.add_task(Task {
            task_id: "2".to_string(),
            name: "Task 2".to_string(),
            description: "".to_string(),
            priority: 1,
            status: Status::Pending,
            prompt: "".to_string(),
            executor: "log".to_string(),
            dependencies: vec!["1".to_string()],
            role: Default::default(),
            args: serde_json::Value::Null,
            inputs: serde_json::Value::Null,
        });
        dag.add_edge("1", "2");

        // Task 3 depends on 1
        dag.add_task(Task {
            task_id: "3".to_string(),
            name: "Task 3".to_string(),
            description: "".to_string(),
            priority: 1,
            status: Status::Pending,
            prompt: "".to_string(),
            executor: "log".to_string(),
            dependencies: vec!["1".to_string()],
            role: Default::default(),
            args: serde_json::Value::Null,
            inputs: serde_json::Value::Null,
        });
        dag.add_edge("1", "3");

        // Task 4 depends on 2 and 3
        dag.add_task(Task {
            task_id: "4".to_string(),
            name: "Task 4".to_string(),
            description: "".to_string(),
            priority: 1,
            status: Status::Pending,
            prompt: "".to_string(),
            executor: "log".to_string(),
            dependencies: vec!["2".to_string(), "3".to_string()],
            role: Default::default(),
            args: serde_json::Value::Null,
            inputs: serde_json::Value::Null,
        });
        dag.add_edge("2", "4");
        dag.add_edge("3", "4");

        dag
    }

    #[test]
    fn test_analyze_dag_structure() {
        let dag = create_test_dag();
        let result = analyze_dag_structure(&dag);

        assert!(!result.has_cycle);
        assert!(result.topological_order.is_some());
        assert_eq!(result.root_nodes, vec!["1".to_string()]);
        assert_eq!(result.leaf_nodes, vec!["4".to_string()]);
        assert!(result.orphan_nodes.is_empty());
    }

    #[test]
    fn test_node_depths() {
        let dag = create_test_dag();
        let result = analyze_dag_structure(&dag);

        assert_eq!(result.node_depths.get("1"), Some(&0));
        assert_eq!(result.node_depths.get("2"), Some(&1));
        assert_eq!(result.node_depths.get("3"), Some(&1));
        assert_eq!(result.node_depths.get("4"), Some(&2));
    }

    #[test]
    fn test_critical_path() {
        let dag = create_test_dag();
        let result = analyze_dag_structure(&dag);

        // Critical path should be 1 -> 2 -> 4 or 1 -> 3 -> 4
        assert_eq!(result.critical_path.len(), 3);
        assert_eq!(result.critical_path.first(), Some(&"1".to_string()));
        assert_eq!(result.critical_path.last(), Some(&"4".to_string()));
    }

    #[test]
    fn test_parallel_pairs() {
        let dag = create_test_dag();
        let result = analyze_dag_structure(&dag);

        // Task 2 and 3 can run in parallel
        assert!(result.parallel_pairs.contains(&("2".to_string(), "3".to_string())) ||
                result.parallel_pairs.contains(&("3".to_string(), "2".to_string())));
    }
}
