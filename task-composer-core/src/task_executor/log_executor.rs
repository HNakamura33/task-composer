//! LogExecutor - タスク情報をログ出力するシンプルなExecutor
//!
//! テストやデバッグ用途に使用します。

use super::{ExecutionContext, ExecutionResult, ExecutionStatus, TaskExecutor};
use crate::types::Task;
use async_trait::async_trait;
use rand::Rng;
use serde_json::json;
use tokio::time::{Duration, sleep};

/// タスク情報をログ出力するExecutor
///
/// 実際の処理は行わず、タスク情報と入力データをコンソールに出力します。
pub struct LogExecutor;

impl LogExecutor {
    pub fn new() -> Self {
        LogExecutor
    }
}

impl Default for LogExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskExecutor for LogExecutor {
    fn name(&self) -> &str {
        "log"
    }

    async fn execute_task(
        &self,
        task: &Task,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        // ランダムな遅延（100ms〜1000ms）でタスク実行時間をシミュレート
        let delay_ms = rand::thread_rng().gen_range(100..=1000);
        sleep(Duration::from_millis(delay_ms)).await;

        println!("Executing Task: {}", task.display_name());
        println!("  ID:          {}", task.task_id);
        println!("  Description: {}", task.description.as_deref().unwrap_or(""));
        println!("  Priority:    {}", task.priority);
        println!("  Prompt:      {}", task.prompt.as_deref().unwrap_or(""));
        println!("  Role:        {}", task.role.name);

        // 依存タスク情報
        if !task.dependencies.is_empty() {
            println!("  Dependencies: {:?}", task.dependencies);
        }

        // 引数（inputsから解決した値を含む）
        if !ctx.args.is_null() {
            println!("  Args:        {}", ctx.args);
        }

        let result_message = format!("Task '{}' logged successfully", task.display_name());
        println!("\n  --- Result ---");
        println!("  {}", result_message);
        println!("  --- End ---\n");

        println!("  [Task {} completed]", task.task_id);

        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            status: ExecutionStatus::Success,
            output: json!({
                "executor": "log",
                "task_id": task.task_id,
                "message": result_message
            }),
        })
    }
}
