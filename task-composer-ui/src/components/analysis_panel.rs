//! 静的解析パネルコンポーネント

use dioxus::prelude::*;
use crate::state::{use_app_state, AnalysisLevel};
use crate::hooks::use_analysis;

/// 解析パネルコンポーネント
#[component]
pub fn AnalysisPanel() -> Element {
    let app_state = use_app_state();
    let mut analysis_ops = use_analysis();
    let analysis = app_state.analysis.read();

    let error_count = analysis.error_count();
    let warning_count = analysis.warning_count();

    rsx! {
        div {
            style: "padding: 16px; height: 100%; overflow-y: auto; background: #0a0f1a;",

            // ヘッダー
            div {
                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px;",

                h3 {
                    style: "margin: 0; font-size: 14px; color: #f1f5f9; font-weight: 600;",
                    "Static Analysis"
                }

                button {
                    style: "padding: 6px 12px; background: linear-gradient(135deg, #818cf8 0%, #6366f1 100%); color: #fff; border: none; border-radius: 6px; cursor: pointer; font-size: 11px; font-weight: 500;",
                    onclick: move |_| analysis_ops.run_analysis(),
                    "Run Analysis"
                }
            }

            // サマリー
            if analysis.analyzed {
                div {
                    style: "display: flex; gap: 8px; margin-bottom: 16px;",

                    SummaryBadge {
                        count: error_count,
                        label: "Errors",
                        color: "#ef4444",
                        bg: "rgba(239, 68, 68, 0.15)"
                    }
                    SummaryBadge {
                        count: warning_count,
                        label: "Warnings",
                        color: "#f59e0b",
                        bg: "rgba(245, 158, 11, 0.15)"
                    }
                    SummaryBadge {
                        count: analysis.dag_analysis.parallel_pairs.len(),
                        label: "Parallel",
                        color: "#10b981",
                        bg: "rgba(16, 185, 129, 0.15)"
                    }
                }
            }

            // 解析結果
            if analysis.analyzed {
                // DAG構造
                DagStructureSection {}

                // 検証結果
                ValidationSection {}
            } else {
                div {
                    style: "text-align: center; padding: 32px; color: #64748b; font-size: 12px;",
                    "Click 'Run Analysis' to analyze the DAG"
                }
            }
        }
    }
}

#[component]
fn SummaryBadge(count: usize, label: &'static str, color: &'static str, bg: &'static str) -> Element {
    rsx! {
        div {
            style: "padding: 8px 12px; background: {bg}; border-radius: 6px; display: flex; align-items: center; gap: 6px;",
            span {
                style: "font-size: 16px; font-weight: 600; color: {color};",
                "{count}"
            }
            span {
                style: "font-size: 10px; color: {color}; text-transform: uppercase;",
                "{label}"
            }
        }
    }
}

#[component]
fn DagStructureSection() -> Element {
    let app_state = use_app_state();
    let analysis = app_state.analysis.read();
    let dag = &analysis.dag_analysis;

    rsx! {
        div {
            style: "margin-bottom: 16px;",

            SectionHeader { title: "DAG Structure" }

            // 循環検出
            if dag.has_cycle {
                AnalysisRow {
                    icon: "error",
                    level: "error",
                    text: "Cycle detected in DAG"
                }
            } else {
                AnalysisRow {
                    icon: "check",
                    level: "success",
                    text: "No cycles detected"
                }
            }

            // ルートノード
            if !dag.root_nodes.is_empty() {
                AnalysisRow {
                    icon: "info",
                    level: "info",
                    text: format!("Root nodes: {}", dag.root_nodes.join(", "))
                }
            }

            // リーフノード
            if !dag.leaf_nodes.is_empty() {
                AnalysisRow {
                    icon: "info",
                    level: "info",
                    text: format!("Leaf nodes: {}", dag.leaf_nodes.join(", "))
                }
            }

            // 孤立ノード
            if !dag.orphan_nodes.is_empty() {
                AnalysisRow {
                    icon: "warning",
                    level: "warning",
                    text: format!("Orphan nodes: {}", dag.orphan_nodes.join(", "))
                }
            }

            // クリティカルパス
            if !dag.critical_path.is_empty() {
                AnalysisRow {
                    icon: "info",
                    level: "info",
                    text: format!("Critical path: {}", dag.critical_path.join(" -> "))
                }
            }

            // 並列実行可能ペア
            if !dag.parallel_pairs.is_empty() {
                details {
                    style: "margin-top: 8px;",
                    summary {
                        style: "cursor: pointer; font-size: 11px; color: #94a3b8; padding: 4px 0;",
                        "{dag.parallel_pairs.len()} parallel execution pairs"
                    }
                    div {
                        style: "padding: 8px; background: #0f172a; border-radius: 6px; margin-top: 4px; max-height: 120px; overflow-y: auto;",
                        for (idx, (a, b)) in dag.parallel_pairs.iter().enumerate() {
                            div {
                                key: "{idx}-{a}-{b}",
                                style: "font-size: 11px; color: #64748b; padding: 2px 0;",
                                "{a} || {b}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ValidationSection() -> Element {
    let app_state = use_app_state();
    let analysis = app_state.analysis.read();
    let validation = &analysis.task_validation;

    if validation.items.is_empty() {
        return rsx! {
            div {
                style: "margin-bottom: 16px;",
                SectionHeader { title: "Task Validation" }
                AnalysisRow {
                    icon: "check",
                    level: "success",
                    text: "All tasks are valid"
                }
            }
        };
    }

    rsx! {
        div {
            style: "margin-bottom: 16px;",

            SectionHeader { title: "Task Validation" }

            for (idx, item) in validation.items.iter().enumerate() {
                {
                    let level = match item.level {
                        AnalysisLevel::Error => "error",
                        AnalysisLevel::Warning => "warning",
                        AnalysisLevel::Info => "info",
                    };
                    let icon = match item.level {
                        AnalysisLevel::Error => "error",
                        AnalysisLevel::Warning => "warning",
                        AnalysisLevel::Info => "info",
                    };
                    rsx! {
                        AnalysisRow {
                            key: "{idx}",
                            icon: icon,
                            level: level,
                            text: item.message.clone()
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SectionHeader(title: &'static str) -> Element {
    rsx! {
        div {
            style: "font-size: 11px; color: #64748b; text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 8px; font-weight: 600;",
            "{title}"
        }
    }
}

#[component]
fn AnalysisRow(icon: &'static str, level: &'static str, text: String) -> Element {
    let (icon_char, color) = match (icon, level) {
        ("check", "success") => ("✓", "#10b981"),
        ("error", "error") => ("✕", "#ef4444"),
        ("warning", "warning") => ("⚠", "#f59e0b"),
        ("info", _) | (_, "info") => ("ℹ", "#60a5fa"),
        _ => ("•", "#94a3b8"),
    };

    rsx! {
        div {
            style: "display: flex; align-items: flex-start; gap: 8px; padding: 6px 8px; background: #0f172a; border-radius: 6px; margin-bottom: 4px;",
            span {
                style: "color: {color}; font-size: 12px; flex-shrink: 0;",
                "{icon_char}"
            }
            span {
                style: "color: #94a3b8; font-size: 11px; line-height: 1.4;",
                "{text}"
            }
        }
    }
}
