//! チェックポイントモジュール
//!
//! DAG実行の中断・再開をサポートするためのチェックポイント機能を提供します。
//!
//! # 概要
//!
//! - `Checkpoint`: 実行状態を保存するメインの構造体
//! - `CheckpointState`: 実行の状態（Running, Interrupted, Failed, Completed）
//! - `TaskCheckpoint`: 各タスクの実行結果
//!
//! # Example
//!
//! ```ignore
//! use task_composer_core::checkpoint::{Checkpoint, CheckpointState};
//!
//! let checkpoint = Checkpoint::new("dag.json", "sha256:...");
//! checkpoint.save("dag.json.checkpoint.json")?;
//! ```

pub mod writer;

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use chrono::{DateTime, Utc};

use crate::task_executor::{ExecutionResult, ExecutionStatus};

/// チェックポイントファイルのバージョン
pub const CHECKPOINT_VERSION: u32 = 1;

/// チェックポイント構造体
///
/// DAG実行の状態を保存し、中断・再開を可能にします。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// スキーマバージョン（後方互換性のため）
    pub version: u32,

    /// 元のDAGファイルパス
    pub dag_file: String,

    /// DAG JSONのハッシュ値（変更検出用）
    pub dag_hash: String,

    /// チェックポイント作成日時
    pub created_at: DateTime<Utc>,

    /// チェックポイント最終更新日時
    pub updated_at: DateTime<Utc>,

    /// 実行状態
    pub state: CheckpointState,

    /// 各タスクの実行結果
    pub tasks: HashMap<String, TaskCheckpoint>,

    /// ループ実行の状態（loop_configがある場合）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_state: Option<LoopCheckpointState>,
}

/// チェックポイントの実行状態
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum CheckpointState {
    /// 実行中
    Running,
    /// 中断された（Ctrl+C等）
    Interrupted,
    /// タスク失敗で停止
    Failed {
        /// 失敗したタスクのID
        failed_task: String,
        /// エラーメッセージ
        error: String,
    },
    /// すべてのタスクが完了
    Completed,
}

/// 各タスクのチェックポイントデータ
///
/// 完了したタスクの結果のみを保存します。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCheckpoint {
    /// タスクの実行ステータス
    pub status: ExecutionStatus,
    /// タスクの出力（Successの場合は結果、Failed/Skippedの場合はエラー/理由）
    pub output: serde_json::Value,
    /// タスク完了日時
    pub completed_at: DateTime<Utc>,
}

/// ループ実行の状態
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopCheckpointState {
    /// 現在のイテレーション（0始まり）
    pub iteration: usize,
    /// 前回イテレーションの結果（`$.loop.previous.*`参照用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_results: Option<HashMap<String, serde_json::Value>>,
}

/// チェックポイントの検証結果
#[derive(Debug, Clone, PartialEq)]
pub enum CheckpointValidation {
    /// 有効
    Valid,
    /// DAGが変更されている
    DagModified,
    /// タスクが削除されている
    TaskRemoved(String),
    /// バージョンが異なる
    VersionMismatch { expected: u32, actual: u32 },
}

impl Checkpoint {
    /// 新しいチェックポイントを作成
    ///
    /// # Arguments
    ///
    /// * `dag_file` - DAGファイルのパス
    /// * `dag_hash` - DAG JSONのハッシュ値
    pub fn new(dag_file: impl Into<String>, dag_hash: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            version: CHECKPOINT_VERSION,
            dag_file: dag_file.into(),
            dag_hash: dag_hash.into(),
            created_at: now,
            updated_at: now,
            state: CheckpointState::Running,
            tasks: HashMap::new(),
            loop_state: None,
        }
    }

    /// タスクの結果を更新
    ///
    /// # Arguments
    ///
    /// * `task_id` - タスクID
    /// * `result` - 実行結果
    pub fn update_task(&mut self, task_id: &str, result: &ExecutionResult) {
        self.tasks.insert(
            task_id.to_string(),
            TaskCheckpoint {
                status: result.status.clone(),
                output: result.output.clone(),
                completed_at: Utc::now(),
            },
        );
        self.updated_at = Utc::now();
    }

    /// 実行状態を設定
    pub fn set_state(&mut self, state: CheckpointState) {
        self.state = state;
        self.updated_at = Utc::now();
    }

    /// タスクをスキップすべきか判定
    ///
    /// 再開時に、成功済みのタスクをスキップするかどうかを判定します。
    pub fn should_skip_task(&self, task_id: &str) -> bool {
        match self.tasks.get(task_id) {
            Some(task_checkpoint) => task_checkpoint.status == ExecutionStatus::Success,
            None => false,
        }
    }

    /// チェックポイントから実行結果のHashMapを復元
    ///
    /// 成功したタスクの結果のみを含むHashMapを返します。
    /// これはパス参照の解決に使用されます。
    pub fn to_previous_results(&self) -> HashMap<String, ExecutionResult> {
        self.tasks
            .iter()
            .filter(|(_, tc)| tc.status == ExecutionStatus::Success)
            .map(|(task_id, tc)| {
                (
                    task_id.clone(),
                    ExecutionResult {
                        task_id: task_id.clone(),
                        status: tc.status.clone(),
                        output: tc.output.clone(),
                    },
                )
            })
            .collect()
    }

    /// DAGに対してチェックポイントを検証
    ///
    /// # Arguments
    ///
    /// * `dag_hash` - 現在のDAGのハッシュ値
    /// * `task_ids` - DAG内のタスクIDのリスト
    pub fn validate(&self, dag_hash: &str, task_ids: &[&str]) -> CheckpointValidation {
        // バージョンチェック
        if self.version != CHECKPOINT_VERSION {
            return CheckpointValidation::VersionMismatch {
                expected: CHECKPOINT_VERSION,
                actual: self.version,
            };
        }

        // DAG変更チェック
        if self.dag_hash != dag_hash {
            return CheckpointValidation::DagModified;
        }

        // 削除されたタスクのチェック
        for task_id in self.tasks.keys() {
            if !task_ids.contains(&task_id.as_str()) {
                return CheckpointValidation::TaskRemoved(task_id.clone());
            }
        }

        CheckpointValidation::Valid
    }

    /// 完了済みタスク数を取得
    pub fn completed_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|tc| tc.status == ExecutionStatus::Success)
            .count()
    }

    /// 失敗タスク数を取得
    pub fn failed_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|tc| tc.status == ExecutionStatus::Failed)
            .count()
    }

    /// スキップタスク数を取得
    pub fn skipped_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|tc| tc.status == ExecutionStatus::Skipped)
            .count()
    }
}

