//! Task Composer CLI - DAGベースのタスク実行ツール

use clap::{Parser, Subcommand};
use task_composer_core::dag::DAG;
use task_composer_core::analysis::StaticAnalyzer;
use std::sync::Arc;
use task_composer_core::task_executor::{BashExecutor, LogExecutor, McpExecutor, DagExecutor, DataExecutor, GitExecutor, GitHubExecutor, ExecutionStatus};

#[derive(Parser)]
#[command(name = "task-composer")]
#[command(about = "DAGベースのタスク管理・実行ツール", long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// DAGファイルのパス（後方互換性用）
    file: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// 静的解析のみを実行
    Analyze {
        /// DAGファイルのパス
        file: String,
    },
    /// 静的解析を実行してから実行（エラーがあれば中止）
    Run {
        /// DAGファイルのパス
        file: String,

        /// 静的解析でエラーがあっても実行を続行
        #[arg(short, long)]
        force: bool,
    },
    /// DAGを実行（静的解析なし）
    Exec {
        /// DAGファイルのパス
        file: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Analyze { file }) => {
            run_analyze(&file);
        }
        Some(Commands::Run { file, force }) => {
            run_with_analysis(&file, force).await;
        }
        Some(Commands::Exec { file }) => {
            run_execute_only(&file).await;
        }
        None => {
            // 後方互換性: コマンドなしの場合は従来通り実行のみ
            let file = cli.file.unwrap_or_else(|| "samples/basics/simple_dag.json".to_string());
            run_execute_only(&file).await;
        }
    }
}

