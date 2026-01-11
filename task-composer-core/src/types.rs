//! 型定義モジュール
//!
//! DAGで使用する主要な型を定義します：
//! - [`Task`] - タスク情報
//! - [`Role`] - ロール（役割）情報
//! - [`Status`] - タスク状態
//! - [`FilePermission`] - ファイルアクセス権限
//! - [`ToolPermission`] - ツール実行権限
//! - [`BashPermission`] - Bashコマンド権限
//! - [`WritePermission`] - ファイル書き込み権限

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// タスクを表す構造体
///
/// DAG内の各ノードに対応し、タスクの詳細情報を保持します。
///
/// # 必須フィールド
/// - `task_id`: タスクの一意な識別子
/// - `executor`: 使用するExecutorの名前
///
/// # オプショナルフィールド
/// その他のフィールドはすべてオプショナルで、デフォルト値が適用されます。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// タスクの一意な識別子（必須）
    pub task_id: String,

    /// 使用するExecutorの名前（必須）
    pub executor: String,

    /// タスクの表示名（省略時はtask_idを使用）
    #[serde(default)]
    pub name: Option<String>,

    /// タスクの詳細説明
    #[serde(default)]
    pub description: Option<String>,

    /// タスクの優先度（0-255、数値が大きいほど高優先、デフォルト: 0）
    #[serde(default)]
    pub priority: u8,

    /// タスク実行時のプロンプト
    #[serde(default)]
    pub prompt: Option<String>,

    /// タスクを実行するロール（省略時は全権限許可）
    #[serde(default = "Role::default_full_permission")]
    pub role: Role,

    /// このタスクが依存するタスクIDのリスト
    #[serde(default)]
    pub dependencies: Vec<String>,

    /// タスク実行時の引数（パス参照を含む）
    #[serde(default)]
    pub args: serde_json::Value,

    /// 実行条件（trueなら実行）
    #[serde(default, rename = "if")]
    pub if_condition: Option<String>,

    /// 実行条件（falseなら実行）= ifの否定
    #[serde(default, rename = "else")]
    pub else_condition: Option<String>,

    /// タスクのタイムアウト（秒）
    ///
    /// 指定しない場合はConfig.default_task_timeout_secsが使用されます。
    /// どちらも指定しない場合はタイムアウトなしで実行されます。
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Task のデフォルト値
impl Default for Task {
    fn default() -> Self {
        Task {
            task_id: String::new(),
            executor: String::new(),
            name: None,
            description: None,
            priority: 0,
            prompt: None,
            role: Role::default_full_permission(),
            dependencies: vec![],
            args: serde_json::Value::Null,
            if_condition: None,
            else_condition: None,
            timeout_secs: None,
        }
    }
}

impl Task {
    /// 表示用の名前を取得
    ///
    /// `name`が設定されている場合はそれを、なければ`task_id`を返します。
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.task_id)
    }
}

/// ロール（役割）を表す構造体
///
/// タスクを実行するエージェントの役割と権限を定義します。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
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
    /// ツール実行権限
    pub tool_permissions: ToolPermission,
    /// ファイルアクセス権限
    pub file_permissions: FilePermission,
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
            tool_permissions: ToolPermission::default(),
            file_permissions: FilePermission::default(),
        }
    }
}

impl Role {
    /// 全権限を持つデフォルトロールを作成
    ///
    /// Task構造体でroleが省略された場合に使用されます。
    /// すべてのツールとファイルアクセスが許可されます。
    pub fn default_full_permission() -> Self {
        Role {
            role_id: String::from("default"),
            name: String::from("Full Permission Role"),
            subagents: vec![],
            skills: vec![],
            description: String::from("Default role with full permissions"),
            tool_permissions: ToolPermission::full_permission(),
            file_permissions: FilePermission::full_permission(),
        }
    }
}

/// ファイルアクセス権限を表す構造体
///
/// ファイルシステムへのアクセス制御を定義します。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct FilePermission {
    /// 許可するパス（例: "${project_root}/src"）
    pub allowed_paths: Vec<String>,
    /// 拒否するパス（例: "${project_root}/.env"）
    pub denied_paths: Vec<String>,
    /// 読み取り専用パス（例: "${project_root}/vendor"）
    pub read_only_paths: Vec<String>,
}

/// FilePermission のデフォルト値
impl Default for FilePermission {
    fn default() -> Self {
        FilePermission {
            allowed_paths: vec![],
            denied_paths: vec![],
            read_only_paths: vec![],
        }
    }
}

impl FilePermission {
    /// 全ファイルアクセスを許可する権限を作成
    pub fn full_permission() -> Self {
        FilePermission {
            allowed_paths: vec![String::from("**/*")],
            denied_paths: vec![],
            read_only_paths: vec![],
        }
    }
}

/// Bashコマンド実行権限を表す構造体
///
/// シェルコマンドの実行制御を定義します。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BashPermission {
    /// 許可するコマンド（例: "git", "npm"）
    pub allowed_commands: Vec<String>,
    /// ブロックするコマンド（例: "rm -rf /"）
    pub blocked_commands: Vec<String>,
    /// 確認が必要なコマンド（例: "git push"）
    pub require_confirmation: Vec<String>,
}

/// BashPermission のデフォルト値
impl Default for BashPermission {
    fn default() -> Self {
        BashPermission {
            allowed_commands: vec![],
            blocked_commands: vec![],
            require_confirmation: vec![],
        }
    }
}

impl BashPermission {
    /// 全コマンド実行を許可する権限を作成
    pub fn full_permission() -> Self {
        BashPermission {
            allowed_commands: vec![String::from("*")],
            blocked_commands: vec![],
            require_confirmation: vec![],
        }
    }
}

