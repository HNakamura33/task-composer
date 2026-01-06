//! Task Composer - DAGベースのタスク管理ライブラリ
//!
//! 有向非巡回グラフ(DAG)を使ってタスクの依存関係を管理します。

mod types;
mod dag;
mod conflict;
mod task_executor;
mod path_resolver;

use crate::dag::DAG;
use crate::task_executor::{LogExecutor, McpExecutor};

#[tokio::main]
async fn main() {
    // DAGファイルを読み込む
    let dag_file = std::env::args().nth(1).unwrap_or("sample_dag.json".to_string());
    let json = std::fs::read_to_string(&dag_file)
        .expect(&format!("Failed to read {}", dag_file));

    // JSONからDAGを作成
    let mut dag = DAG::from_json(&json)
        .expect("Failed to parse JSON");

    // Executorを登録
    dag.register_executor(Box::new(LogExecutor::new()));
    dag.register_executor(Box::new(McpExecutor::new()));

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
    let start = std::time::Instant::now();
    match dag.execute_async().await {
        Ok(results) => {
            let elapsed = start.elapsed();
            println!("\n=== Execution Complete ===");
            println!("Executed {} tasks in {:.2?}", results.len(), elapsed);
            println!("(Sequential would take ~4 seconds)");
            for (task_id, result) in &results {
                println!("  {}: success={}", task_id, result.success);
            }
        }
        Err(e) => {
            eprintln!("Execution failed: {}", e);
        }
    }
}
