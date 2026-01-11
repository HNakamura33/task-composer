//! タスク一覧コンポーネント - モダンデザイン

use dioxus::prelude::*;
use crate::state::{use_app_state, UiStatus};

/// タスク一覧（n8n風）
#[component]
pub fn TaskList() -> Element {
    let app_state = use_app_state();
    let dag = app_state.dag.read();
    let mut ui = app_state.ui;

    let selected_id = ui.read().selected_task.clone();

    // タスクをIDでソート
    let mut tasks: Vec<_> = dag.tasks.values().collect();
    tasks.sort_by(|a, b| a.task_id.cmp(&b.task_id));

    rsx! {
        div {
            // セクションヘッダー
            div {
                style: "display: flex; align-items: center; gap: 8px; margin-bottom: 12px;",
                span {
                    style: "font-size: 14px;",
                    "📋"
                }
                h3 {
                    style: "margin: 0; font-size: 11px; color: #64748b; text-transform: uppercase; letter-spacing: 0.08em; font-weight: 600;",
                    "Workflow"
                }
                span {
                    style: "margin-left: auto; font-size: 11px; color: #475569; background: #1e293b; padding: 2px 8px; border-radius: 10px;",
                    "{tasks.len()}"
                }
            }

            // タスクリスト
            div {
                style: "display: flex; flex-direction: column; gap: 6px;",
                for task in tasks {
                    TaskListItem {
                        key: "{task.task_id}",
                        task_id: task.task_id.clone(),
                        name: task.name.clone(),
                        executor: task.executor.clone(),
                        status: task.status.clone(),
                        selected: selected_id.as_ref() == Some(&task.task_id),
                        on_select: move |id: String| {
                            ui.write().select_task(Some(id));
                        }
                    }
                }
            }

            // 空の状態
            if dag.tasks.is_empty() {
                div {
                    style: "text-align: center; padding: 24px 16px;",
                    div {
                        style: "font-size: 32px; opacity: 0.3; margin-bottom: 8px;",
                        "📭"
                    }
                    p {
                        style: "color: #475569; font-size: 12px; margin: 0;",
                        "No nodes yet"
                    }
                }
            }
        }
    }
}

#[component]
fn TaskListItem(
    task_id: String,
    name: String,
    executor: String,
    status: UiStatus,
    selected: bool,
    on_select: EventHandler<String>,
) -> Element {
    let (status_color, status_glow) = match status {
        UiStatus::Pending => ("#64748b", "none"),
        UiStatus::InProgress => ("#3b82f6", "0 0 8px rgba(59, 130, 246, 0.5)"),
        UiStatus::Completed => ("#10b981", "0 0 8px rgba(16, 185, 129, 0.5)"),
        UiStatus::Failed => ("#ef4444", "0 0 8px rgba(239, 68, 68, 0.5)"),
    };

    let executor_icon = match executor.as_str() {
        "log" => "📝",
        "mcp" => "🤖",
        _ => "⚙",
    };

    let (bg, border) = if selected {
        ("linear-gradient(135deg, rgba(99, 102, 241, 0.15) 0%, rgba(99, 102, 241, 0.05) 100%)", "1px solid #818cf8")
    } else {
        ("linear-gradient(135deg, #1e293b 0%, #0f172a 100%)", "1px solid #334155")
    };

    let id_for_click = task_id.clone();

    rsx! {
        div {
            style: "display: flex; align-items: center; gap: 10px; padding: 10px 12px; background: {bg}; border-radius: 8px; cursor: pointer; border: {border}; transition: all 0.15s;",
            onclick: move |_| on_select.call(id_for_click.clone()),

            // アイコン
            div {
                style: "font-size: 16px;",
                "{executor_icon}"
            }

            // タスク情報
            div {
                style: "flex: 1; overflow: hidden;",
                div {
                    style: "font-size: 12px; color: #f1f5f9; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;",
                    "{name}"
                }
                div {
                    style: "font-size: 10px; color: #64748b; font-family: monospace;",
                    "#{task_id}"
                }
            }

            // ステータスインジケーター
            div {
                style: "width: 8px; height: 8px; border-radius: 50%; background: {status_color}; box-shadow: {status_glow}; flex-shrink: 0;",
            }
        }
    }
}
