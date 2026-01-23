//! FilterExecutor - 配列の要素を条件式でフィルタリングするExecutor
//!
//! ## 概要
//! 配列の各要素に対して条件式を評価し、条件を満たす要素のみを抽出します。
//! サブDAGは実行せず、条件式のみで判定します。
//!
//! ## args フィールド
//! - `source`: 処理対象配列へのパス参照（必須）
//! - `condition`: フィルタリング条件式（必須）
//!
//! ## 特殊変数
//! 条件式内で以下の特殊変数が使用可能:
//! - `$.@item`: 現在処理中の要素
//! - `$.@index`: 0始まりのインデックス
//! - `$.@length`: 配列の総要素数
//! - `$.@first`: 最初の要素かどうか（boolean）
//! - `$.@last`: 最後の要素かどうか（boolean）
//!
//! ## 条件式の例
//! - `$.@item.is_active == true`
//! - `$.@item.count > 10`
//! - `$.@item.status == "completed"`
//! - `$.@index < 5` (最初の5要素のみ)
//! - `$.@first == false` (最初の要素以外)
//!
//! ## 使用例
//! ```json
//! {
//!   "task_id": "active_users",
//!   "executor": "filter",
//!   "args": {
//!     "source": "$.fetch_users.output.users",
//!     "condition": "$.@item.is_active == true"
//!   }
//! }
//! ```
//!
//! ## 出力
//! ```json
//! {
//!   "items": [...],           // フィルタリングされた要素の配列
//!   "original_length": 10,    // 元の配列の長さ
//!   "filtered_length": 5,     // フィルタリング後の長さ
//!   "indices": [0, 2, 4, 6, 8] // 元の配列でのインデックス
//! }
//! ```

use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::json;

use crate::path_resolver::{evaluate_condition, resolve_inputs, ResolveContext};
use crate::task_executor::{ExecutionContext, ExecutionResult, ExecutionStatus, TaskExecutor};
use crate::types::{MapContext, Task};

/// FilterExecutor - 配列の要素を条件式でフィルタリング
pub struct FilterExecutor;

impl FilterExecutor {
    /// 新しいFilterExecutorを作成
    pub fn new() -> Self {
        FilterExecutor
    }
}

impl Default for FilterExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskExecutor for FilterExecutor {
    fn name(&self) -> &str {
        "filter"
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
            .ok_or("FilterExecutor: 'source' field is required")?;

        // condition: フィルタリング条件式（必須）
        let condition = ctx
            .args
            .get("condition")
            .and_then(|v| v.as_str())
            .ok_or("FilterExecutor: 'condition' field is required and must be a string")?;

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
                .map_err(|e| format!("FilterExecutor: Failed to resolve source '{}': {}", path, e))?;
            resolved
                .as_array()
                .ok_or_else(|| {
                    format!(
                        "FilterExecutor: source must resolve to an array, got: {:?}",
                        resolved
                    )
                })?
                .clone()
        } else {
            return Err(format!(
                "FilterExecutor: source must be an array or path string, got: {:?}",
                source_value
            ));
        };

        let original_length = array.len();

        if original_length == 0 {
            // 空配列の場合は空の結果を返す
            return Ok(ExecutionResult {
                task_id: task.task_id.clone(),
                status: ExecutionStatus::Success,
                output: json!({
                    "items": [],
                    "original_length": 0,
                    "filtered_length": 0,
                    "indices": []
                }),
            });
        }

        // === 各要素のフィルタリング ===

        let mut filtered_items = Vec::new();
        let mut filtered_indices = Vec::new();

        for (index, item) in array.iter().enumerate() {
            // MapContextを作成
            let map_ctx = MapContext::new(item.clone(), index, original_length);

            // 条件評価用のコンテキスト
            let eval_ctx = ResolveContext {
                previous_results: ctx.previous_results.as_ref().unwrap_or(&empty_results),
                current_task: Some(task),
                loop_context: None,
                inputs: None,
                map_context: Some(&map_ctx),
                reduce_context: None,
            };

            // 条件式を評価
            let should_include = evaluate_condition(condition, &eval_ctx).map_err(|e| {
                format!(
                    "FilterExecutor: Failed to evaluate condition at index {}: {}",
                    index, e
                )
            })?;

            if should_include {
                filtered_items.push(item.clone());
                filtered_indices.push(index);
            }
        }

