//! McpExecutor - MCP (Model Context Protocol) サーバーと通信するExecutor
//!
//! MCP サーバーにstdioトランスポートで接続し、ツールを呼び出します。
//! 参考: https://github.com/HNakamura33/claude-code-expriment

use super::{ExecutionContext, ExecutionResult, ExecutionStatus, TaskExecutor};
use crate::types::Task;
use async_trait::async_trait;
use rmcp::{
    model::{CallToolRequestParam, ClientInfo},
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tokio::process::Command;

/// MCP接続設定
///
/// MCPサーバーへの接続方法を定義します。
/// 現在はstdio（子プロセス）接続のみサポートしています。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ConnectionConfig {
    /// stdio接続（子プロセス経由）
    #[serde(rename = "stdio")]
    Stdio {
        /// 実行するコマンド
        command: String,
        /// コマンド引数
        #[serde(default)]
        args: Vec<String>,
        /// 環境変数
        #[serde(default)]
        env: HashMap<String, String>,
    },
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        ConnectionConfig::Stdio {
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
        }
    }
}

/// MCP Executor
///
/// MCP (Model Context Protocol) サーバーに接続してツールを実行するExecutorです。
///
/// # タスクのargs形式
///
/// タスクの`args`には以下のフィールドを指定します：
/// ```json
/// {
///   "connection": {
///     "type": "stdio",
///     "command": "uv",
///     "args": ["run", "--directory", "/path/to/server", "main.py", "serve"],
///     "env": {}
///   },
///   "tool": "tool_name",
///   "arguments": { ... }
/// }
/// ```
///
/// または、`McpExecutor::new()`で渡したデフォルト接続設定を使用し、
/// argsには`tool`と`arguments`のみを指定することもできます。
pub struct McpExecutor {
    /// デフォルトの接続設定
    default_connection: Option<ConnectionConfig>,
}

impl McpExecutor {
    /// 新しいMcpExecutorを作成
    pub fn new() -> Self {
        McpExecutor {
            default_connection: None,
        }
    }

    /// デフォルトの接続設定付きでMcpExecutorを作成
    ///
    /// この設定は、タスクのargsに`connection`が指定されていない場合に使用されます。
    pub fn with_default_connection(connection: ConnectionConfig) -> Self {
        McpExecutor {
            default_connection: Some(connection),
        }
    }

