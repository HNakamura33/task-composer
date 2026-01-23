//! ReduceExecutor - 配列の要素を集約して単一の値に変換するExecutor
//!
//! ## 概要
//! 配列の各要素に対して逐次的にサブDAGを実行し、累積値（accumulator）を更新します。
//! 最終的な累積値が出力となります。
//!
//! ## args フィールド
//! - `source`: 処理対象配列へのパス参照（必須）
//! - `initial`: アキュムレータの初期値（必須）
//! - `inputs`: サブDAGへ渡す入力のマッピング（必須）
//! - `dag`: 各要素に対して実行するサブDAG（必須）
//! - `output_task`: 出力として使用するタスクID（デフォルト: 最後のタスク）
//!
//! ## 特殊変数
//! inputs内で以下の特殊変数が使用可能:
//! - `$.@accumulator`: 現在の累積値
//! - `$.@item`: 現在処理中の要素
//! - `$.@index`: 0始まりのインデックス
//! - `$.@length`: 配列の総要素数
//! - `$.@first`: 最初の要素かどうか（boolean）
//! - `$.@last`: 最後の要素かどうか（boolean）
//!
//! ## 使用例
//! ```json
//! {
//!   "task_id": "sum_values",
//!   "executor": "reduce",
//!   "args": {
//!     "source": "$.fetch_items.output.items",
//!     "initial": { "total": 0, "count": 0 },
//!     "inputs": {
//!       "acc": "$.@accumulator",
//!       "item": "$.@item"
//!     },
//!     "dag": {
//!       "tasks": [
//!         {
//!           "task_id": "add",
//!           "executor": "data",
//!           "args": {
//!             "total": "${$.inputs.acc.total} + ${$.inputs.item.value}",
//!             "count": "${$.inputs.acc.count} + 1"
//!           }
//!         }
//!       ]
//!     }
//!   }
//! }
//! ```
//!
//! ## 出力
//! ```json
//! {
//!   "result": { ... },  // 最終的なアキュムレータの値
//!   "iterations": 5     // 処理した要素数
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use crate::dag::DAG;
use crate::path_resolver::{resolve_inputs, ResolveContext};
use crate::task_executor::{
    ExecutionContext, ExecutionResult, ExecutionStatus, ExecutorRegistry, TaskExecutor,
};
use crate::types::{MapContext, ReduceContext, Task};

/// ReduceExecutor - 配列の要素を集約して単一の値に変換
pub struct ReduceExecutor {
    /// ExecutorRegistry（サブDAGで使用）
    registry: Arc<ExecutorRegistry>,
}

impl ReduceExecutor {
    /// 新しいReduceExecutorを作成
    pub fn new(registry: Arc<ExecutorRegistry>) -> Self {
        ReduceExecutor { registry }
    }
}

#[async_trait]
impl TaskExecutor for ReduceExecutor {
    fn name(&self) -> &str {
        "reduce"
    }

