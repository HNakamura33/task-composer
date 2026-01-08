//! Task Composer UI - DAG管理アプリケーション

mod state;
mod components;
mod hooks;

use dioxus::prelude::*;
use state::{AppState, ViewMode, DagState};
use components::{Toolbar, Sidebar, GraphCanvas, TaskDetail, AnalysisPanel};

/// グローバルな初期ファイルパス（CLI引数から設定）
static INITIAL_FILE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

fn main() {
    // CLI引数からファイルパスを取得
    let args: Vec<String> = std::env::args().collect();
    let file_path = args.get(1).cloned();
    INITIAL_FILE.set(file_path).ok();

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // アプリケーション状態を初期化
    let _app_state = use_context_provider(AppState::new);

    // 初期化時にファイルを読み込み
    use_effect(move || {
        if let Some(Some(path)) = INITIAL_FILE.get() {
            if let Ok(contents) = std::fs::read_to_string(path) {
                if let Ok(dag_state) = DagState::from_json(&contents) {
                    let mut app_state = use_context::<AppState>();
                    *app_state.dag.write() = dag_state;
                }
            }
        }
    });

    rsx! {
        // グローバルスタイル
        style { {GLOBAL_STYLES} }

        div {
            style: "display: flex; flex-direction: column; height: 100vh; background: #0a0f1a; color: #f1f5f9; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;",

            // ツールバー
            Toolbar {}

            // メインコンテンツ
            div {
                style: "display: flex; flex: 1; overflow: hidden;",

                // 左サイドバー（パレット・タスクリスト）
                Sidebar {}

                // 中央エリア（グラフ）
                MainContent {}

                // 右ペイン（タスク編集）
                RightPane {}
            }
        }
    }
}

#[component]
fn MainContent() -> Element {
    let app_state = state::use_app_state();
    let ui = app_state.ui.read();

    match ui.view_mode {
        ViewMode::Graph => rsx! { GraphCanvas {} },
        ViewMode::List => rsx! { ListView {} },
        ViewMode::Json => rsx! { JsonView {} },
    }
}

/// 右ペイン - タスク詳細/解析パネル
#[component]
fn RightPane() -> Element {
    let app_state = state::use_app_state();
    let ui = app_state.ui.read();
    let selected = ui.selected_task.clone();
    let analysis_open = ui.analysis_panel_open;

    // 両方とも非表示の場合
    if !analysis_open && selected.is_none() {
        return rsx! {};
    }

    rsx! {
        aside {
            style: "width: 340px; background: linear-gradient(180deg, #0f172a 0%, #0a0f1a 100%); border-left: 1px solid #1e293b; display: flex; flex-direction: column; overflow: hidden;",

            // 解析パネル
            if analysis_open {
                div {
                    style: if selected.is_some() { "flex: 1; min-height: 200px; max-height: 50%; border-bottom: 1px solid #1e293b; overflow: hidden;" } else { "flex: 1; overflow: hidden;" },
                    AnalysisPanel {}
                }
            }

            // タスク詳細
            if let Some(task_id) = selected {
                div {
                    style: "flex: 1; overflow-y: auto;",
                    TaskDetail { task_id }
                }
            }
        }
    }
}

