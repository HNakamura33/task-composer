//! サブグラフ（入れ子DAG）を実行するExecutor

use std::sync::Arc;
use async_trait::async_trait;
use crate::dag::DAG;
use crate::task_executor::{
    ExecutorRegistry, TaskExecutor, ExecutionContext,
    ExecutionResult, ExecutionStatus
};
use crate::types::Task;

/// サブグラフ（入れ子DAG）を実行するExecutor
///
/// タスクの`args.dag`フィールドに定義されたDAGを実行し、
/// その結果を集約して返します。
pub struct DagExecutor {
    /// サブDAGで使用するExecutorRegistry
    registry: Arc<ExecutorRegistry>,
}

impl DagExecutor {
    /// 新しいDagExecutorを作成
    ///
    /// # Arguments
    /// * `registry` - サブDAGで使用するExecutorRegistry
    pub fn new(registry: Arc<ExecutorRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl TaskExecutor for DagExecutor {
    fn name(&self) -> &str {
        "dag"
    }

    async fn execute_task(
        &self,
        task: &Task,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        // 1. ctx.argsから"dag"フィールドを取得
        let dag_value = ctx.args.get("dag")
            .ok_or_else(|| "Missing 'dag' field in args".to_string())?;

        // 2. ctx.argsから"inputs"フィールドを取得（オプショナル）
        // 親DAGで既に解決済みの値
        let inputs = ctx.args.get("inputs").cloned();

        // 3. DAGを作成
        let dag_str = serde_json::to_string(dag_value)
            .map_err(|e| format!("Failed to serialize dag: {}", e))?;
        let mut sub_dag = DAG::from_json(&dag_str)
            .map_err(|e| format!("Failed to parse sub-DAG: {}", e))?;

        // 4. Executorを登録（親のregistryを共有）
        sub_dag.set_registry(Arc::clone(&self.registry));

        // 5. 外部入力を設定（サブDAG内で $.inputs.xxx で参照可能）
        if let Some(inputs_value) = inputs {
            sub_dag.set_inputs(inputs_value);
        }

        // 6. execute_async()を呼ぶ
        let results = sub_dag.execute_async().await
            .map_err(|e| format!("Sub-DAG execution failed: {}", e))?;

        // 7. 結果をExecutionResultにまとめる
        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            status: ExecutionStatus::Success,
            output: serde_json::to_value(&results)
                .unwrap_or(serde_json::Value::Null),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_executor::LogExecutor;
    use std::collections::HashMap;

    fn create_test_registry() -> Arc<ExecutorRegistry> {
        let mut registry = ExecutorRegistry::new();
        registry.register(Box::new(LogExecutor::new()));
        Arc::new(registry)
    }

    fn create_test_task() -> Task {
        Task {
            task_id: "sub_dag_task".to_string(),
            name: Some("Sub DAG Task".to_string()),
            executor: "dag".to_string(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_dag_executor_name() {
        let registry = create_test_registry();
        let executor = DagExecutor::new(registry);
        assert_eq!(executor.name(), "dag");
    }

    #[tokio::test]
    async fn test_dag_executor_missing_dag_field() {
        let registry = create_test_registry();
        let executor = DagExecutor::new(registry);
        let task = create_test_task();
        let ctx = ExecutionContext {
            args: serde_json::json!({}),
            env_vars: HashMap::new(),
            previous_results: None,
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'dag' field"));
    }

    #[tokio::test]
    async fn test_dag_executor_simple_subdag() {
        let registry = create_test_registry();
        let executor = DagExecutor::new(registry);
        let task = create_test_task();

        let sub_dag_json = serde_json::json!({
            "tasks": [
                {
                    "task_id": "sub_task_1",
                    "name": "Sub Task 1",
                    "description": "A sub task",
                    "priority": 1,
                    "prompt": "",
                    "executor": "log",
                    "args": {},
                    "dependencies": [],
                    "role": {
                        "role_id": "test_role",
                        "name": "Test Role",
                        "description": "",
                        "subagents": [],
                        "skills": [],
                        "tool_permissions": {
                            "bash": {
                                "allowed_commands": [],
                                "blocked_commands": [],
                                "require_confirmation": []
                            },
                            "write": {
                                "max_file_size_mb": 10,
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
            ],
            "config": {
                "max_concurrent_tasks": 1
            }
        });

        let ctx = ExecutionContext {
            args: serde_json::json!({
                "dag": sub_dag_json
            }),
            env_vars: HashMap::new(),
            previous_results: None,
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok(), "Failed: {:?}", result.err());

        let execution_result = result.unwrap();
        assert_eq!(execution_result.task_id, "sub_dag_task");
        assert_eq!(execution_result.status, ExecutionStatus::Success);
    }

    #[tokio::test]
    async fn test_dag_executor_invalid_dag_json() {
        let registry = create_test_registry();
        let executor = DagExecutor::new(registry);
        let task = create_test_task();

        let ctx = ExecutionContext {
            args: serde_json::json!({
                "dag": "not a valid dag object"
            }),
            env_vars: HashMap::new(),
            previous_results: None,
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dag_executor_with_inputs() {
        // DataExecutorを追加
        use crate::task_executor::DataExecutor;
        let mut registry = ExecutorRegistry::new();
        registry.register(Box::new(DataExecutor::new()));
        let registry = Arc::new(registry);

        let executor = DagExecutor::new(Arc::clone(&registry));
        let task = create_test_task();

        // サブDAGで$.inputs.parent_valueを参照する
        let sub_dag_json = serde_json::json!({
            "tasks": [
                {
                    "task_id": "child_task",
                    "executor": "data",
                    "args": {
                        "value": "$.inputs.parent_value"
                    }
                }
            ]
        });

        // 親から渡すinputs
        let ctx = ExecutionContext {
            args: serde_json::json!({
                "inputs": {
                    "parent_value": 42
                },
                "dag": sub_dag_json
            }),
            env_vars: HashMap::new(),
            previous_results: None,
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok(), "Failed: {:?}", result.err());

        let execution_result = result.unwrap();
        assert_eq!(execution_result.status, ExecutionStatus::Success);

        // サブDAGの結果を確認
        let output = &execution_result.output;
        let child_result = output.get("child_task").unwrap();
        let child_output = child_result.get("output").unwrap();
        assert_eq!(child_output.get("value").unwrap(), &serde_json::json!(42));
    }

    #[tokio::test]
    async fn test_dag_executor_with_nested_inputs() {
        use crate::task_executor::DataExecutor;
        let mut registry = ExecutorRegistry::new();
        registry.register(Box::new(DataExecutor::new()));
        let registry = Arc::new(registry);

        let executor = DagExecutor::new(Arc::clone(&registry));
        let task = create_test_task();

        // サブDAG内で埋め込み参照を使用
        let sub_dag_json = serde_json::json!({
            "tasks": [
                {
                    "task_id": "child_task",
                    "executor": "data",
                    "args": {
                        "value": "Hello ${$.inputs.name}!"
                    }
                }
            ]
        });

        let ctx = ExecutionContext {
            args: serde_json::json!({
                "inputs": {
                    "name": "World"
                },
                "dag": sub_dag_json
            }),
            env_vars: HashMap::new(),
            previous_results: None,
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok(), "Failed: {:?}", result.err());

        let execution_result = result.unwrap();
        let output = &execution_result.output;
        let child_result = output.get("child_task").unwrap();
        let child_output = child_result.get("output").unwrap();
        assert_eq!(child_output.get("value").unwrap(), &serde_json::json!("Hello World!"));
    }
}