/// 静的解析のみを実行
fn run_analyze(file: &str) {
    let dag = match load_dag(file) {
        Ok(dag) => dag,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    println!("=== Static Analysis: {} ===\n", file);

    let analyzer = StaticAnalyzer::new(&dag);
    let result = analyzer.analyze();

    // DAG構造情報
    print_dag_structure(&result.dag_structure);

    // 検証結果
    print_validation_results(&result);

    // サマリー
    println!("\n=== Summary ===");
    let error_count = result.error_count();
    let warning_count = result.warning_count();

    if error_count > 0 {
        println!("  Errors:   {}", error_count);
    }
    if warning_count > 0 {
        println!("  Warnings: {}", warning_count);
    }
    if error_count == 0 && warning_count == 0 {
        println!("  No issues found.");
    }

    if error_count > 0 {
        std::process::exit(1);
    }
}

/// 静的解析後に実行
async fn run_with_analysis(file: &str, force: bool) {
    let mut dag = match load_dag(file) {
        Ok(dag) => dag,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    println!("=== Static Analysis: {} ===\n", file);

    let analyzer = StaticAnalyzer::new(&dag);
    let result = analyzer.analyze();

    // 検証結果を表示
    print_validation_results(&result);

    let error_count = result.error_count();
    let warning_count = result.warning_count();

    println!("\n=== Analysis Summary ===");
    println!("  Errors:   {}", error_count);
    println!("  Warnings: {}", warning_count);

    if error_count > 0 && !force {
        eprintln!("\nExecution aborted due to errors.");
        eprintln!("Use --force to execute anyway.");
        std::process::exit(1);
    }

    if error_count > 0 && force {
        println!("\nWarning: Continuing execution despite {} error(s).", error_count);
    }

    // 実行
    println!("\n=== Executing DAG ===\n");
    execute_dag(&mut dag).await;
}

/// 実行のみ（後方互換性）
async fn run_execute_only(file: &str) {
    let mut dag = match load_dag(file) {
        Ok(dag) => dag,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // 読み込んだDAGの情報を表示
    println!("Loaded {} tasks", dag.nodes.len());

    for (task_id, task) in &dag.nodes {
        println!("  Task {}: {}", task_id, task.display_name());
    }

    println!("\nEdges:");
    for (from, to_list) in &dag.edges {
        for to in to_list {
            println!("  {} -> {}", from, to);
        }
    }

    // DAGを実行
    println!("\n=== Executing DAG ===\n");
    execute_dag(&mut dag).await;
}

/// DAGをファイルから読み込む
fn load_dag(file: &str) -> Result<DAG, String> {
    let json = std::fs::read_to_string(file)
        .map_err(|e| format!("Failed to read {}: {}", file, e))?;

    DAG::from_json(&json)
        .map_err(|e| format!("Failed to parse JSON: {}", e))
}

/// ネストしたサブグラフ用のregistryを再帰的に作成
///
/// # Arguments
/// * `depth` - 残りのネスト深度（0になるとDagExecutorを含まない）
fn create_registry_with_depth(depth: usize) -> Arc<task_composer_core::task_executor::ExecutorRegistry> {
    let mut registry = task_composer_core::task_executor::ExecutorRegistry::new();
    registry.register(Box::new(BashExecutor::new()));
    registry.register(Box::new(LogExecutor::new()));
    registry.register(Box::new(DataExecutor::new()));
    registry.register(Box::new(McpExecutor::new()));
    registry.register(Box::new(GitExecutor::new()));
    // GITHUB_TOKEN環境変数からトークンを取得
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        registry.register(Box::new(GitHubExecutor::with_token(token)));
    } else {
        registry.register(Box::new(GitHubExecutor::new()));
    }
    if depth > 0 {
        let sub_registry = create_registry_with_depth(depth - 1);
        registry.register(Box::new(DagExecutor::new(sub_registry)));
    }
    Arc::new(registry)
}

/// DAGを実行
async fn execute_dag(dag: &mut DAG) {
    // 最大3レベルのネストをサポート
    const MAX_SUBGRAPH_DEPTH: usize = 3;
    let registry = create_registry_with_depth(MAX_SUBGRAPH_DEPTH);
    dag.set_registry(registry);

    let start = std::time::Instant::now();
    match dag.execute_async().await {
        Ok(results) => {
            let elapsed = start.elapsed();
            println!("\n=== Execution Complete ===");
            println!("Executed {} tasks in {:.2?}", results.len(), elapsed);
            for (task_id, result) in &results {
                let status = match result.status {
                    ExecutionStatus::Success => "OK",
                    ExecutionStatus::Failed => "FAILED",
                    ExecutionStatus::Skipped => "SKIPPED",
                };
                println!("  {}: {}", task_id, status);
            }
        }
        Err(e) => {
            eprintln!("Execution failed: {}", e);
            std::process::exit(1);
        }
    }
}

/// DAG構造情報を表示
fn print_dag_structure(structure: &task_composer_core::analysis::DagStructureAnalysis) {
    println!("DAG Structure:");
    println!("  Root nodes:  {:?}", structure.root_nodes);
    println!("  Leaf nodes:  {:?}", structure.leaf_nodes);

    if structure.has_cycle {
        println!("  [ERROR] Cycle detected!");
    } else if let Some(ref order) = structure.topological_order {
        println!("  Topological order: {:?}", order);
    }

    if !structure.orphan_nodes.is_empty() {
        println!("  [WARNING] Orphan nodes: {:?}", structure.orphan_nodes);
    }

    if !structure.parallel_pairs.is_empty() {
        println!("  Parallel pairs: {} pair(s)", structure.parallel_pairs.len());
    }

    if !structure.critical_path.is_empty() {
        println!("  Critical path: {:?}", structure.critical_path);
    }
}

/// 検証結果を表示
fn print_validation_results(result: &task_composer_core::analysis::AnalysisResult) {
    if result.items.is_empty() {
        return;
    }

    println!("\nValidation Results:");
    for item in &result.items {
        let prefix = match item.level {
            task_composer_core::analysis::AnalysisLevel::Error => "[ERROR]",
            task_composer_core::analysis::AnalysisLevel::Warning => "[WARN]",
            task_composer_core::analysis::AnalysisLevel::Info => "[INFO]",
        };
        println!("  {} {}", prefix, item.message);
        if !item.related_tasks.is_empty() {
            println!("         Related: {:?}", item.related_tasks);
        }
    }
}
