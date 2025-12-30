//! 型定義モジュール
//!
//! DAGで使用する主要な型を定義します：
//! - [`Task`] - タスク情報
//! - [`Role`] - ロール（役割）情報
//! - [`Status`] - タスク状態

use serde::Deserialize;

/// タスクを表す構造体
///
/// DAG内の各ノードに対応し、タスクの詳細情報を保持します。
#[derive(Deserialize)]
pub struct Task {
    /// タスクの一意な識別子
    pub task_id: String,
    /// タスクの名前
    pub name: String,
    /// タスクの詳細説明
    pub description: String,
    /// タスクの優先度（0-255、数値が大きいほど高優先）
    pub priority: u8,
    /// タスクの現在の状態
    pub status: Status,
    /// タスク実行時のプロンプト
    pub prompt: String,
    /// タスクを実行するロール
    pub role: Role,
    /// このタスクが依存するタスクIDのリスト
    pub dependencies: Vec<String>,
}

/// Task のデフォルト値
impl Default for Task {
    fn default() -> Self {
        Task {
            task_id: String::new(),
            name: String::from("Untitled Task"),
            description: String::new(),
            priority: 0,
            status: Status::default(),
            prompt: String::new(),
            role: Role::default(),
            dependencies: vec![],
        }
    }
}

/// ロール（役割）を表す構造体
///
/// タスクを実行するエージェントの役割と権限を定義します。
#[derive(Deserialize)]
pub struct Role {
    /// ロールの一意な識別子
    pub role_id: String,
    /// ロールの名前
    pub name: String,
    /// 利用可能なサブエージェントのリスト
    pub subagents: Vec<String>,
    /// このロールが持つスキルのリスト
    pub skills: Vec<String>,
    /// ロールの詳細説明
    pub description: String,
    /// 許可されたツールのリスト
    pub tool_permissions: Vec<String>,
    /// 許可されたファイル操作のリスト
    pub file_permissions: Vec<String>,
}

/// Role のデフォルト値
impl Default for Role {
    fn default() -> Self {
        Role {
            role_id: String::new(),
            name: String::from("Default Role"),
            subagents: vec![],
            skills: vec![],
            description: String::new(),
            tool_permissions: vec![],
            file_permissions: vec![],
        }
    }
}

/// タスクの状態を表すenum
///
/// タスクのライフサイクルにおける現在の状態を示します。
#[derive(Deserialize)]
pub enum Status {
    /// 未着手: タスクがまだ開始されていない
    Pending,
    /// 進行中: タスクが現在実行中
    InProgress,
    /// 完了: タスクが正常に完了した
    Completed,
}

/// Status のデフォルト値
impl Default for Status {
    fn default() -> Self {
        Status::Pending
    }
}