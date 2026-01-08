//! 静的解析の状態管理

use std::collections::HashMap;

/// 解析結果の種類
#[derive(Debug, Clone, PartialEq)]
pub enum AnalysisLevel {
    Info,
    Warning,
    Error,
}

/// 解析結果の1項目
#[derive(Debug, Clone)]
pub struct AnalysisItem {
    pub level: AnalysisLevel,
    pub category: String,
    pub message: String,
    pub related_tasks: Vec<String>,
}

/// DAG構造の解析結果
#[derive(Debug, Clone, Default)]
pub struct DagAnalysis {
    /// トポロジカルソート結果（循環がある場合はNone）
    pub topological_order: Option<Vec<String>>,
    /// 循環が検出されたか
    pub has_cycle: bool,
    /// 並列実行可能なペア
    pub parallel_pairs: Vec<(String, String)>,
    /// 孤立ノード（依存も被依存もない）
    pub orphan_nodes: Vec<String>,
    /// ルートノード（依存がない）
    pub root_nodes: Vec<String>,
    /// リーフノード（被依存がない）
    pub leaf_nodes: Vec<String>,
    /// 各ノードの深さ（ルートからの最大距離）
    pub node_depths: HashMap<String, usize>,
    /// クリティカルパス（最長パス）
    pub critical_path: Vec<String>,
}

/// タスク定義の検証結果
#[derive(Debug, Clone, Default)]
pub struct TaskValidation {
    /// 検証項目リスト
    pub items: Vec<AnalysisItem>,
}

/// 全体の解析状態
#[derive(Debug, Clone, Default)]
pub struct AnalysisState {
    /// DAG構造解析結果
    pub dag_analysis: DagAnalysis,
    /// タスク検証結果
    pub task_validation: TaskValidation,
    /// 解析実行済みフラグ
    pub analyzed: bool,
}

impl AnalysisState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 解析結果をクリア
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// エラー数を取得
    pub fn error_count(&self) -> usize {
        self.task_validation.items.iter()
            .filter(|i| i.level == AnalysisLevel::Error)
            .count()
            + if self.dag_analysis.has_cycle { 1 } else { 0 }
    }

    /// 警告数を取得
    pub fn warning_count(&self) -> usize {
        self.task_validation.items.iter()
            .filter(|i| i.level == AnalysisLevel::Warning)
            .count()
            + self.dag_analysis.orphan_nodes.len()
    }

    /// 情報数を取得
    pub fn info_count(&self) -> usize {
        self.task_validation.items.iter()
            .filter(|i| i.level == AnalysisLevel::Info)
            .count()
    }
}