    /// MCPサーバーに接続してツールを実行
    async fn execute_mcp_tool(
        &self,
        connection: &ConnectionConfig,
        tool_name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<serde_json::Value, String> {
        match connection {
            ConnectionConfig::Stdio { command, args, env } => {
                self.execute_stdio(command, args, env, tool_name, arguments)
                    .await
            }
        }
    }

    /// stdio経由でMCPサーバーに接続してツールを実行
    async fn execute_stdio(
        &self,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        tool_name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<serde_json::Value, String> {
        let args_clone = args.to_vec();
        let env_clone = env.clone();

        // トランスポートを作成
        let transport = TokioChildProcess::new(
            Command::new(command).configure(move |cmd| {
                cmd.args(&args_clone);
                for (key, value) in &env_clone {
                    cmd.env(key, value);
                }
            }),
        )
        .map_err(|e| format!("Failed to create transport: {}", e))?;

        // MCPサーバーに接続
        let client = ClientInfo::default()
            .serve(transport)
            .await
            .map_err(|e| format!("Failed to connect to MCP server: {}", e))?;

        tracing::info!("Connected to MCP server");

        // 利用可能なツールを取得（デバッグ用）
        match client.list_tools(None).await {
            Ok(tools) => {
                let tool_names: Vec<String> =
                    tools.tools.iter().map(|t| t.name.to_string()).collect();
                tracing::debug!("Available tools: {:?}", tool_names);
            }
            Err(e) => {
                tracing::warn!("Failed to list tools: {}", e);
            }
        }

        // ツールを呼び出す
        let result = client
            .call_tool(CallToolRequestParam {
                name: tool_name.to_string().into(),
                arguments,
            })
            .await
            .map_err(|e| format!("Failed to call tool '{}': {}", tool_name, e))?;

        // 結果を処理
        let mut output = json!({
            "tool": tool_name,
            "is_error": result.is_error.unwrap_or(false),
            "content": []
        });

        let contents: Vec<serde_json::Value> = result
            .content
            .iter()
            .map(|c| match &c.raw {
                rmcp::model::RawContent::Text(text) => {
                    json!({
                        "type": "text",
                        "text": text.text
                    })
                }
                _ => {
                    json!({
                        "type": "other",
                        "raw": format!("{:?}", c.raw)
                    })
                }
            })
            .collect();

        output["content"] = json!(contents);

        // 接続を終了
        client
            .cancel()
            .await
            .map_err(|e| format!("Failed to disconnect: {}", e))?;

        Ok(output)
    }
}

impl Default for McpExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskExecutor for McpExecutor {
    fn name(&self) -> &str {
        "mcp"
    }

    async fn execute_task(
        &self,
        task: &Task,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        println!("Executing MCP Task: {}", task.display_name());
        println!("  ID: {}", task.task_id);

        // 接続設定を取得（argsから、またはデフォルト設定）
        let connection: ConnectionConfig = if let Some(conn) = ctx.args.get("connection") {
            serde_json::from_value(conn.clone())
                .map_err(|e| format!("Invalid connection config: {}", e))?
        } else if let Some(ref default_conn) = self.default_connection {
            default_conn.clone()
        } else {
            return Err("No connection config provided and no default configured".to_string());
        };

        // ツール名を取得
        let tool_name = ctx
            .args
            .get("tool")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'tool' field in args".to_string())?;

        // ツールの引数を取得
        let arguments = ctx
            .args
            .get("arguments")
            .and_then(|v| v.as_object())
            .cloned();

        println!("  Tool: {}", tool_name);
        if let Some(ref args) = arguments {
            println!("  Arguments: {}", serde_json::to_string_pretty(args).unwrap_or_default());
        }

        // MCPツールを実行
        let output = self
            .execute_mcp_tool(&connection, tool_name, arguments)
            .await?;

        // 実行結果を出力
        println!("\n  --- Result ---");
        if let Some(contents) = output.get("content").and_then(|c| c.as_array()) {
            for content in contents {
                if let Some(text) = content.get("text").and_then(|t| t.as_str()) {
                    // 各行にインデントを追加
                    for line in text.lines() {
                        println!("  {}", line);
                    }
                }
            }
        }
        println!("  --- End ---\n");

        println!("  [MCP Task {} completed]", task.task_id);

        let is_error = output
            .get("is_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            status: if is_error { ExecutionStatus::Failed } else { ExecutionStatus::Success },
            output,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_config_default() {
        let config = ConnectionConfig::default();
        match config {
            ConnectionConfig::Stdio { command, args, env } => {
                assert!(command.is_empty());
                assert!(args.is_empty());
                assert!(env.is_empty());
            }
        }
    }

    #[test]
    fn test_connection_config_deserialize() {
        let json = r#"{
            "type": "stdio",
            "command": "python",
            "args": ["-m", "server"],
            "env": {"KEY": "value"}
        }"#;

        let config: ConnectionConfig = serde_json::from_str(json).unwrap();
        match config {
            ConnectionConfig::Stdio { command, args, env } => {
                assert_eq!(command, "python");
                assert_eq!(args, vec!["-m", "server"]);
                assert_eq!(env.get("KEY"), Some(&"value".to_string()));
            }
        }
    }

    #[test]
    fn test_mcp_executor_new() {
        let executor = McpExecutor::new();
        assert!(executor.default_connection.is_none());
    }

    #[test]
    fn test_mcp_executor_with_default_connection() {
        let connection = ConnectionConfig::Stdio {
            command: "python".to_string(),
            args: vec!["-m".to_string(), "server".to_string()],
            env: HashMap::new(),
        };
        let executor = McpExecutor::with_default_connection(connection);
        assert!(executor.default_connection.is_some());
    }

    #[test]
    fn test_mcp_executor_name() {
        let executor = McpExecutor::new();
        assert_eq!(executor.name(), "mcp");
    }
}