    async fn execute_task(
        &self,
        task: &Task,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        // === パラメータの抽出 ===

        // source: 処理対象配列（解決済みの配列またはパス参照文字列）
        let source_value = ctx
            .args
            .get("source")
            .ok_or("ReduceExecutor: 'source' field is required")?;

        // initial: アキュムレータの初期値（必須）
        let initial = ctx
            .args
            .get("initial")
            .ok_or("ReduceExecutor: 'initial' field is required")?
            .clone();

        // inputs: サブDAGへ渡す入力のマッピング（必須）
        let inputs_template = ctx
            .args
            .get("inputs")
            .ok_or("ReduceExecutor: 'inputs' field is required")?
            .clone();

        // dag: サブDAG定義（必須）
        let dag_value = ctx
            .args
            .get("dag")
            .ok_or("ReduceExecutor: 'dag' field is required")?;

        // output_task: 出力として使用するタスクID（オプション）
        let output_task = ctx
            .args
            .get("output_task")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // === source配列の取得 ===

        let empty_results = HashMap::new();
        let resolve_ctx = ResolveContext {
            previous_results: ctx.previous_results.as_ref().unwrap_or(&empty_results),
            current_task: Some(task),
            loop_context: None,
            inputs: None,
            map_context: None,
            reduce_context: None,
        };

        // sourceが既に配列の場合はそのまま使用、文字列の場合はパス参照として解決
        let array = if let Some(arr) = source_value.as_array() {
            arr.clone()
        } else if let Some(path) = source_value.as_str() {
            let resolved = resolve_inputs(&json!(path), &resolve_ctx)
                .map_err(|e| format!("ReduceExecutor: Failed to resolve source '{}': {}", path, e))?;
            resolved
                .as_array()
                .ok_or_else(|| {
                    format!(
                        "ReduceExecutor: source must resolve to an array, got: {:?}",
                        resolved
                    )
                })?
                .clone()
        } else {
            return Err(format!(
                "ReduceExecutor: source must be an array or path string, got: {:?}",
                source_value
            ));
        };

        let array_len = array.len();

        if array_len == 0 {
            // 空配列の場合は初期値を返す
            return Ok(ExecutionResult {
                task_id: task.task_id.clone(),
                status: ExecutionStatus::Success,
                output: json!({
                    "result": initial,
                    "iterations": 0
                }),
            });
        }

        // === 各要素の処理（逐次実行） ===

        let mut accumulator = initial;

        for (index, item) in array.iter().enumerate() {
            // MapContextを作成
            let map_ctx = MapContext::new(item.clone(), index, array_len);

            // ReduceContextを作成
            let reduce_ctx = ReduceContext {
                accumulator: accumulator.clone(),
            };

            // inputs解決用のコンテキスト
            let input_resolve_ctx = ResolveContext {
                previous_results: ctx.previous_results.as_ref().unwrap_or(&empty_results),
                current_task: Some(task),
                loop_context: None,
                inputs: None,
                map_context: Some(&map_ctx),
                reduce_context: Some(&reduce_ctx),
            };

            // inputs_templateを解決
            let resolved_inputs = resolve_inputs(&inputs_template, &input_resolve_ctx)
                .map_err(|e| {
                    format!(
                        "ReduceExecutor: Failed to resolve inputs at index {}: {}",
                        index, e
                    )
                })?;

            // サブDAGを作成
            let dag_str = serde_json::to_string(dag_value)
                .map_err(|e| format!("ReduceExecutor: Failed to serialize DAG: {}", e))?;

            let mut sub_dag = DAG::from_json(&dag_str)
                .map_err(|e| format!("ReduceExecutor: Failed to parse DAG: {}", e))?;

            // レジストリとinputsを設定
            sub_dag.set_registry(Arc::clone(&self.registry));
            sub_dag.set_inputs(resolved_inputs);

            // サブDAGを実行
            let results = sub_dag.execute_async().await.map_err(|e| {
                format!(
                    "ReduceExecutor: Failed to execute DAG at index {}: {}",
                    index, e
                )
            })?;

            // 結果からアキュムレータを更新
            // output_taskが指定されていればそれを使用、なければ最後のタスクを使用
            let new_accumulator = if let Some(ref task_id) = output_task {
                results
                    .get(task_id)
                    .map(|r| r.output.clone())
                    .ok_or_else(|| {
                        format!(
                            "ReduceExecutor: output_task '{}' not found in results",
                            task_id
                        )
                    })?
            } else {
                // 最後のタスクの出力を使用
                // トポロジカルソートの最後のタスクを探す
                let last_result = results.values().last().map(|r| r.output.clone());
                last_result.ok_or("ReduceExecutor: No results from sub-DAG")?
            };

            accumulator = new_accumulator;
        }

        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            status: ExecutionStatus::Success,
            output: json!({
                "result": accumulator,
                "iterations": array_len
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_reduce_executor_name() {
        let registry = Arc::new(ExecutorRegistry::new());
        let executor = ReduceExecutor::new(registry);
        assert_eq!(executor.name(), "reduce");
    }

    #[tokio::test]
    async fn test_reduce_executor_missing_source() {
        let registry = Arc::new(ExecutorRegistry::new());
        let executor = ReduceExecutor::new(registry);

        let task = Task {
            task_id: "test".to_string(),
            executor: "reduce".to_string(),
            ..Default::default()
        };

        let ctx = ExecutionContext {
            args: json!({
                "initial": 0,
                "inputs": {},
                "dag": {"tasks": []}
            }),
            env_vars: HashMap::new(),
            previous_results: None,
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("source"));
    }

    #[tokio::test]
    async fn test_reduce_executor_missing_initial() {
        let registry = Arc::new(ExecutorRegistry::new());
        let executor = ReduceExecutor::new(registry);

        let task = Task {
            task_id: "test".to_string(),
            executor: "reduce".to_string(),
            ..Default::default()
        };

        let mut previous = HashMap::new();
        previous.insert(
            "source_task".to_string(),
            ExecutionResult {
                task_id: "source_task".to_string(),
                status: ExecutionStatus::Success,
                output: json!({"items": [1, 2, 3]}),
            },
        );

        let ctx = ExecutionContext {
            args: json!({
                "source": "$.source_task.output.items",
                "inputs": {},
                "dag": {"tasks": []}
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("initial"));
    }

    #[tokio::test]
    async fn test_reduce_executor_missing_inputs() {
        let registry = Arc::new(ExecutorRegistry::new());
        let executor = ReduceExecutor::new(registry);

        let task = Task {
            task_id: "test".to_string(),
            executor: "reduce".to_string(),
            ..Default::default()
        };

        let mut previous = HashMap::new();
        previous.insert(
            "source_task".to_string(),
            ExecutionResult {
                task_id: "source_task".to_string(),
                status: ExecutionStatus::Success,
                output: json!({"items": [1, 2, 3]}),
            },
        );

        let ctx = ExecutionContext {
            args: json!({
                "source": "$.source_task.output.items",
                "initial": 0,
                "dag": {"tasks": []}
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("inputs"));
    }

    #[tokio::test]
    async fn test_reduce_executor_missing_dag() {
        let registry = Arc::new(ExecutorRegistry::new());
        let executor = ReduceExecutor::new(registry);

        let task = Task {
            task_id: "test".to_string(),
            executor: "reduce".to_string(),
            ..Default::default()
        };

        let mut previous = HashMap::new();
        previous.insert(
            "source_task".to_string(),
            ExecutionResult {
                task_id: "source_task".to_string(),
                status: ExecutionStatus::Success,
                output: json!({"items": [1, 2, 3]}),
            },
        );

        let ctx = ExecutionContext {
            args: json!({
                "source": "$.source_task.output.items",
                "initial": 0,
                "inputs": {}
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dag"));
    }

    #[tokio::test]
    async fn test_reduce_executor_empty_array() {
        let registry = Arc::new(ExecutorRegistry::new());
        let executor = ReduceExecutor::new(registry);

        let task = Task {
            task_id: "test".to_string(),
            executor: "reduce".to_string(),
            ..Default::default()
        };

        let mut previous = HashMap::new();
        previous.insert(
            "source_task".to_string(),
            ExecutionResult {
                task_id: "source_task".to_string(),
                status: ExecutionStatus::Success,
                output: json!({"items": []}),
            },
        );

        let ctx = ExecutionContext {
            args: json!({
                "source": "$.source_task.output.items",
                "initial": {"total": 0},
                "inputs": {"acc": "$.@accumulator", "item": "$.@item"},
                "dag": {"tasks": []}
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap().output;
        assert_eq!(output["result"], json!({"total": 0}));
        assert_eq!(output["iterations"], 0);
    }
}
