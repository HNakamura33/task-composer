//! ツールバーコンポーネント - モダンデザイン

use dioxus::prelude::*;
use crate::state::{use_app_state, ViewMode};
use crate::hooks::use_file_operations;

/// ツールバーコンポーネント（n8n/Dify風）
#[component]
pub fn Toolbar() -> Element {
    let app_state = use_app_state();
    let mut ui = app_state.ui;
    let dag = app_state.dag;

    let task_count = dag.read().tasks.len();
    let is_dirty = dag.read().is_dirty;

    rsx! {
        header {
            class: "toolbar",
            style: "display: flex; align-items: center; padding: 0 16px; height: 56px; background: linear-gradient(180deg, #0f172a 0%, #0a0f1a 100%); border-bottom: 1px solid #1e293b; gap: 16px;",

            // ロゴ・タイトル
            div {
                style: "display: flex; align-items: center; gap: 10px;",
                div {
                    style: "width: 32px; height: 32px; background: linear-gradient(135deg, #818cf8 0%, #6366f1 100%); border-radius: 8px; display: flex; align-items: center; justify-content: center; font-size: 16px;",
                    "📊"
                }
                div {
                    h1 {
                        style: "margin: 0; font-size: 15px; color: #f1f5f9; font-weight: 600; letter-spacing: -0.02em;",
                        "Task Composer"
                    }
                    if is_dirty {
                        span {
                            style: "font-size: 10px; color: #f59e0b; margin-left: 6px; padding: 2px 6px; background: rgba(245, 158, 11, 0.15); border-radius: 4px;",
                            "Unsaved"
                        }
                    }
                }
            }

            // セパレータ
            div {
                style: "width: 1px; height: 24px; background: #1e293b;",
            }

            // ファイル操作ボタン
            div {
                style: "display: flex; gap: 6px;",
                SampleButton {}
                OpenButton {}
                SaveButton {}
            }

            // スペーサー
            div { style: "flex: 1;" }

            // ビューモード切り替え
            div {
                style: "display: flex; gap: 2px; background: #0f172a; padding: 3px; border-radius: 8px; border: 1px solid #1e293b;",

                ViewModeButton {
                    label: "Graph",
                    icon: "◇",
                    mode: ViewMode::Graph,
                    current: ui.read().view_mode.clone(),
                    on_click: move |_| ui.write().set_view_mode(ViewMode::Graph)
                }
                ViewModeButton {
                    label: "List",
                    icon: "☰",
                    mode: ViewMode::List,
                    current: ui.read().view_mode.clone(),
                    on_click: move |_| ui.write().set_view_mode(ViewMode::List)
                }
                ViewModeButton {
                    label: "JSON",
                    icon: "⟨⟩",
                    mode: ViewMode::Json,
                    current: ui.read().view_mode.clone(),
                    on_click: move |_| ui.write().set_view_mode(ViewMode::Json)
                }
            }

            // セパレータ
            div {
                style: "width: 1px; height: 24px; background: #1e293b;",
            }

            // 解析ボタン
            AnalysisButton {}

            // タスク数表示
            div {
                style: "display: flex; align-items: center; gap: 6px; padding: 6px 12px; background: #0f172a; border-radius: 6px; border: 1px solid #1e293b;",
                span {
                    style: "font-size: 14px; font-weight: 600; color: #818cf8;",
                    "{task_count}"
                }
                span {
                    style: "color: #64748b; font-size: 12px;",
                    "nodes"
                }
            }
        }
    }
}

#[component]
fn AnalysisButton() -> Element {
    let app_state = use_app_state();
    let mut ui = app_state.ui;
    let analysis = app_state.analysis;

    let is_open = ui.read().analysis_panel_open;
    let error_count = analysis.read().error_count();
    let warning_count = analysis.read().warning_count();

    let (bg, border) = if is_open {
        ("linear-gradient(135deg, #818cf8 0%, #6366f1 100%)", "transparent")
    } else {
        ("#1e293b", "#334155")
    };

    rsx! {
        button {
            style: "padding: 6px 12px; background: {bg}; color: #fff; border: 1px solid {border}; border-radius: 6px; cursor: pointer; font-size: 12px; font-weight: 500; display: flex; align-items: center; gap: 6px; transition: all 0.15s;",
            onclick: move |_| ui.write().toggle_analysis_panel(),
            span { style: "font-size: 12px;", "🔍" }
            "Analysis"

            // エラー/警告バッジ
            if error_count > 0 {
                span {
                    style: "padding: 2px 6px; background: #ef4444; border-radius: 10px; font-size: 10px; font-weight: 600;",
                    "{error_count}"
                }
            } else if warning_count > 0 {
                span {
                    style: "padding: 2px 6px; background: #f59e0b; border-radius: 10px; font-size: 10px; font-weight: 600; color: #000;",
                    "{warning_count}"
                }
            }
        }
    }
}

