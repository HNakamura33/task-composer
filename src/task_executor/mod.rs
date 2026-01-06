//! タスク実行モジュール
//!
//! タスクを実行するためのExecutorパターンを提供します。
//! 各Executorは`TaskExecutor`トレイトを実装し、`ExecutorRegistry`に登録して使用します。
//!
//! # Example
//! ```ignore
//! use task_composer::task_executor::{ExecutorRegistry, LogExecutor};
//!
//! let mut registry = ExecutorRegistry::new();
//! registry.register(Box::new(LogExecutor::new()));
//! ```

pub mod log_executor;

use std::collections::HashMap;
use crate::types::Task;
use async_trait::async_trait;

pub use log_executor::LogExecutor;

/// タスク実行の結果
///
/// Executorがタスクを実行した結果を格納します。
/// 成功/失敗のステータスと、出力データを含みます。
///
/// # Fields
/// - `task_id`: 実行されたタスクのID
/// - `success`: 実行が成功したかどうか
/// - `output`: 実行結果のJSON出力（次のタスクの`inputs`から参照可能）
#[derive(Clone, Debug)]
pub struct ExecutionResult {
    /// 実行されたタスクのID
    pub task_id: String,
    /// 実行が成功したかどうか
    pub success: bool,
    /// 実行結果の出力データ（JSON形式）
    ///
    /// 他のタスクから`$.{task_id}.output.{field}`の形式で参照できます。
    pub output: serde_json::Value,
}

/// タスク実行時のコンテキスト
///
/// Executorがタスクを実行する際に必要な情報を提供します。
#[derive(Debug)]
pub struct ExecutionContext {
    /// タスクの引数（`args`と`inputs`から解決された値がマージされています）
    pub args: serde_json::Value,
    /// 環境変数
    pub env_vars: HashMap<String, String>,
}

/// タスク実行時のエラー
#[derive(Debug)]
pub enum ExecutionError {
    /// 指定されたタスクが見つからない
    TaskNotFound(String),
    /// タスクの実行に失敗
    ExecutionFailed(String),
    /// 入力が無効
    InvalidInput(String),
    /// その他のエラー
    Other(String),
}

/// タスク実行トレイト
///
/// 各種Executorが実装するトレイトです。
/// `Send + Sync`を要求するため、複数のタスクから並列で呼び出せます。
///
/// # Example
/// ```ignore
/// use async_trait::async_trait;
/// use task_composer::task_executor::{TaskExecutor, ExecutionContext, ExecutionResult};
/// use task_composer::types::Task;
///
/// struct MyExecutor;
///
/// #[async_trait]
/// impl TaskExecutor for MyExecutor {
///     fn name(&self) -> &str {
///         "my_executor"
///     }
///
///     async fn execute_task(&self, task: &Task, ctx: &ExecutionContext) -> Result<ExecutionResult, String> {
///         // タスクを実行...
///         Ok(ExecutionResult {
///             task_id: task.task_id.clone(),
///             success: true,
///             output: serde_json::json!({"result": "done"}),
///         })
///     }
/// }
/// ```
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// Executorの名前を返す
    ///
    /// この名前はタスクの`executor`フィールドと照合されます。
    fn name(&self) -> &str;

    /// タスクを実行する
    ///
    /// # Arguments
    /// * `task` - 実行するタスク
    /// * `ctx` - 実行コンテキスト（引数、環境変数など）
    ///
    /// # Returns
    /// 実行結果、またはエラーメッセージ
    async fn execute_task(
        &self,
        task: &Task,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String>;
}

/// Executorのレジストリ
///
/// 名前でExecutorを登録・取得するためのコンテナです。
/// DAGが使用するExecutorをここに登録します。
pub struct ExecutorRegistry {
    executors: HashMap<String, Box<dyn TaskExecutor + Send + Sync>>,
}

impl ExecutorRegistry {
    /// 新しい空のレジストリを作成
    pub fn new() -> Self {
        ExecutorRegistry {
            executors: HashMap::new(),
        }
    }

    /// Executorを登録する
    ///
    /// # Arguments
    /// * `executor` - 登録するExecutor
    ///
    /// 同じ名前のExecutorが既に登録されている場合は上書きされます。
    pub fn register(&mut self, executor: Box<dyn TaskExecutor>) {
        self.executors.insert(executor.name().to_string(), executor);
    }

    /// 名前でExecutorを取得する
    ///
    /// # Arguments
    /// * `name` - Executorの名前
    ///
    /// # Returns
    /// Executorへの参照、または`None`
    pub fn get(&self, name: &str) -> Option<&Box<dyn TaskExecutor + Send + Sync>> {
        self.executors.get(name)
    }
}

impl Default for ExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// タスクマネージャー（非推奨）
///
/// 注意: 現在は`DAG::execute_async()`を直接使用することを推奨します。
pub struct TaskManager {
    /// タスクキュー
    pub queue: Vec<Task>,
    /// Executorレジストリ
    pub registry: ExecutorRegistry,
}

impl TaskManager {
    /// 新しいタスクマネージャーを作成
    pub fn new(registry: ExecutorRegistry) -> Self {
        TaskManager {
            queue: Vec::new(),
            registry,
        }
    }

    /// タスクを追加して実行
    pub async fn add_task(
        &mut self,
        task: Task,
        ctx: ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        self.registry
            .get(&task.executor)
            .ok_or_else(|| format!("Executor not found: {}", task.executor))?
            .execute_task(&task, &ctx)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_result_clone() {
        let result = ExecutionResult {
            task_id: "test".to_string(),
            success: true,
            output: serde_json::json!({"key": "value"}),
        };
        let cloned = result.clone();
        assert_eq!(cloned.task_id, "test");
        assert!(cloned.success);
        assert_eq!(cloned.output["key"], "value");
    }

    #[test]
    fn test_executor_registry_new() {
        let registry = ExecutorRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_executor_registry_default() {
        let registry = ExecutorRegistry::default();
        assert!(registry.get("test").is_none());
    }
}
