//! Task Composer - DAGベースのタスク管理ライブラリ
//!
//! 有向非巡回グラフ(DAG)を使ってタスクの依存関係を管理します。

mod types;
mod dag;
mod conflict;
mod task_executor;

use crate::dag::DAG;
use crate::task_executor::LogExecutor;

fn main() {
    // sample_dag.json を読み込む
    let json = std::fs::read_to_string("sample_dag.json")
        .expect("Failed to read sample_dag.json");

    // JSONからDAGを作成
    let mut dag = DAG::from_json(&json)
        .expect("Failed to parse JSON");

    // LogExecutor を登録
    dag.task_manager.registry.register(Box::new(LogExecutor::new()));

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

    // DAGを実行
    println!("\n=== Executing DAG ===\n");
    match dag.execute() {
        Ok(results) => {
            println!("\n=== Execution Complete ===");
            println!("Executed {} tasks", results.len());
            for (task_id, result) in &results {
                println!("  {}: success={}", task_id, result.success);
            }
        }
        Err(e) => {
            eprintln!("Execution failed: {}", e);
        }
    }
}
