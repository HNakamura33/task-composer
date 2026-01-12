//! BashExecutor - シェルコマンドを実行するExecutor
//!
//! # 使用例
//!
//! ```json
//! {
//!   "task_id": "run_script",
//!   "executor": "bash",
//!   "args": {
//!     "command": "echo 'hello world'"
//!   }
//! }
//! ```
//!
//! # 出力形式
//!
//! ```json
//! {
//!   "exit_code": 0,
//!   "stdout": "hello world\n",
//!   "stderr": "",
//!   "success": true
//! }
//! ```

use async_trait::async_trait;
use serde_json::json;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::task_executor::{ExecutionContext, ExecutionResult, ExecutionStatus, TaskExecutor};
use crate::types::Task;

#[cfg(test)]
use crate::types::Role;

/// デフォルトのタイムアウト（秒）
const DEFAULT_TIMEOUT_SECS: u64 = 300;

/// 出力の最大サイズ（バイト）
const MAX_OUTPUT_SIZE: usize = 1024 * 1024; // 1MB

/// シェルコマンドを実行するExecutor
pub struct BashExecutor;

impl BashExecutor {
    /// 新しいBashExecutorを作成する
    pub fn new() -> Self {
        BashExecutor
    }
}

impl Default for BashExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskExecutor for BashExecutor {
    fn name(&self) -> &str {
        "bash"
    }

    async fn execute_task(
        &self,
        task: &Task,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        // argsからコマンドを取得
        let command = ctx
            .args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "args.command is required".to_string())?;

        // オプショナルな設定を取得
        let timeout_secs = ctx
            .args
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        let working_dir = ctx.args.get("cwd").and_then(|v| v.as_str());

        let shell = ctx
            .args
            .get("shell")
            .and_then(|v| v.as_str())
            .unwrap_or("sh");

        // コマンドを構築
        let mut cmd = Command::new(shell);
        cmd.arg("-c").arg(command);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // 作業ディレクトリを設定
        if let Some(cwd) = working_dir {
            cmd.current_dir(cwd);
        }

        // 環境変数を設定
        for (key, value) in &ctx.env_vars {
            cmd.env(key, value);
        }