/// ファイル書き込み権限を表す構造体
///
/// ファイル書き込み操作の制限を定義します。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct WritePermission {
    /// 最大ファイルサイズ（MB）
    pub max_file_size_mb: Option<u32>,
    /// 許可する拡張子（例: ".py", ".js"）
    pub allowed_extensions: Vec<String>,
}

/// WritePermission のデフォルト値
impl Default for WritePermission {
    fn default() -> Self {
        WritePermission {
            max_file_size_mb: None,
            allowed_extensions: vec![],
        }
    }
}

impl WritePermission {
    /// 全ファイル書き込みを許可する権限を作成
    pub fn full_permission() -> Self {
        WritePermission {
            max_file_size_mb: None, // 無制限
            allowed_extensions: vec![String::from("*")],
        }
    }
}

/// ツール実行権限を表す構造体
///
/// 各ツールの実行権限をまとめて管理します。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ToolPermission {
    /// Bashコマンド権限
    pub bash: BashPermission,
    /// ファイル書き込み権限
    pub write: WritePermission,
}

/// ToolPermission のデフォルト値
impl Default for ToolPermission {
    fn default() -> Self {
        ToolPermission {
            bash: BashPermission::default(),
            write: WritePermission::default(),
        }
    }
}

impl ToolPermission {
    /// 全ツール実行を許可する権限を作成
    pub fn full_permission() -> Self {
        ToolPermission {
            bash: BashPermission::full_permission(),
            write: WritePermission::full_permission(),
        }
    }
}


/// タスクの状態を表すenum
///
/// タスクのライフサイクルにおける現在の状態を示します。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
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

/// ファイルアクセスの競合を表す構造体
///
/// 並行実行可能な2つのタスク間で発生する
/// ファイルアクセスの競合情報を保持します。
#[derive(Deserialize)]
pub struct FileConflict {
    /// 競合する1つ目のタスクID
    pub task_a: String,
    /// 競合する2つ目のタスクID
    pub task_b: String,
    /// 競合が発生するファイルパス
    pub file_path: String,
    /// 競合の種類
    pub conflict_type: FileConflictType,
}

/// ファイル競合の種類を表すenum
///
/// 並行タスク間で発生しうるファイルアクセス競合のパターンを定義します。
#[derive(Debug, Clone, Deserialize)]
pub enum FileConflictType {
    /// 書き込み-書き込み競合: 両タスクが同じパスに書き込もうとする
    WriteWrite,
    /// 書き込み-読み取り競合: 一方が書き込み、他方が読み取りを行う
    WriteRead,
    /// 読み取り-書き込み競合: 一方が読み取り、他方が書き込みを行う
    ReadWrite,
}

/// DAG実行時の設定
///
/// タスク実行の並列度などを制御します。
///
/// # Example
/// ```
/// # use task_composer_core::Config;
/// let config = Config::default();
/// assert_eq!(config.max_concurrent_tasks, 4);
///
/// let custom_config = Config {
///     max_concurrent_tasks: 10,
///     default_task_timeout_secs: Some(300), // 5分のタイムアウト
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Config {
    /// 同時に実行できるタスクの最大数
    ///
    /// この値を超えるタスクは、実行中のタスクが完了するまでキューで待機します。
    /// デフォルト値は4です。
    pub max_concurrent_tasks: usize,

    /// タスクのデフォルトタイムアウト（秒）
    ///
    /// 各タスクでtimeout_secsが指定されていない場合に使用されます。
    /// Noneの場合はタイムアウトなしで実行されます。
    #[serde(default)]
    pub default_task_timeout_secs: Option<u64>,
}

impl Default for Config {
    /// デフォルト設定を作成
    ///
    /// - `max_concurrent_tasks`: 4
    /// - `default_task_timeout_secs`: None（タイムアウトなし）
    fn default() -> Self {
        Config {
            max_concurrent_tasks: 4,
            default_task_timeout_secs: None,
        }
    }
}

/// ループ設定
///
/// DAGを繰り返し実行するための設定を定義します。
/// DAGの非巡回性を維持しつつ、外側でループを制御します。
///
/// # Example
/// ```json
/// {
///   "loop_config": {
///     "max_iterations": 5,
///     "until_condition": "$.counter.output.value >= 10"
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct LoopConfig {
    /// 最大繰り返し回数
    pub max_iterations: usize,
    /// 継続条件（trueの間ループ継続）
    #[serde(default)]
    pub while_condition: Option<String>,
    /// 終了条件（trueになったらループ終了）
    #[serde(default)]
    pub until_condition: Option<String>,
}

impl Default for LoopConfig {
    fn default() -> Self {
        LoopConfig {
            max_iterations: 1,
            while_condition: None,
            until_condition: None,
        }
    }
}

/// ループ実行時のコンテキスト
///
/// ループ内で参照可能な情報を保持します。
/// `$.loop.iteration`, `$.loop.first`, `$.loop.previous.*` で参照できます。
///
/// # 参照パス
/// | 参照 | 意味 | 例 |
/// |------|------|---|
/// | `$.loop.iteration` | 現在のイテレーション番号（0始まり） | `0`, `1`, `2`... |
/// | `$.loop.first` | 初回かどうか | `true` / `false` |
/// | `$.loop.previous.{task_id}.output` | 前回の結果 | `$.loop.previous.counter.output.value` |
#[derive(Debug, Clone)]
pub struct LoopContext {
    /// 現在のイテレーション番号（0始まり）
    pub iteration: usize,
    /// 初回かどうか
    pub first: bool,
    /// 前回イテレーションの結果（task_id -> JSON出力）
    /// ExecutionResultの循環参照を避けるためserde_json::Valueで保持
    pub previous_results: Option<HashMap<String, serde_json::Value>>,
}