//! 静的解析フック
//!
//! task-composer-core の解析機能を使用

use dioxus::prelude::*;
use task_composer_core::dag::DAG;
use task_composer_core::analysis::{StaticAnalyzer, AnalysisResult as CoreAnalysisResult};
use crate::state::{use_app_state, AnalysisState, DagAnalysis, TaskValidation, AnalysisItem, AnalysisLevel};

/// 解析操作を提供するフック
pub fn use_analysis() -> AnalysisOperations {
    let app_state = use_app_state();

    AnalysisOperations {
        dag: app_state.dag,
        analysis: app_state.analysis,
    }
}

#[derive(Clone, Copy)]
pub struct AnalysisOperations {
    dag: Signal<crate::state::DagState>,
    analysis: Signal<AnalysisState>,
}

impl AnalysisOperations {
    /// 静的解析を実行（coreの機能を使用）
    pub fn run_analysis(&mut self) {
        let dag_state = self.dag.read();

        // DagStateからDAGを構築
        let dag = self.build_dag(&dag_state);

        // coreのStaticAnalyzerを使用
        let analyzer = StaticAnalyzer::new(&dag);
        let core_result = analyzer.analyze();

        // coreの結果をUIの状態に変換
        let (dag_analysis, task_validation) = convert_analysis_result(&core_result);

        // 結果を更新
        let mut analysis = self.analysis.write();
        analysis.dag_analysis = dag_analysis;
        analysis.task_validation = task_validation;
        analysis.analyzed = true;
    }

    /// 解析結果をクリア
    pub fn clear(&mut self) {
        self.analysis.write().clear();
    }

    /// DagStateからDAGを構築
    fn build_dag(&self, dag_state: &crate::state::DagState) -> DAG {
        let mut dag = DAG::new();

        // タスクを追加
        for (task_id, ui_task) in &dag_state.tasks {
            let task = task_composer_core::types::Task {
                task_id: task_id.clone(),
                name: ui_task.name.clone(),
                description: ui_task.description.clone(),
                priority: ui_task.priority,
                status: ui_task.status.clone().into(),
                prompt: ui_task.prompt.clone(),
                executor: ui_task.executor.clone(),
                dependencies: ui_task.dependencies.clone(),
                role: ui_task.role.clone(),
                args: ui_task.args.clone(),
                inputs: ui_task.inputs.clone(),
                if_condition: None,
                else_condition: None,
            };
            dag.add_task(task);
        }

        // エッジを追加（依存関係から）
        for (task_id, ui_task) in &dag_state.tasks {
            for dep in &ui_task.dependencies {
                dag.add_edge(dep, task_id);
            }
        }

        dag
    }
}

/// coreの解析結果をUIの状態に変換
fn convert_analysis_result(core_result: &CoreAnalysisResult) -> (DagAnalysis, TaskValidation) {
    // DAG構造解析の変換
    let dag_analysis = DagAnalysis {
        topological_order: core_result.dag_structure.topological_order.clone(),
        has_cycle: core_result.dag_structure.has_cycle,
        parallel_pairs: core_result.dag_structure.parallel_pairs.clone(),
        orphan_nodes: core_result.dag_structure.orphan_nodes.clone(),
        root_nodes: core_result.dag_structure.root_nodes.clone(),
        leaf_nodes: core_result.dag_structure.leaf_nodes.clone(),
        node_depths: core_result.dag_structure.node_depths.clone(),
        critical_path: core_result.dag_structure.critical_path.clone(),
    };

    // 検証項目の変換
    let items: Vec<AnalysisItem> = core_result.items.iter().map(|item| {
        AnalysisItem {
            level: match item.level {
                task_composer_core::analysis::AnalysisLevel::Info => AnalysisLevel::Info,
                task_composer_core::analysis::AnalysisLevel::Warning => AnalysisLevel::Warning,
                task_composer_core::analysis::AnalysisLevel::Error => AnalysisLevel::Error,
            },
            category: item.category.clone(),
            message: item.message.clone(),
            related_tasks: item.related_tasks.clone(),
        }
    }).collect();

    let task_validation = TaskValidation { items };

    (dag_analysis, task_validation)
}
