//! ファイル操作フック

use dioxus::prelude::*;
use crate::state::{use_app_state, DagState};

/// ファイル操作を提供するフック
pub fn use_file_operations() -> FileOperations {
    let app_state = use_app_state();

    FileOperations {
        dag: app_state.dag,
    }
}

#[derive(Clone, Copy)]
pub struct FileOperations {
    dag: Signal<DagState>,
}

impl FileOperations {
    /// JSONファイルを読み込んでDAG状態を更新
    pub fn load_json(&mut self, json: &str) -> Result<(), String> {
        let dag_state = DagState::from_json(json)?;
        *self.dag.write() = dag_state;
        Ok(())
    }

    /// DAG状態をJSONとして取得
    pub fn save_json(&self) -> Result<String, String> {
        self.dag.read().to_json()
    }

    /// サンプルDAGを読み込む（デモ用）
    pub fn load_sample(&mut self) {
        let sample = r#"{
            "tasks": [
                {
                    "task_id": "1",
                    "name": "Initial Setup",
                    "description": "Setup project environment",
                    "priority": 1,
                    "status": "Completed",
                    "prompt": "Initialize the project",
                    "executor": "log",
                    "dependencies": [],
                    "role": { "role_id": "r1", "name": "Setup", "subagents": [], "skills": [], "description": "", "tool_permissions": { "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] }, "write": { "max_file_size_mb": null, "allowed_extensions": [] } }, "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] } },
                    "args": {},
                    "inputs": {}
                },
                {
                    "task_id": "2",
                    "name": "Build Frontend",
                    "description": "Compile frontend assets",
                    "priority": 2,
                    "status": "InProgress",
                    "prompt": "Build frontend",
                    "executor": "log",
                    "dependencies": ["1"],
                    "role": { "role_id": "r2", "name": "Build", "subagents": [], "skills": [], "description": "", "tool_permissions": { "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] }, "write": { "max_file_size_mb": null, "allowed_extensions": [] } }, "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] } },
                    "args": {},
                    "inputs": {}
                },
                {
                    "task_id": "3",
                    "name": "Build Backend",
                    "description": "Compile backend services",
                    "priority": 2,
                    "status": "Pending",
                    "prompt": "Build backend",
                    "executor": "log",
                    "dependencies": ["1"],
                    "role": { "role_id": "r3", "name": "Build", "subagents": [], "skills": [], "description": "", "tool_permissions": { "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] }, "write": { "max_file_size_mb": null, "allowed_extensions": [] } }, "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] } },
                    "args": {},
                    "inputs": {}
                },
                {
                    "task_id": "4",
                    "name": "Deploy",
                    "description": "Deploy to production",
                    "priority": 3,
                    "status": "Pending",
                    "prompt": "Deploy application",
                    "executor": "log",
                    "dependencies": ["2", "3"],
                    "role": { "role_id": "r4", "name": "Deploy", "subagents": [], "skills": [], "description": "", "tool_permissions": { "bash": { "allowed_commands": [], "blocked_commands": [], "require_confirmation": [] }, "write": { "max_file_size_mb": null, "allowed_extensions": [] } }, "file_permissions": { "allowed_paths": [], "denied_paths": [], "read_only_paths": [] } },
                    "args": {},
                    "inputs": {}
                }
            ],
            "config": {
                "max_concurrent_tasks": 4
            }
        }"#;

        if let Err(e) = self.load_json(sample) {
            eprintln!("Failed to load sample DAG: {}", e);
        }
    }
}