        let filtered_length = filtered_items.len();

        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            status: ExecutionStatus::Success,
            output: json!({
                "items": filtered_items,
                "original_length": original_length,
                "filtered_length": filtered_length,
                "indices": filtered_indices
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_filter_executor_name() {
        let executor = FilterExecutor::new();
        assert_eq!(executor.name(), "filter");
    }

    #[test]
    fn test_filter_executor_default() {
        let executor = FilterExecutor::default();
        assert_eq!(executor.name(), "filter");
    }

    #[tokio::test]
    async fn test_filter_executor_missing_source() {
        let executor = FilterExecutor::new();

        let task = Task {
            task_id: "test".to_string(),
            executor: "filter".to_string(),
            ..Default::default()
        };

        let ctx = ExecutionContext {
            args: json!({
                "condition": "$.@item.active == true"
            }),
            env_vars: HashMap::new(),
            previous_results: None,
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("source"));
    }

    #[tokio::test]
    async fn test_filter_executor_missing_condition() {
        let executor = FilterExecutor::new();

        let task = Task {
            task_id: "test".to_string(),
            executor: "filter".to_string(),
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
                "source": "$.source_task.output.items"
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("condition"));
    }

    #[tokio::test]
    async fn test_filter_executor_empty_array() {
        let executor = FilterExecutor::new();

        let task = Task {
            task_id: "test".to_string(),
            executor: "filter".to_string(),
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
                "condition": "$.@item > 0"
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap().output;
        assert_eq!(output["filtered_length"], 0);
        assert_eq!(output["original_length"], 0);
    }

    #[tokio::test]
    async fn test_filter_executor_filter_by_value() {
        let executor = FilterExecutor::new();

        let task = Task {
            task_id: "test".to_string(),
            executor: "filter".to_string(),
            ..Default::default()
        };

        let mut previous = HashMap::new();
        previous.insert(
            "source_task".to_string(),
            ExecutionResult {
                task_id: "source_task".to_string(),
                status: ExecutionStatus::Success,
                output: json!({"items": [1, 5, 2, 8, 3, 9, 4]}),
            },
        );

        let ctx = ExecutionContext {
            args: json!({
                "source": "$.source_task.output.items",
                "condition": "$.@item > 4"
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap().output;
        assert_eq!(output["items"], json!([5, 8, 9]));
        assert_eq!(output["original_length"], 7);
        assert_eq!(output["filtered_length"], 3);
        assert_eq!(output["indices"], json!([1, 3, 5]));
    }

    #[tokio::test]
    async fn test_filter_executor_filter_by_object_field() {
        let executor = FilterExecutor::new();

        let task = Task {
            task_id: "test".to_string(),
            executor: "filter".to_string(),
            ..Default::default()
        };

        let mut previous = HashMap::new();
        previous.insert(
            "source_task".to_string(),
            ExecutionResult {
                task_id: "source_task".to_string(),
                status: ExecutionStatus::Success,
                output: json!({
                    "users": [
                        {"name": "Alice", "active": true},
                        {"name": "Bob", "active": false},
                        {"name": "Charlie", "active": true}
                    ]
                }),
            },
        );

        let ctx = ExecutionContext {
            args: json!({
                "source": "$.source_task.output.users",
                "condition": "$.@item.active == true"
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap().output;
        assert_eq!(
            output["items"],
            json!([
                {"name": "Alice", "active": true},
                {"name": "Charlie", "active": true}
            ])
        );
        assert_eq!(output["filtered_length"], 2);
        assert_eq!(output["indices"], json!([0, 2]));
    }

    #[tokio::test]
    async fn test_filter_executor_filter_by_index() {
        let executor = FilterExecutor::new();

        let task = Task {
            task_id: "test".to_string(),
            executor: "filter".to_string(),
            ..Default::default()
        };

        let mut previous = HashMap::new();
        previous.insert(
            "source_task".to_string(),
            ExecutionResult {
                task_id: "source_task".to_string(),
                status: ExecutionStatus::Success,
                output: json!({"items": ["a", "b", "c", "d", "e"]}),
            },
        );

        let ctx = ExecutionContext {
            args: json!({
                "source": "$.source_task.output.items",
                "condition": "$.@index < 3"
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap().output;
        assert_eq!(output["items"], json!(["a", "b", "c"]));
        assert_eq!(output["filtered_length"], 3);
    }

    #[tokio::test]
    async fn test_filter_executor_exclude_first_last() {
        let executor = FilterExecutor::new();

        let task = Task {
            task_id: "test".to_string(),
            executor: "filter".to_string(),
            ..Default::default()
        };

        let mut previous = HashMap::new();
        previous.insert(
            "source_task".to_string(),
            ExecutionResult {
                task_id: "source_task".to_string(),
                status: ExecutionStatus::Success,
                output: json!({"items": [1, 2, 3, 4, 5]}),
            },
        );

        let ctx = ExecutionContext {
            args: json!({
                "source": "$.source_task.output.items",
                "condition": "$.@first == false && $.@last == false"
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap().output;
        assert_eq!(output["items"], json!([2, 3, 4]));
        assert_eq!(output["indices"], json!([1, 2, 3]));
    }

    #[tokio::test]
    async fn test_filter_executor_no_matches() {
        let executor = FilterExecutor::new();

        let task = Task {
            task_id: "test".to_string(),
            executor: "filter".to_string(),
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
                "condition": "$.@item > 100"
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap().output;
        assert_eq!(output["items"], json!([]));
        assert_eq!(output["original_length"], 3);
        assert_eq!(output["filtered_length"], 0);
    }

    #[tokio::test]
    async fn test_filter_executor_all_matches() {
        let executor = FilterExecutor::new();

        let task = Task {
            task_id: "test".to_string(),
            executor: "filter".to_string(),
            ..Default::default()
        };

        let mut previous = HashMap::new();
        previous.insert(
            "source_task".to_string(),
            ExecutionResult {
                task_id: "source_task".to_string(),
                status: ExecutionStatus::Success,
                output: json!({"items": [10, 20, 30]}),
            },
        );

        let ctx = ExecutionContext {
            args: json!({
                "source": "$.source_task.output.items",
                "condition": "$.@item > 0"
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap().output;
        assert_eq!(output["items"], json!([10, 20, 30]));
        assert_eq!(output["filtered_length"], 3);
        assert_eq!(output["indices"], json!([0, 1, 2]));
    }
}