#[component]
fn SampleButton() -> Element {
    let mut file_ops = use_file_operations();

    rsx! {
        button {
            style: "padding: 8px 14px; background: linear-gradient(135deg, #10b981 0%, #059669 100%); color: #fff; border: none; border-radius: 6px; cursor: pointer; font-size: 12px; font-weight: 500; display: flex; align-items: center; gap: 6px; transition: all 0.15s;",
            onclick: move |_| {
                file_ops.load_sample();
            },
            span { style: "font-size: 14px;", "✨" }
            "Sample"
        }
    }
}

// Desktop版のOpen/Saveボタン
#[cfg(feature = "desktop")]
#[component]
fn OpenButton() -> Element {
    let mut file_ops = use_file_operations();
    let mut error_msg = use_signal(|| None::<String>);

    rsx! {
        button {
            style: "padding: 8px 14px; background: #1e293b; color: #94a3b8; border: 1px solid #334155; border-radius: 6px; cursor: pointer; font-size: 12px; font-weight: 500; display: flex; align-items: center; gap: 6px; transition: all 0.15s;",
            onclick: move |_| {
                spawn(async move {
                    if let Some(file) = rfd::AsyncFileDialog::new()
                        .add_filter("JSON", &["json"])
                        .pick_file()
                        .await
                    {
                        let contents = file.read().await;
                        match String::from_utf8(contents) {
                            Ok(json) => {
                                if let Err(e) = file_ops.load_json(&json) {
                                    error_msg.set(Some(format!("Parse error: {}", e)));
                                } else {
                                    error_msg.set(None);
                                }
                            }
                            Err(e) => {
                                error_msg.set(Some(format!("UTF-8 error: {}", e)));
                            }
                        }
                    }
                });
            },
            span { style: "font-size: 14px;", "📂" }
            "Open"
        }

        if let Some(err) = error_msg.read().as_ref() {
            div {
                style: "position: fixed; top: 64px; left: 50%; transform: translateX(-50%); padding: 10px 16px; background: #7f1d1d; color: #fecaca; border-radius: 6px; font-size: 12px; z-index: 100;",
                "{err}"
            }
        }
    }
}

#[cfg(feature = "desktop")]
#[component]
fn SaveButton() -> Element {
    let file_ops = use_file_operations();
    let mut error_msg = use_signal(|| None::<String>);

    rsx! {
        button {
            style: "padding: 8px 14px; background: #1e293b; color: #94a3b8; border: 1px solid #334155; border-radius: 6px; cursor: pointer; font-size: 12px; font-weight: 500; display: flex; align-items: center; gap: 6px; transition: all 0.15s;",
            onclick: move |_| {
                let json_result = file_ops.save_json();
                spawn(async move {
                    match json_result {
                        Ok(json) => {
                            if let Some(file) = rfd::AsyncFileDialog::new()
                                .add_filter("JSON", &["json"])
                                .set_file_name("workflow.json")
                                .save_file()
                                .await
                            {
                                if let Err(e) = file.write(json.as_bytes()).await {
                                    error_msg.set(Some(format!("Save failed: {}", e)));
                                } else {
                                    error_msg.set(None);
                                }
                            }
                        }
                        Err(e) => {
                            error_msg.set(Some(format!("Serialize error: {}", e)));
                        }
                    }
                });
            },
            span { style: "font-size: 14px;", "💾" }
            "Save"
        }

        if let Some(err) = error_msg.read().as_ref() {
            div {
                style: "position: fixed; top: 64px; left: 50%; transform: translateX(-50%); padding: 10px 16px; background: #7f1d1d; color: #fecaca; border-radius: 6px; font-size: 12px; z-index: 100;",
                "{err}"
            }
        }
    }
}

