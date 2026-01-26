//! MapExecutor - 配列の各要素に対してサブDAGを実行するExecutor
//!
//! ## 概要
//! 配列の各要素に対してサブDAGを実行し、結果を集約します。
//! デフォルトで並列実行され、`sequential: true`で逐次実行に切り替えられます。
//!
//! ## args フィールド
//! - `source`: 処理対象配列へのパス参照（必須）
//! - `inputs`: サブDAGへ渡す入力のマッピング（必須）
//! - `dag`: 各要素に対して実行するサブDAG（必須）
//! - `sequential`: trueで逐次実行（デフォルト: false）
//! - `max_concurrency`: 最大同時実行数（デフォルト: 無制限）
//! - `on_error`: エラー時の動作（"stop"/"continue"/"skip"、デフォルト: "stop"）
//! - `result_format`: 出力形式（"by_element"/"by_task"、デフォルト: "by_element"）
//!
//! ## 特殊変数
//! inputsマッピング内で以下の特殊変数が使用可能:
//! - `$.@item`: 現在処理中の要素
//! - `$.@index`: 0始まりのインデックス
//! - `$.@length`: 配列の総要素数
//! - `$.@first`: 最初の要素かどうか（boolean）
//! - `$.@last`: 最後の要素かどうか（boolean）
//!
//! ## 使用例
//! ```json
//! {
//!   "task_id": "process_users",
//!   "executor": "map",
//!   "args": {
//!     "source": "$.fetch_users.output.users",
//!     "max_concurrency": 3,
//!     "on_error": "continue",
//!     "inputs": {
//!       "user": "$.@item",
//!       "index": "$.@index"
//!     },
//!     "dag": {
//!       "tasks": [
//!         {
//!           "task_id": "greet",
//!           "executor": "data",
//!           "args": { "value": "Hello, ${$.inputs.user.name}!" }
//!         }
//!       ]
//!     }
//!   }
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use tokio::sync::Semaphore;

use crate::dag::DAG;
use crate::path_resolver::{resolve_inputs, ResolveContext};
use crate::task_executor::{
    ExecutionContext, ExecutionResult, ExecutionStatus, ExecutorRegistry, TaskExecutor,
};
use crate::types::{MapContext, OnErrorMode, ResultFormat, Task};

/// MapExecutor - 配列の各要素に対してサブDAGを実行
pub struct MapExecutor {
    /// ExecutorRegistry（サブDAGで使用）
    registry: Arc<ExecutorRegistry>,
}

impl MapExecutor {
    /// 新しいMapExecutorを作成
    pub fn new(registry: Arc<ExecutorRegistry>) -> Self {
        MapExecutor { registry }
    }
}

/// 各要素の実行結果
#[derive(Debug, Clone)]
struct ElementResult {
    index: usize,
    status: ElementStatus,
}

/// 要素の実行ステータス
#[derive(Debug, Clone)]
enum ElementStatus {
    Success(HashMap<String, ExecutionResult>),
    Failed(String),
    Skipped,
}

