//! DataExecutor - 定数データを保持するExecutor
//!
//! リポジトリ情報、Issue情報、閾値などの定数を保持するデータノードとして使用します。
//! `args`フィールドの内容をそのまま`output`として返します。

use super::{ExecutionContext, ExecutionResult, ExecutionStatus, TaskExecutor};
use crate::types::Task;
use async_trait::async_trait;

/// 定数データを保持するExecutor
///
/// `args`フィールドの内容をそのまま`output`として返します。
/// 実際の処理は行わず、他のタスクから参照されるデータを提供します。
///
/// # Example
/// ```json
/// {
///   "task_id": "repo",
///   "executor": "data",
///   "args": {
///     "owner": "HNakamura33",
///     "repo": "task-composer"
///   }
/// }
/// ```
///
/// 参照方法: `$.repo.output.owner` → `"HNakamura33"`
pub struct DataExecutor;

impl DataExecutor {
    pub fn new() -> Self {
        DataExecutor
    }
}

impl Default for DataExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskExecutor for DataExecutor {
    fn name(&self) -> &str {
        "data"
    }

    async fn execute_task(
        &self,
        task: &Task,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        // argsをそのままoutputとして返す
        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            status: ExecutionStatus::Success,
            output: ctx.args.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_data_executor_name() {
        let executor = DataExecutor::new();
        assert_eq!(executor.name(), "data");
    }

    #[tokio::test]
    async fn test_data_executor_returns_args_as_output() {
        let executor = DataExecutor::new();
        let task = Task {
            task_id: "config".to_string(),
            executor: "data".to_string(),
            ..Default::default()
        };
        let ctx = ExecutionContext {
            args: json!({
                "owner": "HNakamura33",
                "repo": "task-composer",
                "threshold": 0.8
            }),
            env_vars: HashMap::new(),
        };

        let result = executor.execute_task(&task, &ctx).await.unwrap();

        assert_eq!(result.task_id, "config");
        assert_eq!(result.status, ExecutionStatus::Success);
        assert_eq!(result.output["owner"], "HNakamura33");
        assert_eq!(result.output["repo"], "task-composer");
        assert_eq!(result.output["threshold"], 0.8);
    }

    #[tokio::test]
    async fn test_data_executor_with_nested_data() {
        let executor = DataExecutor::new();
        let task = Task {
            task_id: "nested".to_string(),
            executor: "data".to_string(),
            ..Default::default()
        };
        let ctx = ExecutionContext {
            args: json!({
                "github": {
                    "owner": "HNakamura33",
                    "repo": "task-composer"
                },
                "issues": [10, 11, 12]
            }),
            env_vars: HashMap::new(),
        };

        let result = executor.execute_task(&task, &ctx).await.unwrap();

        assert_eq!(result.output["github"]["owner"], "HNakamura33");
        assert_eq!(result.output["issues"][0], 10);
        assert_eq!(result.output["issues"][2], 12);
    }

    #[tokio::test]
    async fn test_data_executor_with_empty_args() {
        let executor = DataExecutor::new();
        let task = Task {
            task_id: "empty".to_string(),
            executor: "data".to_string(),
            ..Default::default()
        };
        let ctx = ExecutionContext {
            args: json!(null),
            env_vars: HashMap::new(),
        };

        let result = executor.execute_task(&task, &ctx).await.unwrap();

        assert_eq!(result.status, ExecutionStatus::Success);
        assert!(result.output.is_null());
    }

    #[tokio::test]
    async fn test_data_executor_default() {
        let executor = DataExecutor::default();
        assert_eq!(executor.name(), "data");
    }
}