// Web版のOpen/Saveボタン
#[cfg(feature = "web")]
#[component]
fn OpenButton() -> Element {
    let mut file_ops = use_file_operations();

    rsx! {
        label {
            style: "padding: 8px 14px; background: #1e293b; color: #94a3b8; border: 1px solid #334155; border-radius: 6px; cursor: pointer; font-size: 12px; font-weight: 500; display: flex; align-items: center; gap: 6px; transition: all 0.15s;",
            span { style: "font-size: 14px;", "📂" }
            "Open"
            input {
                r#type: "file",
                accept: ".json",
                style: "display: none;",
                onchange: move |evt| {
                    if let Some(file_engine) = &evt.files() {
                        let files = file_engine.files();
                        if let Some(file_name) = files.first() {
                            let file_engine = file_engine.clone();
                            let file_name = file_name.clone();
                            spawn(async move {
                                if let Some(contents) = file_engine.read_file(&file_name).await {
                                    if let Ok(json) = String::from_utf8(contents) {
                                        let _ = file_ops.load_json(&json);
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }
    }
}

#[cfg(feature = "web")]
#[component]
fn SaveButton() -> Element {
    let file_ops = use_file_operations();

    rsx! {
        button {
            style: "padding: 8px 14px; background: #1e293b; color: #94a3b8; border: 1px solid #334155; border-radius: 6px; cursor: pointer; font-size: 12px; font-weight: 500; display: flex; align-items: center; gap: 6px; transition: all 0.15s;",
            onclick: move |_| {
                if let Ok(json) = file_ops.save_json() {
                    // Web APIでダウンロード
                    #[cfg(feature = "web")]
                    {
                        use wasm_bindgen::JsCast;
                        let window = web_sys::window().unwrap();
                        let document = window.document().unwrap();

                        let blob_parts = js_sys::Array::new();
                        blob_parts.push(&json.into());

                        let blob = web_sys::Blob::new_with_str_sequence_and_options(
                            &blob_parts,
                            web_sys::BlobPropertyBag::new().type_("application/json"),
                        ).unwrap();

                        let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();

                        let a = document.create_element("a").unwrap();
                        let a: web_sys::HtmlAnchorElement = a.dyn_into().unwrap();
                        a.set_href(&url);
                        a.set_download("workflow.json");
                        a.click();

                        web_sys::Url::revoke_object_url(&url).unwrap();
                    }
                }
            },
            span { style: "font-size: 14px;", "💾" }
            "Save"
        }
    }
}

// TUI/その他のフォールバック
#[cfg(not(any(feature = "desktop", feature = "web")))]
#[component]
fn OpenButton() -> Element {
    rsx! {
        button {
            style: "padding: 8px 14px; background: #1e293b; color: #64748b; border: 1px solid #334155; border-radius: 6px; cursor: not-allowed; font-size: 12px; font-weight: 500; display: flex; align-items: center; gap: 6px;",
            disabled: true,
            span { style: "font-size: 14px;", "📂" }
            "Open"
        }
    }
}

#[cfg(not(any(feature = "desktop", feature = "web")))]
#[component]
fn SaveButton() -> Element {
    rsx! {
        button {
            style: "padding: 8px 14px; background: #1e293b; color: #64748b; border: 1px solid #334155; border-radius: 6px; cursor: not-allowed; font-size: 12px; font-weight: 500; display: flex; align-items: center; gap: 6px;",
            disabled: true,
            span { style: "font-size: 14px;", "💾" }
            "Save"
        }
    }
}

#[component]
fn ViewModeButton(
    label: &'static str,
    icon: &'static str,
    mode: ViewMode,
    current: ViewMode,
    on_click: EventHandler<MouseEvent>,
) -> Element {
    let is_active = mode == current;
    let (bg, color) = if is_active {
        ("linear-gradient(135deg, #818cf8 0%, #6366f1 100%)", "#fff")
    } else {
        ("transparent", "#64748b")
    };

    rsx! {
        button {
            style: "padding: 6px 12px; background: {bg}; color: {color}; border: none; border-radius: 6px; cursor: pointer; font-size: 11px; font-weight: 500; display: flex; align-items: center; gap: 4px; transition: all 0.15s;",
            onclick: move |e| on_click.call(e),
            span { style: "font-size: 10px;", "{icon}" }
            "{label}"
        }
    }
}