#[async_trait]
impl TaskExecutor for MapExecutor {
    fn name(&self) -> &str {
        "map"
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
            .ok_or("MapExecutor: 'source' field is required")?;

        // inputs: サブDAGへ渡す入力のマッピング（必須）
        let inputs_template = ctx
            .args
            .get("inputs")
            .ok_or("MapExecutor: 'inputs' field is required")?
            .clone();

        // dag: サブDAG定義（必須）
        let dag_value = ctx
            .args
            .get("dag")
            .ok_or("MapExecutor: 'dag' field is required")?;

        // sequential: 逐次実行フラグ（デフォルト: false）
        let sequential = ctx
            .args
            .get("sequential")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // max_concurrency: 最大同時実行数（デフォルト: 無制限）
        let max_concurrency = ctx
            .args
            .get("max_concurrency")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        // on_error: エラー時の動作（デフォルト: stop）
        let on_error = ctx
            .args
            .get("on_error")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "continue" => OnErrorMode::Continue,
                "skip" => OnErrorMode::Skip,
                _ => OnErrorMode::Stop,
            })
            .unwrap_or_default();

        // result_format: 出力形式（デフォルト: by_element）
        let result_format = ctx
            .args
            .get("result_format")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "by_task" => ResultFormat::ByTask,
                _ => ResultFormat::ByElement,
            })
            .unwrap_or_default();

        // === source配列の取得 ===

        // パス参照を解決するためのコンテキストを作成
        // previous_resultsはExecutionContextから取得（ない場合は空）
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
                .map_err(|e| format!("MapExecutor: Failed to resolve source '{}': {}", path, e))?;
            resolved
                .as_array()
                .ok_or_else(|| {
                    format!(
                        "MapExecutor: source must resolve to an array, got: {:?}",
                        resolved
                    )
                })?
                .clone()
        } else {
            return Err(format!(
                "MapExecutor: source must be an array or path string, got: {:?}",
                source_value
            ));
        };

        let array_len = array.len();

        if array_len == 0 {
            // 空配列の場合は空の結果を返す
            return Ok(ExecutionResult {
                task_id: task.task_id.clone(),
                status: ExecutionStatus::Success,
                output: json!({
                    "results": [],
                    "length": 0,
                    "succeeded": 0,
                    "failed": 0
                }),
            });
        }

        // === 各要素の処理 ===

        let results = if sequential {
            // 逐次実行
            self.execute_sequential(
                &array,
                &inputs_template,
                dag_value,
                &on_error,
                &resolve_ctx,
            )
            .await?
        } else {
            // 並列実行
            self.execute_parallel(
                &array,
                &inputs_template,
                dag_value,
                &on_error,
                max_concurrency,
                &resolve_ctx,
            )
            .await?
        };

        // === 結果の集約 ===

        let output = self.aggregate_results(&results, result_format);

        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            status: if results.iter().any(|r| matches!(r.status, ElementStatus::Failed(_))) {
                ExecutionStatus::Failed
            } else {
                ExecutionStatus::Success
            },
            output,
        })
    }
}

impl MapExecutor {
    /// 逐次実行
    async fn execute_sequential(
        &self,
        array: &[serde_json::Value],
        inputs_template: &serde_json::Value,
        dag_value: &serde_json::Value,
        on_error: &OnErrorMode,
        parent_ctx: &ResolveContext<'_>,
    ) -> Result<Vec<ElementResult>, String> {
        let mut results = Vec::with_capacity(array.len());
        let array_len = array.len();

        for (index, item) in array.iter().enumerate() {
            let result = self
                .execute_element(index, item.clone(), array_len, inputs_template, dag_value, parent_ctx)
                .await;

            // エラーの場合の処理を先に判定
            let is_failed = matches!(&result.status, ElementStatus::Failed(_));
            let error_msg = if let ElementStatus::Failed(err) = &result.status {
                Some(err.clone())
            } else {
                None
            };

            if is_failed {
                match on_error {
                    OnErrorMode::Stop => {
                        let err = error_msg.unwrap();
                        results.push(result);
                        return Err(format!(
                            "MapExecutor: Element {} failed: {}",
                            index, err
                        ));
                    }
                    OnErrorMode::Continue => {
                        results.push(result);
                    }
                    OnErrorMode::Skip => {
                        results.push(ElementResult {
                            index,
                            status: ElementStatus::Skipped,
                        });
                    }
                }
            } else {
                results.push(result);
            }
        }

        Ok(results)
    }

    /// 並列実行
    async fn execute_parallel(
        &self,
        array: &[serde_json::Value],
        inputs_template: &serde_json::Value,
        dag_value: &serde_json::Value,
        on_error: &OnErrorMode,
        max_concurrency: Option<usize>,
        parent_ctx: &ResolveContext<'_>,
    ) -> Result<Vec<ElementResult>, String> {
        let array_len = array.len();

        // Semaphoreで同時実行数を制限
        let semaphore = max_concurrency.map(|n| Arc::new(Semaphore::new(n)));

        // previous_resultsをクローンして所有権を持たせる
        let previous_results = parent_ctx.previous_results.clone();

        // タスクを生成
        let mut handles = Vec::with_capacity(array_len);

        for (index, item) in array.iter().enumerate() {
            let item = item.clone();
            let inputs_template = inputs_template.clone();
            let dag_value = dag_value.clone();
            let registry = Arc::clone(&self.registry);
            let semaphore = semaphore.clone();
            let previous_results = previous_results.clone();

            let handle = tokio::spawn(async move {
                // Semaphoreを取得（max_concurrency制限）
                let _permit = if let Some(ref sem) = semaphore {
                    Some(sem.acquire().await.unwrap())
                } else {
                    None
                };

                // 要素を処理
                execute_element_standalone(
                    index,
                    item,
                    array_len,
                    &inputs_template,
                    &dag_value,
                    registry,
                    &previous_results,
                )
                .await
            });

            handles.push(handle);
        }

        // 結果を収集
        let mut results = Vec::with_capacity(array_len);
        let mut first_error: Option<String> = None;

        for handle in handles {
            let result = handle
                .await
                .map_err(|e| format!("MapExecutor: Task join error: {}", e))?;

            match &result.status {
                ElementStatus::Failed(err) => {
                    match on_error {
                        OnErrorMode::Stop => {
                            if first_error.is_none() {
                                first_error = Some(format!(
                                    "MapExecutor: Element {} failed: {}",
                                    result.index, err
                                ));
                            }
                            results.push(result);
                        }
                        OnErrorMode::Continue => {
                            results.push(result);
                        }
                        OnErrorMode::Skip => {
                            results.push(ElementResult {
                                index: result.index,
                                status: ElementStatus::Skipped,
                            });
                        }
                    }
                }
                _ => {
                    results.push(result);
                }
            }
        }

        // Stopモードでエラーがあった場合
        if let Some(err) = first_error {
            return Err(err);
        }

        // インデックス順にソート
        results.sort_by_key(|r| r.index);

        Ok(results)
    }

