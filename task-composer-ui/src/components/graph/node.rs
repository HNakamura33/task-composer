//! タスクノードコンポーネント - モダンデザイン

use dioxus::prelude::*;
use crate::state::{use_app_state, UiStatus, DragState, PortType};
use super::canvas::{NODE_WIDTH, NODE_HEIGHT};

/// グラフ上のタスクノード（Dify/n8n風デザイン）
#[component]
pub fn TaskNode(
    task_id: String,
    name: String,
    executor: String,
    status: UiStatus,
    x: f32,
    y: f32,
    selected: bool,
) -> Element {
    let app_state = use_app_state();
    let mut ui = app_state.ui;
    let mut dag = app_state.dag;

    // ステータスに応じた色とグロー
    let (status_color, status_glow) = match status {
        UiStatus::Pending => ("#64748b", "none"),
        UiStatus::InProgress => ("#3b82f6", "url(#glow-blue)"),
        UiStatus::Completed => ("#10b981", "url(#glow-green)"),
        UiStatus::Failed => ("#ef4444", "url(#glow-red)"),
    };

    // Executor色とアイコン
    let (executor_color, executor_icon) = match executor.as_str() {
        "log" => ("#8b5cf6", "📝"),
        "mcp" => ("#f59e0b", "🤖"),
        _ => ("#64748b", "⚙"),
    };

    // 選択状態のスタイル
    let (stroke_color, stroke_width, shadow_filter) = if selected {
        ("#818cf8", "2", "url(#shadow-selected)")
    } else {
        ("#334155", "1", "url(#shadow-normal)")
    };

    let task_id_for_drag = task_id.clone();
    let task_id_for_click = task_id.clone();
    let task_id_for_output = task_id.clone();
    let task_id_for_input = task_id.clone();

    // ノードドラッグ開始
    let on_node_mousedown = move |evt: MouseEvent| {
        evt.stop_propagation();
        let coords = evt.element_coordinates();
        let zoom = ui.read().zoom;
        let mouse_x = coords.x as f32 / zoom;
        let mouse_y = coords.y as f32 / zoom;
        let offset_x = mouse_x - x;
        let offset_y = mouse_y - y;
        ui.write().start_dragging_node(task_id_for_drag.clone(), offset_x, offset_y);
    };

    // ノードクリック（選択）
    let on_node_click = move |evt: MouseEvent| {
        evt.stop_propagation();
        ui.write().select_task(Some(task_id_for_click.clone()));
    };

    // 出力ポートからの接続開始
    let on_output_mousedown = move |evt: MouseEvent| {
        evt.stop_propagation();
        ui.write().start_connecting(
            task_id_for_output.clone(),
            PortType::Output,
            x + NODE_WIDTH / 2.0,
            y + NODE_HEIGHT,
        );
    };

    // 入力ポートへの接続完了
    let on_input_mouseup = move |evt: MouseEvent| {
        evt.stop_propagation();
        let drag = ui.read().drag_state.clone();
        if let DragState::Connecting { from_task_id, from_port: PortType::Output, .. } = drag {
            if from_task_id != task_id_for_input {
                dag.write().add_edge(&from_task_id, &task_id_for_input);
            }
        }
        ui.write().end_drag();
    };

    rsx! {
        g {
            class: "task-node",
            onmousedown: on_node_mousedown,
            onclick: on_node_click,
            cursor: "grab",

            // シャドウ付きノード背景
            rect {
                x: "{x}",
                y: "{y}",
                width: "{NODE_WIDTH}",
                height: "{NODE_HEIGHT}",
                rx: "12",
                fill: "url(#node-gradient)",
                stroke: "{stroke_color}",
                stroke_width: "{stroke_width}",
                filter: "{shadow_filter}",
            }

            // ステータスインジケーター（左サイド）
            rect {
                x: "{x}",
                y: "{y}",
                width: "4",
                height: "{NODE_HEIGHT}",
                rx: "2",
                fill: "{status_color}",
                filter: "{status_glow}",
            }
            // 左上の角丸を隠す
            rect {
                x: "{x}",
                y: "{y + 12.0}",
                width: "4",
                height: "{NODE_HEIGHT - 24.0}",
                fill: "{status_color}",
            }

            // Executorアイコンバッジ
            g {
                // バッジ背景
                rect {
                    x: "{x + 12.0}",
                    y: "{y + 10.0}",
                    width: "28",
                    height: "28",
                    rx: "8",
                    fill: "{executor_color}",
                    opacity: "0.15",
                }
                // アイコン
                text {
                    x: "{x + 26.0}",
                    y: "{y + 30.0}",
                    text_anchor: "middle",
                    font_size: "14",
                    "{executor_icon}"
                }
            }

            // タスク名
            text {
                x: "{x + 48.0}",
                y: "{y + 28.0}",
                fill: "#f1f5f9",
                font_size: "13",
                font_family: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
                font_weight: "600",
                {
                    if name.len() > 14 {
                        format!("{}...", &name[..11])
                    } else {
                        name.clone()
                    }
                }
            }

            // Executorタイプ
            text {
                x: "{x + 48.0}",
                y: "{y + 44.0}",
                fill: "{executor_color}",
                font_size: "10",
                font_family: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
                font_weight: "500",
                {executor.to_uppercase()}
            }

            // タスクID
            text {
                x: "{x + 12.0}",
                y: "{y + 62.0}",
                fill: "#64748b",
                font_size: "9",
                font_family: "monospace",
                "#{task_id}"
            }

            // 入力ポート（上部中央）
            g {
                class: "input-port",
                onmouseup: on_input_mouseup,
                cursor: "crosshair",

                // ヒットエリア
                rect {
                    x: "{x + NODE_WIDTH / 2.0 - 15.0}",
                    y: "{y - 15.0}",
                    width: "30",
                    height: "20",
                    fill: "transparent",
                }
                // ポート外側
                circle {
                    cx: "{x + NODE_WIDTH / 2.0}",
                    cy: "{y}",
                    r: "7",
                    fill: "#0f172a",
                    stroke: "#475569",
                    stroke_width: "2",
                }
                // ポート内側
                circle {
                    cx: "{x + NODE_WIDTH / 2.0}",
                    cy: "{y}",
                    r: "3",
                    fill: "#475569",
                }
            }

            // 出力ポート（下部中央）
            g {
                class: "output-port",
                onmousedown: on_output_mousedown,
                cursor: "crosshair",

                // ヒットエリア
                rect {
                    x: "{x + NODE_WIDTH / 2.0 - 15.0}",
                    y: "{y + NODE_HEIGHT - 5.0}",
                    width: "30",
                    height: "20",
                    fill: "transparent",
                }
                // ポート外側
                circle {
                    cx: "{x + NODE_WIDTH / 2.0}",
                    cy: "{y + NODE_HEIGHT}",
                    r: "7",
                    fill: "#0f172a",
                    stroke: "#818cf8",
                    stroke_width: "2",
                }
                // ポート内側（グラデーション）
                circle {
                    cx: "{x + NODE_WIDTH / 2.0}",
                    cy: "{y + NODE_HEIGHT}",
                    r: "3",
                    fill: "#818cf8",
                }
            }
        }
    }
}