/// DAG JSONからハッシュ値を計算
///
/// SHA-256ハッシュを計算し、hex文字列として返します。
pub fn compute_dag_hash(dag_json: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(dag_json.as_bytes());
    let result = hasher.finalize();
    format!("sha256:{:x}", result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_new() {
        let checkpoint = Checkpoint::new("test.json", "sha256:abc123");

        assert_eq!(checkpoint.version, CHECKPOINT_VERSION);
        assert_eq!(checkpoint.dag_file, "test.json");
        assert_eq!(checkpoint.dag_hash, "sha256:abc123");
        assert_eq!(checkpoint.state, CheckpointState::Running);
        assert!(checkpoint.tasks.is_empty());
    }

    #[test]
    fn test_checkpoint_update_task() {
        let mut checkpoint = Checkpoint::new("test.json", "sha256:abc123");

        let result = ExecutionResult {
            task_id: "task_1".to_string(),
            status: ExecutionStatus::Success,
            output: serde_json::json!({"message": "done"}),
        };

        checkpoint.update_task("task_1", &result);

        assert!(checkpoint.tasks.contains_key("task_1"));
        assert_eq!(checkpoint.tasks["task_1"].status, ExecutionStatus::Success);
    }

    #[test]
    fn test_should_skip_task() {
        let mut checkpoint = Checkpoint::new("test.json", "sha256:abc123");

        // 成功タスクを追加
        checkpoint.update_task(
            "task_1",
            &ExecutionResult {
                task_id: "task_1".to_string(),
                status: ExecutionStatus::Success,
                output: serde_json::Value::Null,
            },
        );

        // 失敗タスクを追加
        checkpoint.update_task(
            "task_2",
            &ExecutionResult {
                task_id: "task_2".to_string(),
                status: ExecutionStatus::Failed,
                output: serde_json::Value::Null,
            },
        );

        assert!(checkpoint.should_skip_task("task_1"));  // 成功はスキップ
        assert!(!checkpoint.should_skip_task("task_2")); // 失敗はスキップしない
        assert!(!checkpoint.should_skip_task("task_3")); // 存在しないタスクはスキップしない
    }

    #[test]
    fn test_to_previous_results() {
        let mut checkpoint = Checkpoint::new("test.json", "sha256:abc123");

        checkpoint.update_task(
            "task_1",
            &ExecutionResult {
                task_id: "task_1".to_string(),
                status: ExecutionStatus::Success,
                output: serde_json::json!({"value": 42}),
            },
        );

        checkpoint.update_task(
            "task_2",
            &ExecutionResult {
                task_id: "task_2".to_string(),
                status: ExecutionStatus::Failed,
                output: serde_json::json!({"error": "failed"}),
            },
        );

        let results = checkpoint.to_previous_results();

        // 成功タスクのみ含まれる
        assert_eq!(results.len(), 1);
        assert!(results.contains_key("task_1"));
        assert!(!results.contains_key("task_2"));
    }

    #[test]
    fn test_validate() {
        let checkpoint = Checkpoint::new("test.json", "sha256:abc123");

        // 有効
        assert_eq!(
            checkpoint.validate("sha256:abc123", &[]),
            CheckpointValidation::Valid
        );

        // DAG変更
        assert_eq!(
            checkpoint.validate("sha256:different", &[]),
            CheckpointValidation::DagModified
        );
    }

    #[test]
    fn test_compute_dag_hash() {
        let hash1 = compute_dag_hash(r#"{"tasks": []}"#);
        let hash2 = compute_dag_hash(r#"{"tasks": []}"#);
        let hash3 = compute_dag_hash(r#"{"tasks": ["a"]}"#);

        assert!(hash1.starts_with("sha256:"));
        assert_eq!(hash1, hash2); // 同じ内容は同じハッシュ
        assert_ne!(hash1, hash3); // 異なる内容は異なるハッシュ
    }

    #[test]
    fn test_checkpoint_serialization() {
        let mut checkpoint = Checkpoint::new("test.json", "sha256:abc123");
        checkpoint.set_state(CheckpointState::Failed {
            failed_task: "task_1".to_string(),
            error: "Something went wrong".to_string(),
        });

        let json = serde_json::to_string_pretty(&checkpoint).unwrap();
        let deserialized: Checkpoint = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.dag_file, "test.json");
        assert!(matches!(deserialized.state, CheckpointState::Failed { .. }));
    }
}