    /// 単一要素を処理（逐次実行用）
    async fn execute_element(
        &self,
        index: usize,
        item: serde_json::Value,
        array_len: usize,
        inputs_template: &serde_json::Value,
        dag_value: &serde_json::Value,
        parent_ctx: &ResolveContext<'_>,
    ) -> ElementResult {
        let result = execute_element_inner(
            index,
            item,
            array_len,
            inputs_template,
            dag_value,
            Arc::clone(&self.registry),
            parent_ctx.previous_results,
        )
        .await;

        result
    }

    /// 結果を集約
    fn aggregate_results(
        &self,
        results: &[ElementResult],
        format: ResultFormat,
    ) -> serde_json::Value {
        let succeeded = results
            .iter()
            .filter(|r| matches!(r.status, ElementStatus::Success(_)))
            .count();
        let failed = results
            .iter()
            .filter(|r| matches!(r.status, ElementStatus::Failed(_)))
            .count();
        let skipped = results
            .iter()
            .filter(|r| matches!(r.status, ElementStatus::Skipped))
            .count();

        match format {
            ResultFormat::ByElement => {
                // 要素ごとにグループ化
                let results_array: Vec<serde_json::Value> = results
                    .iter()
                    .map(|r| match &r.status {
                        ElementStatus::Success(outputs) => {
                            let output_map: serde_json::Map<String, serde_json::Value> = outputs
                                .iter()
                                .map(|(k, v)| (k.clone(), v.output.clone()))
                                .collect();
                            json!({
                                "index": r.index,
                                "status": "success",
                                "output": output_map
                            })
                        }
                        ElementStatus::Failed(err) => {
                            json!({
                                "index": r.index,
                                "status": "failed",
                                "error": err
                            })
                        }
                        ElementStatus::Skipped => {
                            json!({
                                "index": r.index,
                                "status": "skipped"
                            })
                        }
                    })
                    .collect();

                json!({
                    "results": results_array,
                    "length": results.len(),
                    "succeeded": succeeded,
                    "failed": failed,
                    "skipped": skipped
                })
            }
            ResultFormat::ByTask => {
                // タスクごとにグループ化
                let mut task_results: HashMap<String, Vec<Option<serde_json::Value>>> = HashMap::new();

                // 全タスクIDを収集
                for result in results {
                    if let ElementStatus::Success(outputs) = &result.status {
                        for task_id in outputs.keys() {
                            task_results.entry(task_id.clone()).or_insert_with(|| {
                                vec![None; results.len()]
                            });
                        }
                    }
                }

                // 結果を配置
                for result in results {
                    match &result.status {
                        ElementStatus::Success(outputs) => {
                            for (task_id, exec_result) in outputs {
                                if let Some(arr) = task_results.get_mut(task_id) {
                                    arr[result.index] = Some(json!({
                                        "output": exec_result.output
                                    }));
                                }
                            }
                        }
                        ElementStatus::Failed(_) | ElementStatus::Skipped => {
                            // 失敗/スキップの要素はnullのまま
                        }
                    }
                }

                let mut output = serde_json::Map::new();
                for (task_id, values) in task_results {
                    output.insert(task_id, json!(values));
                }
                output.insert("length".to_string(), json!(results.len()));
                output.insert("succeeded".to_string(), json!(succeeded));
                output.insert("failed".to_string(), json!(failed));
                output.insert("skipped".to_string(), json!(skipped));

                serde_json::Value::Object(output)
            }
        }
    }
}

