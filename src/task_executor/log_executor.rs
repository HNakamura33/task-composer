//! LogExecutor - タスク情報をログ出力するシンプルなExecutor
//!
//! テストやデバッグ用途に使用します。

use super::{ExecutionContext, TaskExecutor};
use crate::types::{ExecutionResult, Task};
use serde_json::json;

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

impl TaskExecutor for LogExecutor {
    fn name(&self) -> &str {
        "log"
    }

    fn execute_task(
        &self,
        task: &Task,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        println!("========================================");
        println!("Executing Task: {}", task.name);
        println!("========================================");
        println!("  ID:          {}", task.task_id);
        println!("  Description: {}", task.description);
        println!("  Priority:    {}", task.priority);
        println!("  Prompt:      {}", task.prompt);
        println!("  Role:        {}", task.role.name);

        // 依存タスク情報
        if !task.dependencies.is_empty() {
            println!("  Dependencies: {:?}", task.dependencies);
        }

        // 引数
        if !ctx.args.is_null() {
            println!("  Args:        {}", ctx.args);
        }

        // 前回の結果
        if !ctx.previous_results.is_empty() {
            println!("  Previous Results:");
            for (task_id, result) in &ctx.previous_results {
                println!("    - {}: success={}, output={}", task_id, result.success, result.output);
            }
        }

        println!("========================================\n");

        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            success: true,
            output: json!({
                "executor": "log",
                "task_id": task.task_id,
                "message": "Task logged successfully"
            }),
        })
    }
}
