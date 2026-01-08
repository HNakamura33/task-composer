//! ノードパレットコンポーネント - モダンデザイン

use dioxus::prelude::*;
use crate::state::use_app_state;

/// ノードパレット - ドラッグしてノードを追加（n8n風）
#[component]
pub fn NodePalette() -> Element {
    rsx! {
        div {
            style: "padding: 16px;",

            // セクションヘッダー
            div {
                style: "display: flex; align-items: center; gap: 8px; margin-bottom: 12px;",
                span {
                    style: "font-size: 14px;",
                    "🧩"
                }
                h3 {
                    style: "margin: 0; font-size: 11px; color: #64748b; text-transform: uppercase; letter-spacing: 0.08em; font-weight: 600;",
                    "Nodes"
                }
            }

            // パレットアイテム
            div {
                style: "display: flex; flex-direction: column; gap: 8px;",

                PaletteItem {
                    executor: "log",
                    label: "Log",
                    icon: "📝",
                    color: "#8b5cf6",
                    description: "Debug output"
                }
                PaletteItem {
                    executor: "mcp",
                    label: "MCP",
                    icon: "🤖",
                    color: "#f59e0b",
                    description: "AI Agent"
                }
            }

            // ヒント
            div {
                style: "margin-top: 16px; padding: 10px; background: rgba(99, 102, 241, 0.1); border-radius: 6px; border: 1px dashed #334155;",
                p {
                    style: "margin: 0; font-size: 10px; color: #64748b; line-height: 1.5;",
                    "Drag nodes onto the canvas to create your workflow"
                }
            }
        }
    }
}

#[component]
fn PaletteItem(
    executor: &'static str,
    label: &'static str,
    icon: &'static str,
    color: &'static str,
    description: &'static str,
) -> Element {
    let app_state = use_app_state();
    let mut ui = app_state.ui;

    let on_mousedown = move |evt: MouseEvent| {
        evt.prevent_default();
        ui.write().start_dragging_from_palette(
            executor.to_string(),
            0.0,
            0.0,
        );
    };

    rsx! {
        div {
            style: "display: flex; align-items: center; gap: 12px; padding: 10px 12px; background: linear-gradient(135deg, #1e293b 0%, #0f172a 100%); border-radius: 8px; cursor: grab; border: 1px solid #334155; transition: all 0.15s;",
            onmousedown: on_mousedown,
            title: "{description}",

            // アイコン
            div {
                style: "width: 36px; height: 36px; background: {color}; opacity: 0.15; border-radius: 8px; display: flex; align-items: center; justify-content: center; position: relative;",
                div {
                    style: "position: absolute; font-size: 18px;",
                    "{icon}"
                }
            }

            // ラベルと説明
            div {
                style: "flex: 1;",
                div {
                    style: "font-size: 12px; color: #f1f5f9; font-weight: 500; margin-bottom: 2px;",
                    "{label}"
                }
                div {
                    style: "font-size: 10px; color: #64748b;",
                    "{description}"
                }
            }

            // ドラッグインジケーター
            div {
                style: "color: #475569; font-size: 12px;",
                "⋮⋮"
            }
        }
    }
}