        // タイムアウト付きで実行
        let result = timeout(Duration::from_secs(timeout_secs), cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let exit_code = output.status.code().unwrap_or(-1);
                let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();

                // 出力サイズを制限
                if stdout.len() > MAX_OUTPUT_SIZE {
                    stdout.truncate(MAX_OUTPUT_SIZE);
                    stdout.push_str("\n... (truncated)");
                }
                if stderr.len() > MAX_OUTPUT_SIZE {
                    stderr.truncate(MAX_OUTPUT_SIZE);
                    stderr.push_str("\n... (truncated)");
                }

                let success = output.status.success();

                // stdoutがJSONの場合、パースを試みる
                let parsed_stdout = serde_json::from_str::<serde_json::Value>(stdout.trim())
                    .ok();

                let output_json = json!({
                    "exit_code": exit_code,
                    "stdout": stdout.trim(),
                    "stderr": stderr.trim(),
                    "success": success,
                    "parsed": parsed_stdout,
                });

                Ok(ExecutionResult {
                    task_id: task.task_id.clone(),
                    status: if success {
                        ExecutionStatus::Success
                    } else {
                        ExecutionStatus::Failed
                    },
                    output: output_json,
                })
            }
            Ok(Err(e)) => {
                // プロセス起動エラー
                Ok(ExecutionResult {
                    task_id: task.task_id.clone(),
                    status: ExecutionStatus::Failed,
                    output: json!({
                        "exit_code": -1,
                        "stdout": "",
                        "stderr": format!("Failed to execute command: {}", e),
                        "success": false,
                        "parsed": null,
                    }),
                })
            }
            Err(_) => {
                // タイムアウト
                Ok(ExecutionResult {
                    task_id: task.task_id.clone(),
                    status: ExecutionStatus::Failed,
                    output: json!({
                        "exit_code": -1,
                        "stdout": "",
                        "stderr": format!("Command timed out after {} seconds", timeout_secs),
                        "success": false,
                        "parsed": null,
                    }),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_task(task_id: &str) -> Task {
        Task {
            task_id: task_id.to_string(),
            name: Some(task_id.to_string()),
            description: None,
            priority: 0,
            prompt: None,
            executor: "bash".to_string(),
            args: json!({}),
            dependencies: vec![],
            role: Role::default_full_permission(),
            if_condition: None,
            else_condition: None,
            timeout_secs: None,
        }
    }

    fn create_context(args: serde_json::Value) -> ExecutionContext {
        ExecutionContext {
            args,
            env_vars: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_simple_echo() {
        let executor = BashExecutor::new();
        let task = create_test_task("test");
        let ctx = create_context(json!({
            "command": "echo 'hello world'"
        }));

        let result = executor.execute_task(&task, &ctx).await.unwrap();

        assert_eq!(result.status, ExecutionStatus::Success);
        assert_eq!(result.output["stdout"], "hello world");
        assert_eq!(result.output["exit_code"], 0);
        assert!(result.output["success"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_json_output_parsing() {
        let executor = BashExecutor::new();
        let task = create_test_task("test");
        let ctx = create_context(json!({
            "command": r#"echo '{"completed": true, "count": 42}'"#
        }));

        let result = executor.execute_task(&task, &ctx).await.unwrap();

        assert_eq!(result.status, ExecutionStatus::Success);
        assert!(result.output["parsed"]["completed"].as_bool().unwrap());
        assert_eq!(result.output["parsed"]["count"], 42);
    }

    #[tokio::test]
    async fn test_command_failure() {
        let executor = BashExecutor::new();
        let task = create_test_task("test");
        let ctx = create_context(json!({
            "command": "exit 1"
        }));

        let result = executor.execute_task(&task, &ctx).await.unwrap();

        assert_eq!(result.status, ExecutionStatus::Failed);
        assert_eq!(result.output["exit_code"], 1);
        assert!(!result.output["success"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_missing_command() {
        let executor = BashExecutor::new();
        let task = create_test_task("test");
        let ctx = create_context(json!({}));

        let result = executor.execute_task(&task, &ctx).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("args.command is required"));
    }

    #[tokio::test]
    async fn test_timeout() {
        let executor = BashExecutor::new();
        let task = create_test_task("test");
        let ctx = create_context(json!({
            "command": "sleep 10",
            "timeout_secs": 1
        }));

        let result = executor.execute_task(&task, &ctx).await.unwrap();

        assert_eq!(result.status, ExecutionStatus::Failed);
        assert!(result.output["stderr"]
            .as_str()
            .unwrap()
            .contains("timed out"));
    }

    #[tokio::test]
    async fn test_with_working_directory() {
        let executor = BashExecutor::new();
        let task = create_test_task("test");
        let ctx = create_context(json!({
            "command": "pwd",
            "cwd": "/tmp"
        }));

        let result = executor.execute_task(&task, &ctx).await.unwrap();

        assert_eq!(result.status, ExecutionStatus::Success);
        // /tmpまたは/private/tmp（macOS）
        assert!(result.output["stdout"].as_str().unwrap().contains("tmp"));
    }

    #[tokio::test]
    async fn test_pipe_command() {
        let executor = BashExecutor::new();
        let task = create_test_task("test");
        let ctx = create_context(json!({
            "command": "echo 'hello\nworld' | grep 'world'"
        }));

        let result = executor.execute_task(&task, &ctx).await.unwrap();

        assert_eq!(result.status, ExecutionStatus::Success);
        assert_eq!(result.output["stdout"], "world");
    }

    #[tokio::test]
    async fn test_grep_pattern_matching() {
        let executor = BashExecutor::new();
        let task = create_test_task("test");
        let ctx = create_context(json!({
            "command": r#"echo '{"all_tests_passed": true}' | grep -q 'all_tests_passed.*true' && echo '{"completed": true}' || echo '{"completed": false}'"#
        }));

        let result = executor.execute_task(&task, &ctx).await.unwrap();

        assert_eq!(result.status, ExecutionStatus::Success);
        assert!(result.output["parsed"]["completed"].as_bool().unwrap());
    }
}