/// 単一要素を処理（内部関数）
async fn execute_element_inner(
    index: usize,
    item: serde_json::Value,
    array_len: usize,
    inputs_template: &serde_json::Value,
    dag_value: &serde_json::Value,
    registry: Arc<ExecutorRegistry>,
    previous_results: &HashMap<String, ExecutionResult>,
) -> ElementResult {
    // MapContextを作成
    let map_ctx = MapContext::new(item, index, array_len);

    // inputsを解決するためのコンテキスト
    let resolve_ctx = ResolveContext {
        previous_results,
        current_task: None,
        loop_context: None,
        inputs: None,
        map_context: Some(&map_ctx),
        reduce_context: None,
    };

    // inputs_templateを解決
    let resolved_inputs = match resolve_inputs(inputs_template, &resolve_ctx) {
        Ok(v) => v,
        Err(e) => {
            return ElementResult {
                index,
                status: ElementStatus::Failed(format!("Failed to resolve inputs: {}", e)),
            };
        }
    };

    // サブDAGを作成
    let dag_str = match serde_json::to_string(dag_value) {
        Ok(s) => s,
        Err(e) => {
            return ElementResult {
                index,
                status: ElementStatus::Failed(format!("Failed to serialize DAG: {}", e)),
            };
        }
    };

    let mut sub_dag = match DAG::from_json(&dag_str) {
        Ok(d) => d,
        Err(e) => {
            return ElementResult {
                index,
                status: ElementStatus::Failed(format!("Failed to parse DAG: {}", e)),
            };
        }
    };

    // レジストリとinputsを設定
    sub_dag.set_registry(registry);
    sub_dag.set_inputs(resolved_inputs);

    // サブDAGを実行
    match sub_dag.execute_async().await {
        Ok(results) => ElementResult {
            index,
            status: ElementStatus::Success(results),
        },
        Err(e) => ElementResult {
            index,
            status: ElementStatus::Failed(e),
        },
    }
}

/// 単一要素を処理（並列実行用スタンドアロン関数）
async fn execute_element_standalone(
    index: usize,
    item: serde_json::Value,
    array_len: usize,
    inputs_template: &serde_json::Value,
    dag_value: &serde_json::Value,
    registry: Arc<ExecutorRegistry>,
    previous_results: &HashMap<String, ExecutionResult>,
) -> ElementResult {
    execute_element_inner(
        index,
        item,
        array_len,
        inputs_template,
        dag_value,
        registry,
        previous_results,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_map_executor_name() {
        let registry = Arc::new(ExecutorRegistry::new());
        let executor = MapExecutor::new(registry);
        assert_eq!(executor.name(), "map");
    }

    #[tokio::test]
    async fn test_map_executor_missing_source() {
        let registry = Arc::new(ExecutorRegistry::new());
        let executor = MapExecutor::new(registry);

        let task = Task {
            task_id: "test".to_string(),
            executor: "map".to_string(),
            ..Default::default()
        };

        let ctx = ExecutionContext {
            args: json!({
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
    async fn test_map_executor_missing_inputs() {
        let registry = Arc::new(ExecutorRegistry::new());
        let executor = MapExecutor::new(registry);

        let task = Task {
            task_id: "test".to_string(),
            executor: "map".to_string(),
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
    async fn test_map_executor_missing_dag() {
        let registry = Arc::new(ExecutorRegistry::new());
        let executor = MapExecutor::new(registry);

        let task = Task {
            task_id: "test".to_string(),
            executor: "map".to_string(),
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
                "inputs": {"item": "$.@item"}
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dag"));
    }

    #[tokio::test]
    async fn test_map_executor_empty_array() {
        let registry = Arc::new(ExecutorRegistry::new());
        let executor = MapExecutor::new(registry);

        let task = Task {
            task_id: "test".to_string(),
            executor: "map".to_string(),
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
                "inputs": {"item": "$.@item"},
                "dag": {"tasks": []}
            }),
            env_vars: HashMap::new(),
            previous_results: Some(previous),
        };

        let result = executor.execute_task(&task, &ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap().output;
        assert_eq!(output["length"], 0);
        assert_eq!(output["succeeded"], 0);
        assert_eq!(output["failed"], 0);
    }
}
