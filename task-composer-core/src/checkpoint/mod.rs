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
    /// サブグラフのループイテレーション履歴（完了済み）
    ///
    /// DagExecutorでループ実行されたサブグラフの全イテレーション結果を保持します。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_iterations: Option<Vec<IterationCheckpoint>>,
    /// サブグラフの進行中イテレーション
    ///
    /// イテレーション途中で中断された場合、ここに進行中の状態が保存されます。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_iteration: Option<InProgressIteration>,
}

/// 進行中のイテレーション状態
///
/// イテレーション途中で中断された場合の再開用データです。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InProgressIteration {
    /// イテレーション番号（0始まり）
    pub iteration: usize,
    /// このイテレーションで完了したタスク（再帰的にネスト可能）
    pub tasks: HashMap<String, TaskCheckpoint>,
}

/// ループ実行の状態
///
/// 全イテレーションの履歴を保持します。
/// メモリ上の`LoopContext`は直前1回分のみですが、
/// チェックポイントファイルには全履歴が保存されます。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopCheckpointState {
    /// 完了済みイテレーション数
    pub current_iteration: usize,
    /// 全イテレーションの結果（index = イテレーション番号）
    pub iterations: Vec<IterationCheckpoint>,
}

/// 1イテレーション分のチェックポイントデータ
///
/// ループの各イテレーションで実行された全タスクの結果を保持します。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationCheckpoint {
    /// このイテレーションで実行された各タスクの結果
    pub tasks: HashMap<String, TaskCheckpoint>,
    /// イテレーション完了日時
    pub completed_at: DateTime<Utc>,
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
        // 既存のタスクからloop_iterationsとcurrent_iterationを保持
        let (loop_iterations, current_iteration) = self.tasks.get(task_id)
            .map(|tc| (tc.loop_iterations.clone(), tc.current_iteration.clone()))
            .unwrap_or((None, None));

        self.tasks.insert(
            task_id.to_string(),
            TaskCheckpoint {
                status: result.status.clone(),
                output: result.output.clone(),
                completed_at: Utc::now(),
                loop_iterations,
                current_iteration,
            },
        );
        self.updated_at = Utc::now();
    }

    /// ループイテレーションの結果を追加（トップレベルループ用）
    ///
    /// 完了したイテレーションの全タスク結果をloop_stateに保存します。
    pub fn add_loop_iteration(&mut self, results: &HashMap<String, ExecutionResult>) {
        let iter_tasks: HashMap<String, TaskCheckpoint> = results
            .iter()
            .map(|(k, v)| {
                // 既存のタスクからサブグラフ情報を引き継ぐ
                let (loop_iters, curr_iter) = self.tasks.get(k)
                    .map(|tc| (tc.loop_iterations.clone(), tc.current_iteration.clone()))
                    .unwrap_or((None, None));
                (
                    k.clone(),
                    TaskCheckpoint {
                        status: v.status.clone(),
                        output: v.output.clone(),
                        completed_at: Utc::now(),
                        loop_iterations: loop_iters,
                        current_iteration: curr_iter,
                    },
                )
            })
            .collect();

        let iteration = IterationCheckpoint {
            tasks: iter_tasks,
            completed_at: Utc::now(),
        };

        match &mut self.loop_state {
            Some(state) => {
                state.iterations.push(iteration);
                state.current_iteration = state.iterations.len();
            }
            None => {
                self.loop_state = Some(LoopCheckpointState {
                    current_iteration: 1,
                    iterations: vec![iteration],
                });
            }
        }
        self.updated_at = Utc::now();
    }

    /// サブグラフのイテレーション開始を記録
    ///
    /// サブグラフの新しいイテレーションを開始する際に呼ばれます。
    pub fn start_subgraph_iteration(&mut self, task_id: &str, iteration: usize) {
        let task = self.tasks.entry(task_id.to_string()).or_insert_with(|| {
            TaskCheckpoint {
                status: ExecutionStatus::Success, // 仮のステータス（後で更新）
                output: serde_json::Value::Null,
                completed_at: Utc::now(),
                loop_iterations: None,
                current_iteration: None,
            }
        });

        task.current_iteration = Some(InProgressIteration {
            iteration,
            tasks: HashMap::new(),
        });
        self.updated_at = Utc::now();
    }

    /// サブグラフ内のタスク結果を保存
    ///
    /// サブグラフ実行中に各タスクが完了した際に呼ばれます。
    pub fn update_subgraph_task(
        &mut self,
        subgraph_task_id: &str,
        inner_task_id: &str,
        result: &ExecutionResult,
    ) {
        if let Some(task) = self.tasks.get_mut(subgraph_task_id) {
            if let Some(ref mut current) = task.current_iteration {
                current.tasks.insert(
                    inner_task_id.to_string(),
                    TaskCheckpoint {
                        status: result.status.clone(),
                        output: result.output.clone(),
                        completed_at: Utc::now(),
                        loop_iterations: None,
                        current_iteration: None,
                    },
                );
                self.updated_at = Utc::now();
            }
        }
    }

    /// サブグラフのイテレーション完了を記録
    ///
    /// current_iterationをloop_iterationsに移動します。
    pub fn complete_subgraph_iteration(
        &mut self,
        task_id: &str,
        results: &HashMap<String, ExecutionResult>,
    ) {
        let task = self.tasks.entry(task_id.to_string()).or_insert_with(|| {
            TaskCheckpoint {
                status: ExecutionStatus::Success,
                output: serde_json::Value::Null,
                completed_at: Utc::now(),
                loop_iterations: None,
                current_iteration: None,
            }
        });

        // current_iterationのtasksを使用、なければresultsから作成
        let iter_tasks: HashMap<String, TaskCheckpoint> = if let Some(ref current) = task.current_iteration {
            // current_iterationにあるものを使用し、resultsで補完
            let mut tasks = current.tasks.clone();
            for (k, v) in results {
                tasks.entry(k.clone()).or_insert_with(|| TaskCheckpoint {
                    status: v.status.clone(),
                    output: v.output.clone(),
                    completed_at: Utc::now(),
                    loop_iterations: None,
                    current_iteration: None,
                });
            }
            tasks
        } else {
            results.iter().map(|(k, v)| {
                (
                    k.clone(),
                    TaskCheckpoint {
                        status: v.status.clone(),
                        output: v.output.clone(),
                        completed_at: Utc::now(),
                        loop_iterations: None,
                        current_iteration: None,
                    },
                )
            }).collect()
        };

        let iteration = IterationCheckpoint {
            tasks: iter_tasks,
            completed_at: Utc::now(),
        };

        // loop_iterationsに追加
        task.loop_iterations
            .get_or_insert_with(Vec::new)
            .push(iteration);

        // current_iterationをクリア
        task.current_iteration = None;
        self.updated_at = Utc::now();
    }

    /// 完了済みループイテレーション数を取得（トップレベル）
    pub fn completed_loop_iterations(&self) -> usize {
        self.loop_state
            .as_ref()
            .map(|s| s.iterations.len())
            .unwrap_or(0)
    }

    /// 完了済みサブグラフイテレーション数を取得
    pub fn completed_subgraph_iterations(&self, task_id: &str) -> usize {
        self.tasks.get(task_id)
            .and_then(|tc| tc.loop_iterations.as_ref())
            .map(|iters| iters.len())
            .unwrap_or(0)
    }

    /// 進行中のサブグラフイテレーション情報を取得
    pub fn get_in_progress_iteration(&self, task_id: &str) -> Option<&InProgressIteration> {
        self.tasks.get(task_id)
            .and_then(|tc| tc.current_iteration.as_ref())
    }

    /// 最後のループイテレーションからprevious_resultsを復元（トップレベル）
    ///
    /// ループ再開時に`$.loop.previous.*`参照を解決するために使用します。
    pub fn last_loop_previous_results(&self) -> Option<HashMap<String, serde_json::Value>> {
        self.loop_state
            .as_ref()
            .and_then(|s| s.iterations.last())
            .map(|iter| {
                iter.tasks
                    .iter()
                    .filter(|(_, tc)| tc.status == ExecutionStatus::Success)
                    .map(|(k, v)| (k.clone(), v.output.clone()))
                    .collect()
            })
    }

    /// 最後のサブグラフイテレーションからprevious_resultsを復元
    pub fn last_subgraph_previous_results(
        &self,
        task_id: &str,
    ) -> Option<HashMap<String, serde_json::Value>> {
        self.tasks.get(task_id)
            .and_then(|tc| tc.loop_iterations.as_ref())
            .and_then(|iters| iters.last())
            .map(|iter| {
                iter.tasks
                    .iter()
                    .filter(|(_, tc)| tc.status == ExecutionStatus::Success)
                    .map(|(k, v)| (k.clone(), v.output.clone()))
                    .collect()
            })
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