#[component]
fn ListView() -> Element {
    let app_state = state::use_app_state();
    let dag = app_state.dag.read();

    let mut tasks: Vec<_> = dag.tasks.values().collect();
    tasks.sort_by(|a, b| a.task_id.cmp(&b.task_id));

    rsx! {
        div {
            style: "flex: 1; padding: 24px; overflow-y: auto; background: #0a0f1a;",

            h2 { style: "margin: 0 0 20px 0; font-size: 16px; color: #f1f5f9; font-weight: 600;", "Task List" }

            table {
                style: "width: 100%; border-collapse: separate; border-spacing: 0; background: #0f172a; border-radius: 8px; overflow: hidden; border: 1px solid #1e293b;",

                thead {
                    tr {
                        style: "background: linear-gradient(180deg, #1e293b 0%, #0f172a 100%);",
                        th { style: "padding: 12px 16px; text-align: left; border-bottom: 1px solid #1e293b; font-size: 11px; color: #64748b; text-transform: uppercase; letter-spacing: 0.05em; font-weight: 600;", "ID" }
                        th { style: "padding: 12px 16px; text-align: left; border-bottom: 1px solid #1e293b; font-size: 11px; color: #64748b; text-transform: uppercase; letter-spacing: 0.05em; font-weight: 600;", "Name" }
                        th { style: "padding: 12px 16px; text-align: left; border-bottom: 1px solid #1e293b; font-size: 11px; color: #64748b; text-transform: uppercase; letter-spacing: 0.05em; font-weight: 600;", "Status" }
                        th { style: "padding: 12px 16px; text-align: left; border-bottom: 1px solid #1e293b; font-size: 11px; color: #64748b; text-transform: uppercase; letter-spacing: 0.05em; font-weight: 600;", "Executor" }
                        th { style: "padding: 12px 16px; text-align: left; border-bottom: 1px solid #1e293b; font-size: 11px; color: #64748b; text-transform: uppercase; letter-spacing: 0.05em; font-weight: 600;", "Dependencies" }
                    }
                }

                tbody {
                    for task in tasks {
                        tr {
                            key: "{task.task_id}",
                            td { style: "padding: 12px 16px; border-bottom: 1px solid #1e293b; font-size: 12px; color: #94a3b8; font-family: monospace;", "#{task.task_id}" }
                            td { style: "padding: 12px 16px; border-bottom: 1px solid #1e293b; font-size: 13px; color: #f1f5f9; font-weight: 500;", "{task.name}" }
                            td { style: "padding: 12px 16px; border-bottom: 1px solid #1e293b; font-size: 12px; color: #64748b;", "{task.status:?}" }
                            td { style: "padding: 12px 16px; border-bottom: 1px solid #1e293b; font-size: 12px; color: #64748b;", "{task.executor}" }
                            td { style: "padding: 12px 16px; border-bottom: 1px solid #1e293b; font-size: 12px; color: #64748b;", "{task.dependencies.join(\", \")}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn JsonView() -> Element {
    let app_state = state::use_app_state();
    let dag = app_state.dag.read();

    let json = dag.to_json().unwrap_or_else(|e| format!("Error: {}", e));

    rsx! {
        div {
            style: "flex: 1; padding: 24px; overflow-y: auto; background: #0a0f1a;",

            h2 { style: "margin: 0 0 20px 0; font-size: 16px; color: #f1f5f9; font-weight: 600;", "JSON View" }

            pre {
                style: "background: linear-gradient(135deg, #0f172a 0%, #1e293b 100%); padding: 20px; border-radius: 12px; overflow-x: auto; font-size: 12px; line-height: 1.6; border: 1px solid #1e293b; margin: 0;",
                code {
                    style: "color: #94a3b8; font-family: 'SF Mono', 'Fira Code', monospace;",
                    "{json}"
                }
            }
        }
    }
}

const GLOBAL_STYLES: &str = r#"
    * {
        box-sizing: border-box;
    }

    body {
        margin: 0;
        padding: 0;
    }

    ::-webkit-scrollbar {
        width: 6px;
        height: 6px;
    }

    ::-webkit-scrollbar-track {
        background: transparent;
    }

    ::-webkit-scrollbar-thumb {
        background: #334155;
        border-radius: 3px;
    }

    ::-webkit-scrollbar-thumb:hover {
        background: #475569;
    }

    button:hover {
        filter: brightness(1.1);
    }

    button:active {
        transform: scale(0.98);
    }

    input:focus, textarea:focus, select:focus {
        outline: none;
        border-color: #818cf8 !important;
    }
"#;
