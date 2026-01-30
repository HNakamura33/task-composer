//! Task Composer CLI - DAGベースのタスク実行ツール

mod signal;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use task_composer_core::dag::DAG;
use task_composer_core::analysis::StaticAnalyzer;
use task_composer_core::checkpoint::{Checkpoint, CheckpointState};
use task_composer_core::checkpoint::writer::{CheckpointWriter, JsonCheckpointWriter};
use task_composer_core::task_executor::{
    BashExecutor, LogExecutor, McpExecutor, DagExecutor, DataExecutor,
    GitExecutor, GitHubExecutor, MapExecutor, FilterExecutor, ReduceExecutor,
    ExecutionStatus,
};

use signal::SignalState;

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

        /// チェックポイントファイルのパス（デフォルト: {file}.checkpoint.json）
        #[arg(long)]
        checkpoint: Option<PathBuf>,

        /// チェックポイントを無効化
        #[arg(long)]
        no_checkpoint: bool,
    },
    /// DAGを実行（静的解析なし）
    Exec {
        /// DAGファイルのパス
        file: String,

        /// チェックポイントファイルのパス（デフォルト: {file}.checkpoint.json）
        #[arg(long)]
        checkpoint: Option<PathBuf>,

        /// チェックポイントを無効化
        #[arg(long)]
        no_checkpoint: bool,
    },
    /// チェックポイントから再開
    Resume {
        /// チェックポイントファイルのパス
        checkpoint_file: PathBuf,

        /// DAGファイルのパス（省略時はチェックポイントに記録されたパスを使用）
        #[arg(long)]
        dag: Option<PathBuf>,

        /// DAG変更警告を無視
        #[arg(long)]
        ignore_dag_changes: bool,
    },
    /// チェックポイントの状態を表示
    Status {
        /// チェックポイントファイルのパス
        checkpoint_file: PathBuf,
    },
    /// チェックポイントファイルを削除
    Clean {
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
        Some(Commands::Run { file, force, checkpoint, no_checkpoint }) => {
            run_with_analysis(&file, force, checkpoint, no_checkpoint).await;
        }
        Some(Commands::Exec { file, checkpoint, no_checkpoint }) => {
            run_execute_only(&file, checkpoint, no_checkpoint).await;
        }
        Some(Commands::Resume { checkpoint_file, dag, ignore_dag_changes }) => {
            run_resume(&checkpoint_file, dag, ignore_dag_changes).await;
        }
        Some(Commands::Status { checkpoint_file }) => {
            run_status(&checkpoint_file);
        }
        Some(Commands::Clean { file }) => {
            run_clean(&file);
        }
        None => {
            // 後方互換性: コマンドなしの場合は従来通り実行のみ（チェックポイントなし）
            let file = cli.file.unwrap_or_else(|| "samples/basics/simple_dag.json".to_string());
            run_execute_only(&file, None, true).await;
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
async fn run_with_analysis(file: &str, force: bool, checkpoint_path: Option<PathBuf>, no_checkpoint: bool) {
    let (mut dag, dag_json) = match load_dag_with_json(file) {
        Ok(result) => result,
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
    execute_dag_with_checkpoint(&mut dag, file, &dag_json, checkpoint_path, no_checkpoint).await;
}

/// 実行のみ（後方互換性）
async fn run_execute_only(file: &str, checkpoint_path: Option<PathBuf>, no_checkpoint: bool) {
    let (mut dag, dag_json) = match load_dag_with_json(file) {
        Ok(result) => result,
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
    execute_dag_with_checkpoint(&mut dag, file, &dag_json, checkpoint_path, no_checkpoint).await;
}

/// DAGをファイルから読み込む
fn load_dag(file: &str) -> Result<DAG, String> {
    let json = std::fs::read_to_string(file)
        .map_err(|e| format!("Failed to read {}: {}", file, e))?;

    DAG::from_json(&json)
        .map_err(|e| format!("Failed to parse JSON: {}", e))
}

/// DAGをファイルから読み込み、JSONも返す
fn load_dag_with_json(file: &str) -> Result<(DAG, String), String> {
    let json = std::fs::read_to_string(file)
        .map_err(|e| format!("Failed to read {}: {}", file, e))?;

    let dag = DAG::from_json(&json)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    Ok((dag, json))
}

/// チェックポイントから再開
async fn run_resume(checkpoint_file: &PathBuf, dag_path: Option<PathBuf>, _ignore_dag_changes: bool) {
    // チェックポイントを読み込み
    let writer = JsonCheckpointWriter::new(checkpoint_file);
    let checkpoint = match writer.load() {
        Ok(Some(cp)) => cp,
        Ok(None) => {
            eprintln!("Error: Checkpoint file not found: {}", checkpoint_file.display());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: Failed to load checkpoint: {}", e);
            std::process::exit(1);
        }
    };

    // DAGファイルパスを決定
    let dag_file = dag_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| checkpoint.dag_file.clone());

    println!("=== Resuming from Checkpoint ===");
    println!("  Checkpoint: {}", checkpoint_file.display());
    println!("  DAG file:   {}", dag_file);
    println!("  State:      {:?}", checkpoint.state);
    println!("  Completed:  {} tasks", checkpoint.completed_count());
    println!("  Failed:     {} tasks", checkpoint.failed_count());
    println!("  Skipped:    {} tasks", checkpoint.skipped_count());
    println!();

    // DAGを読み込んで実行
    let (mut dag, dag_json) = match load_dag_with_json(&dag_file) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    execute_dag_with_checkpoint(&mut dag, &dag_file, &dag_json, Some(checkpoint_file.clone()), false).await;
}

/// チェックポイントの状態を表示
fn run_status(checkpoint_file: &PathBuf) {
    let writer = JsonCheckpointWriter::new(checkpoint_file);
    let checkpoint = match writer.load() {
        Ok(Some(cp)) => cp,
        Ok(None) => {
            eprintln!("Error: Checkpoint file not found: {}", checkpoint_file.display());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error: Failed to load checkpoint: {}", e);
            std::process::exit(1);
        }
    };

    println!("=== Checkpoint Status ===");
    println!("  File:       {}", checkpoint_file.display());
    println!("  Version:    {}", checkpoint.version);
    println!("  DAG file:   {}", checkpoint.dag_file);
    println!("  DAG hash:   {}", checkpoint.dag_hash);
    println!("  Created:    {}", checkpoint.created_at);
    println!("  Updated:    {}", checkpoint.updated_at);
    println!();

    println!("Execution State: {:?}", checkpoint.state);
    println!();

    println!("Task Summary:");
    println!("  Completed:  {} tasks", checkpoint.completed_count());
    println!("  Failed:     {} tasks", checkpoint.failed_count());
    println!("  Skipped:    {} tasks", checkpoint.skipped_count());
    println!();

    if !checkpoint.tasks.is_empty() {
        println!("Task Details:");
        for (task_id, task_cp) in &checkpoint.tasks {
            let status = match task_cp.status {
                ExecutionStatus::Success => "OK",
                ExecutionStatus::Failed => "FAILED",
                ExecutionStatus::Skipped => "SKIPPED",
            };
            println!("  {}: {} ({})", task_id, status, task_cp.completed_at);
        }
    }

    if let Some(ref loop_state) = checkpoint.loop_state {
        println!();
        println!("Loop State:");
        println!("  Completed iterations: {}", loop_state.iterations.len());
        println!("  Current iteration: {}", loop_state.current_iteration);
    }
}

/// チェックポイントファイルを削除
fn run_clean(file: &str) {
    let checkpoint_path = format!("{}.checkpoint.json", file);
    let path = std::path::Path::new(&checkpoint_path);

    if path.exists() {
        match std::fs::remove_file(path) {
            Ok(_) => println!("Removed checkpoint: {}", checkpoint_path),
            Err(e) => {
                eprintln!("Error: Failed to remove checkpoint: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        println!("No checkpoint file found: {}", checkpoint_path);
    }
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
    registry.register(Box::new(FilterExecutor::new()));
    // GITHUB_TOKEN環境変数からトークンを取得
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        registry.register(Box::new(GitHubExecutor::with_token(token)));
    } else {
        registry.register(Box::new(GitHubExecutor::new()));
    }
    if depth > 0 {
        let sub_registry = create_registry_with_depth(depth - 1);
        registry.register(Box::new(DagExecutor::new(Arc::clone(&sub_registry))));
        registry.register(Box::new(MapExecutor::new(Arc::clone(&sub_registry))));
        registry.register(Box::new(ReduceExecutor::new(sub_registry)));
    }
    Arc::new(registry)
}

/// DAGを実行（チェックポイント対応）
async fn execute_dag_with_checkpoint(
    dag: &mut DAG,
    dag_file: &str,
    dag_json: &str,
    checkpoint_path: Option<PathBuf>,
    no_checkpoint: bool,
) {
    // 最大3レベルのネストをサポート
    const MAX_SUBGRAPH_DEPTH: usize = 3;
    let registry = create_registry_with_depth(MAX_SUBGRAPH_DEPTH);
    dag.set_registry(registry);

    let start = std::time::Instant::now();

    if no_checkpoint {
        // チェックポイントなしで実行
        match dag.execute_async().await {
            Ok(results) => {
                print_execution_results(&results, start.elapsed());
            }
            Err(e) => {
                eprintln!("Execution failed: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // シグナルハンドラーをセットアップ
        let signal_state = SignalState::new();
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let shutdown_flag_clone = Arc::clone(&shutdown_flag);

        // シグナルハンドラーを起動
        let signal_state_clone = Arc::clone(&signal_state);
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                eprintln!("\n[Signal] Ctrl+C received. Saving checkpoint...");
                signal_state_clone.request_shutdown();
                shutdown_flag_clone.store(true, Ordering::SeqCst);
            }
        });

        // チェックポイント付きで実行
        let cp_path = checkpoint_path.as_deref();
        match dag.execute_with_checkpoint(dag_file, dag_json, cp_path, Some(shutdown_flag)).await {
            Ok((results, state)) => {
                print_execution_results(&results, start.elapsed());
                print_checkpoint_state(&state, dag_file, checkpoint_path);
            }
            Err(e) => {
                eprintln!("Execution failed: {}", e);
                std::process::exit(1);
            }
        }
    }
}

/// 実行結果を表示
fn print_execution_results(
    results: &std::collections::HashMap<String, task_composer_core::task_executor::ExecutionResult>,
    elapsed: std::time::Duration,
) {
    println!("\n=== Execution Complete ===");
    println!("Executed {} tasks in {:.2?}", results.len(), elapsed);
    for (task_id, result) in results {
        let status = match result.status {
            ExecutionStatus::Success => "OK",
            ExecutionStatus::Failed => "FAILED",
            ExecutionStatus::Skipped => "SKIPPED",
        };
        println!("  {}: {}", task_id, status);
    }
}

/// チェックポイント状態を表示
fn print_checkpoint_state(state: &CheckpointState, dag_file: &str, checkpoint_path: Option<PathBuf>) {
    let cp_path = checkpoint_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("{}.checkpoint.json", dag_file));

    match state {
        CheckpointState::Completed => {
            println!("\nCheckpoint saved: {}", cp_path);
        }
        CheckpointState::Interrupted => {
            println!("\n[Interrupted] Checkpoint saved: {}", cp_path);
            println!("Resume with: task-composer resume {}", cp_path);
        }
        CheckpointState::Failed { failed_task, error } => {
            println!("\n[Failed] Task '{}' failed: {}", failed_task, error);
            println!("Checkpoint saved: {}", cp_path);
            println!("Resume with: task-composer resume {}", cp_path);
        }
        CheckpointState::Running => {
            // 通常ここには来ない
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
