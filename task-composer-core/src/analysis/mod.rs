//! 静的解析モジュール
//!
//! DAG構造とタスク定義の静的解析機能を提供します。
//!
//! # 機能
//! - DAG構造解析（循環検出、ルート/リーフノード、クリティカルパス等）
//! - タスク定義検証（必須フィールド、依存関係、FilePermission等）
//!
//! # 使用例
//! ```ignore
//! use task_composer_core::analysis::{StaticAnalyzer, AnalysisResult};
//!
//! let analyzer = StaticAnalyzer::new(&dag);
//! let result = analyzer.analyze();
//!
//! if result.has_errors() {
//!     for error in result.errors() {
//!         println!("Error: {}", error.message);
//!     }
//! }
//! ```

mod dag_analysis;
mod task_validation;
mod conflict;

pub use dag_analysis::*;
pub use task_validation::*;
pub use conflict::{ConflictDetector, paths_overlap};

use std::collections::HashMap;
use crate::dag::DAG;

/// 解析結果のレベル
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisLevel {
    /// 情報（問題なし）
    Info,
    /// 警告（推奨事項）
    Warning,
    /// エラー（修正が必要）
    Error,
}

/// 解析結果の1項目
#[derive(Debug, Clone)]
pub struct AnalysisItem {
    /// レベル
    pub level: AnalysisLevel,
    /// カテゴリ
    pub category: String,
    /// メッセージ
    pub message: String,
    /// 関連するタスクID
    pub related_tasks: Vec<String>,
}

/// DAG構造の解析結果
#[derive(Debug, Clone, Default)]
pub struct DagStructureAnalysis {
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

/// 全体の解析結果
#[derive(Debug, Clone, Default)]
pub struct AnalysisResult {
    /// DAG構造解析結果
    pub dag_structure: DagStructureAnalysis,
    /// 検証項目リスト
    pub items: Vec<AnalysisItem>,
}

impl AnalysisResult {
    /// エラーがあるか
    pub fn has_errors(&self) -> bool {
        self.dag_structure.has_cycle ||
        self.items.iter().any(|i| i.level == AnalysisLevel::Error)
    }

    /// エラー数を取得
    pub fn error_count(&self) -> usize {
        let cycle_error = if self.dag_structure.has_cycle { 1 } else { 0 };
        cycle_error + self.items.iter()
            .filter(|i| i.level == AnalysisLevel::Error)
            .count()
    }

    /// 警告数を取得
    pub fn warning_count(&self) -> usize {
        self.dag_structure.orphan_nodes.len() +
        self.items.iter()
            .filter(|i| i.level == AnalysisLevel::Warning)
            .count()
    }

    /// エラー項目を取得
    pub fn errors(&self) -> impl Iterator<Item = &AnalysisItem> {
        self.items.iter().filter(|i| i.level == AnalysisLevel::Error)
    }

    /// 警告項目を取得
    pub fn warnings(&self) -> impl Iterator<Item = &AnalysisItem> {
        self.items.iter().filter(|i| i.level == AnalysisLevel::Warning)
    }
}

/// 静的解析を実行する構造体
pub struct StaticAnalyzer<'a> {
    dag: &'a DAG,
}

impl<'a> StaticAnalyzer<'a> {
    /// 新しいStaticAnalyzerを作成
    pub fn new(dag: &'a DAG) -> Self {
        Self { dag }
    }

    /// 全ての静的解析を実行
    pub fn analyze(&self) -> AnalysisResult {
        let mut result = AnalysisResult::default();

        // DAG構造解析
        result.dag_structure = self.analyze_dag_structure();

        // タスク定義検証
        let validation_items = self.validate_tasks();
        result.items.extend(validation_items);

        // FilePermissionコンフリクト検出（タスク間）
        if let Ok(conflicts) = self.check_file_conflicts() {
            result.items.extend(conflicts);
        }

        result
    }

    /// DAG構造のみを解析
    pub fn analyze_dag_structure(&self) -> DagStructureAnalysis {
        analyze_dag_structure(self.dag)
    }

    /// タスク定義のみを検証
    pub fn validate_tasks(&self) -> Vec<AnalysisItem> {
        validate_all_tasks(self.dag)
    }

    /// FilePermissionコンフリクトを検出
    fn check_file_conflicts(&self) -> Result<Vec<AnalysisItem>, String> {
        // DAGをクローンしてConflictDetectorに渡す
        let dag_clone = self.clone_dag();
        let detector = ConflictDetector::new(dag_clone);
        let conflicts = detector.check_file_conflicts()?;

        let items: Vec<AnalysisItem> = conflicts.into_iter().map(|c| {
            AnalysisItem {
                level: AnalysisLevel::Warning,
                category: "FileConflict".to_string(),
                message: format!(
                    "タスク {} と {} が '{}' で {:?} 競合しています",
                    c.task_a, c.task_b, c.file_path, c.conflict_type
                ),
                related_tasks: vec![c.task_a, c.task_b],
            }
        }).collect();

        Ok(items)
    }

    /// DAGをクローン（ConflictDetector用）
    fn clone_dag(&self) -> DAG {
        let mut new_dag = DAG::new();
        for (_, task) in &self.dag.nodes {
            new_dag.add_task(task.clone());
        }
        for (from, tos) in &self.dag.edges {
            for to in tos {
                new_dag.add_edge(from, to);
            }
        }
        new_dag.config = self.dag.config.clone();
        new_dag
    }
}
