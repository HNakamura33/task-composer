//! グラフキャンバスコンポーネント - モダンデザイン

use dioxus::prelude::*;
use std::collections::HashMap;
use crate::state::{use_app_state, DragState, PortType, UiTask};
use super::node::TaskNode;
use super::edge::Edge;

/// ノードの寸法
pub const NODE_WIDTH: f32 = 180.0;
pub const NODE_HEIGHT: f32 = 76.0;

/// グラフキャンバス
#[component]
pub fn GraphCanvas() -> Element {
    let app_state = use_app_state();
    let mut dag = app_state.dag;
    let mut ui = app_state.ui;

    let selected_id = ui.read().selected_task.clone();
    let zoom = ui.read().zoom;
    let drag_state = ui.read().drag_state.clone();

    // ビューボックス計算
    let positions: HashMap<String, (f32, f32)> = dag.read().tasks
        .iter()
        .filter_map(|(id, task)| task.position.map(|pos| (id.clone(), pos)))
        .collect();
    let (view_box, canvas_width, canvas_height) = calculate_view_box(&positions);

    // エッジデータを事前に収集
    let edge_data: Vec<(String, String, f32, f32, f32, f32)> = {
        let dag_read = dag.read();
        let mut edges = Vec::new();
        for (from_id, to_ids) in dag_read.edges.iter() {
            for to_id in to_ids {
                if let (Some(from_task), Some(to_task)) = (dag_read.tasks.get(from_id), dag_read.tasks.get(to_id)) {
                    if let (Some(from_pos), Some(to_pos)) = (from_task.position, to_task.position) {
                        edges.push((
                            from_id.clone(),
                            to_id.clone(),
                            from_pos.0 + NODE_WIDTH / 2.0,
                            from_pos.1 + NODE_HEIGHT,
                            to_pos.0 + NODE_WIDTH / 2.0,
                            to_pos.1,
                        ));
                    }
                }
            }
        }
        edges
    };

    // ノードデータを事前に収集
    let node_data: Vec<(String, String, String, crate::state::UiStatus, f32, f32, bool)> = {
        let dag_read = dag.read();
        dag_read.tasks.iter().filter_map(|(task_id, task)| {
            let (x, y) = task.position?;
            Some((
                task_id.clone(),
                task.name.clone(),
                task.executor.clone(),
                task.status.clone(),
                x,
                y,
                selected_id.as_ref() == Some(task_id),
            ))
        }).collect()
    };

    // マウス移動ハンドラ
    let on_mouse_move = move |evt: MouseEvent| {
        let drag = ui.read().drag_state.clone();
        match drag {
            DragState::DraggingNode { task_id, offset_x, offset_y } => {
                let coords = evt.element_coordinates();
                let new_x = (coords.x as f32 / zoom) - offset_x;
                let new_y = (coords.y as f32 / zoom) - offset_y;

                // 単一のwriteスコープで二重借用を回避
                let mut dag_write = dag.write();
                if let Some(task) = dag_write.tasks.get_mut(&task_id) {
                    task.position = Some((new_x, new_y));
                }
                dag_write.is_dirty = true;
            }
            DragState::Connecting { from_task_id, from_port, .. } => {
                let coords = evt.element_coordinates();
                ui.write().drag_state = DragState::Connecting {
                    from_task_id,
                    from_port,
                    mouse_x: coords.x as f32 / zoom,
                    mouse_y: coords.y as f32 / zoom,
                };
            }
            DragState::DraggingFromPalette { executor, .. } => {
                let coords = evt.element_coordinates();
                ui.write().drag_state = DragState::DraggingFromPalette {
                    executor,
                    mouse_x: coords.x as f32 / zoom,
                    mouse_y: coords.y as f32 / zoom,
                };
            }
            DragState::None => {}
        }
    };

    // マウスアップハンドラ
    let on_mouse_up = move |_evt: MouseEvent| {
        let drag = ui.read().drag_state.clone();
        if let DragState::DraggingFromPalette { executor, mouse_x, mouse_y } = drag {
            let new_id = generate_new_task_id(&dag.read().tasks);
            let new_task = crate::state::UiTask {
                task_id: new_id.clone(),
                name: format!("New {} Task", executor),
                description: String::new(),
                priority: 1,
                status: crate::state::UiStatus::Pending,
                prompt: String::new(),
                executor: executor.clone(),
                dependencies: vec![],
                role: task_composer_core::types::Role::default(),
                inputs: serde_json::Value::Null,
                args: serde_json::Value::Null,
                position: Some((mouse_x - NODE_WIDTH / 2.0, mouse_y - NODE_HEIGHT / 2.0)),
            };
            // 単一のwriteスコープで複数操作をまとめる
            {
                let mut dag_write = dag.write();
                dag_write.tasks.insert(new_id.clone(), new_task);
                dag_write.edges.insert(new_id.clone(), vec![]);
                dag_write.is_dirty = true;
            }
            ui.write().select_task(Some(new_id));
        }
        ui.write().end_drag();
    };

    // 接続中の一時線の座標を計算
    let connecting_line = if let DragState::Connecting { ref from_task_id, ref from_port, mouse_x, mouse_y } = drag_state {
        dag.read().tasks.get(from_task_id).and_then(|task| {
            task.position.map(|(x, y)| {
                let (start_x, start_y) = match from_port {
                    PortType::Output => (x + NODE_WIDTH / 2.0, y + NODE_HEIGHT),
                    PortType::Input => (x + NODE_WIDTH / 2.0, y),
                };
                (start_x, start_y, mouse_x, mouse_y)
            })
        })
    } else {
        None
    };

    rsx! {
        div {
            style: "flex: 1; background: #0f0f1a; overflow: hidden; position: relative;",

            // ズームコントロール（モダンスタイル）
            div {
                style: "position: absolute; top: 12px; right: 12px; display: flex; gap: 2px; z-index: 10; background: rgba(15, 23, 42, 0.8); padding: 4px; border-radius: 8px; backdrop-filter: blur(8px); border: 1px solid #1e293b;",

                button {
                    style: "width: 32px; height: 32px; background: transparent; color: #94a3b8; border: none; border-radius: 6px; cursor: pointer; font-size: 16px; display: flex; align-items: center; justify-content: center; transition: all 0.15s;",
                    onclick: move |_| ui.write().zoom_out(),
                    "-"
                }
                div {
                    style: "width: 48px; height: 32px; display: flex; align-items: center; justify-content: center; color: #64748b; font-size: 11px; font-family: monospace;",
                    "{(zoom * 100.0) as i32}%"
                }
                button {
                    style: "width: 32px; height: 32px; background: transparent; color: #94a3b8; border: none; border-radius: 6px; cursor: pointer; font-size: 16px; display: flex; align-items: center; justify-content: center; transition: all 0.15s;",
                    onclick: move |_| ui.write().zoom_in(),
                    "+"
                }
                div {
                    style: "width: 1px; height: 20px; background: #1e293b; margin: 6px 4px;",
                }
                button {
                    style: "padding: 0 12px; height: 32px; background: transparent; color: #64748b; border: none; border-radius: 6px; cursor: pointer; font-size: 11px; display: flex; align-items: center; justify-content: center; transition: all 0.15s;",
                    onclick: move |_| ui.write().zoom_reset(),
                    "Reset"
                }
            }

            svg {
                width: "{canvas_width * zoom}",
                height: "{canvas_height * zoom}",
                view_box: "{view_box}",
                style: "display: block; cursor: default;",
                onmousemove: on_mouse_move,
                onmouseup: on_mouse_up,
                onmouseleave: move |_| ui.write().end_drag(),

                // SVG定義（グラデーション、フィルター、パターン）
                defs {
                    // ドットグリッドパターン
                    pattern {
                        id: "dot-grid",
                        width: "24",
                        height: "24",
                        pattern_units: "userSpaceOnUse",
                        circle {
                            cx: "12",
                            cy: "12",
                            r: "1",
                            fill: "#1e293b",
                        }
                    }

                    // ノードグラデーション
                    linearGradient {
                        id: "node-gradient",
                        x1: "0%",
                        y1: "0%",
                        x2: "0%",
                        y2: "100%",
                        stop { offset: "0%", stop_color: "#1e293b" }
                        stop { offset: "100%", stop_color: "#0f172a" }
                    }

                    // エッジグラデーション
                    linearGradient {
                        id: "edge-gradient",
                        x1: "0%",
                        y1: "0%",
                        x2: "0%",
                        y2: "100%",
                        stop { offset: "0%", stop_color: "#475569" }
                        stop { offset: "100%", stop_color: "#818cf8" }
                    }

                    // ノーマルシャドウ
                    filter {
                        id: "shadow-normal",
                        x: "-20%",
                        y: "-20%",
                        width: "140%",
                        height: "140%",
                        feDropShadow {
                            dx: "0",
                            dy: "4",
                            std_deviation: "8",
                            flood_color: "#000",
                            flood_opacity: "0.3",
                        }
                    }

                    // 選択時シャドウ
                    filter {
                        id: "shadow-selected",
                        x: "-30%",
                        y: "-30%",
                        width: "160%",
                        height: "160%",
                        feDropShadow {
                            dx: "0",
                            dy: "4",
                            std_deviation: "12",
                            flood_color: "#818cf8",
                            flood_opacity: "0.4",
                        }
                    }

                    // ステータスグロー（青）
                    filter {
                        id: "glow-blue",
                        x: "-100%",
                        y: "-100%",
                        width: "300%",
                        height: "300%",
                        feGaussianBlur {
                            std_deviation: "4",
                        }
                    }

                    // ステータスグロー（緑）
                    filter {
                        id: "glow-green",
                        x: "-100%",
                        y: "-100%",
                        width: "300%",
                        height: "300%",
                        feGaussianBlur {
                            std_deviation: "4",
                        }
                    }

                    // ステータスグロー（赤）
                    filter {
                        id: "glow-red",
                        x: "-100%",
                        y: "-100%",
                        width: "300%",
                        height: "300%",
                        feGaussianBlur {
                            std_deviation: "4",
                        }
                    }
                }

                // 背景
                rect {
                    width: "100%",
                    height: "100%",
                    fill: "#0a0f1a",
                }
                rect {
                    width: "100%",
                    height: "100%",
                    fill: "url(#dot-grid)",
                }

                // エッジを描画
                g { class: "edges",
                    for (idx, (from_id, to_id, from_x, from_y, to_x, to_y)) in edge_data.iter().enumerate() {
                        Edge {
                            key: "{idx}-{from_id}-{to_id}",
                            from_x: *from_x,
                            from_y: *from_y,
                            to_x: *to_x,
                            to_y: *to_y,
                            from_id: from_id.clone(),
                            to_id: to_id.clone(),
                            on_delete: move |(from, to): (String, String)| {
                                dag.write().remove_edge(&from, &to);
                            },
                        }
                    }
                }

                // 接続中の一時線
                if let Some((start_x, start_y, end_x, end_y)) = connecting_line {
                    line {
                        x1: "{start_x}",
                        y1: "{start_y}",
                        x2: "{end_x}",
                        y2: "{end_y}",
                        stroke: "#6366f1",
                        stroke_width: "2",
                        stroke_dasharray: "5,5",
                    }
                }

                // ノードを描画
                g { class: "nodes",
                    for (task_id, name, executor, status, x, y, selected) in node_data.iter() {
                        TaskNode {
                            key: "{task_id}",
                            task_id: task_id.clone(),
                            name: name.clone(),
                            executor: executor.clone(),
                            status: status.clone(),
                            x: *x,
                            y: *y,
                            selected: *selected,
                        }
                    }
                }

                // ドラッグ中のゴーストノード（モダンスタイル）
                if let DragState::DraggingFromPalette { ref executor, mouse_x, mouse_y } = drag_state {
                    g {
                        opacity: "0.8",
                        // シャドウ
                        rect {
                            x: "{mouse_x - NODE_WIDTH / 2.0 + 2.0}",
                            y: "{mouse_y - NODE_HEIGHT / 2.0 + 4.0}",
                            width: "{NODE_WIDTH}",
                            height: "{NODE_HEIGHT}",
                            rx: "12",
                            fill: "#000",
                            opacity: "0.3",
                        }
                        rect {
                            x: "{mouse_x - NODE_WIDTH / 2.0}",
                            y: "{mouse_y - NODE_HEIGHT / 2.0}",
                            width: "{NODE_WIDTH}",
                            height: "{NODE_HEIGHT}",
                            rx: "12",
                            fill: "url(#node-gradient)",
                            stroke: "#818cf8",
                            stroke_width: "2",
                            stroke_dasharray: "8,4",
                        }
                        // アイコン
                        text {
                            x: "{mouse_x - 30.0}",
                            y: "{mouse_y + 5.0}",
                            font_size: "20",
                            {
                                match executor.as_str() {
                                    "log" => "📝",
                                    "mcp" => "🤖",
                                    _ => "⚙",
                                }
                            }
                        }
                        text {
                            x: "{mouse_x + 10.0}",
                            y: "{mouse_y + 5.0}",
                            text_anchor: "start",
                            dominant_baseline: "middle",
                            fill: "#f1f5f9",
                            font_size: "13",
                            font_weight: "600",
                            "{executor}"
                        }
                    }
                }
            }

            // 空の状態（モダンスタイル）
            if dag.read().tasks.is_empty() {
                div {
                    style: "position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); text-align: center;",
                    div {
                        style: "font-size: 48px; margin-bottom: 16px; opacity: 0.3;",
                        "📊"
                    }
                    p {
                        style: "color: #64748b; font-size: 16px; font-weight: 500; margin: 0 0 8px 0;",
                        "No workflow loaded"
                    }
                    p {
                        style: "color: #475569; font-size: 13px; margin: 0;",
                        "Drag nodes from the sidebar or open a JSON file"
                    }
                }
            }
        }
    }
}

/// 新しいタスクIDを生成
fn generate_new_task_id(tasks: &HashMap<String, UiTask>) -> String {
    let mut id = 1;
    loop {
        let candidate = id.to_string();
        if !tasks.contains_key(&candidate) {
            return candidate;
        }
        id += 1;
    }
}

/// ビューボックスを計算
fn calculate_view_box(positions: &HashMap<String, (f32, f32)>) -> (String, f32, f32) {
    if positions.is_empty() {
        return ("0 0 800 600".to_string(), 800.0, 600.0);
    }

    let min_x = positions.values().map(|(x, _)| *x).fold(f32::INFINITY, f32::min) - 50.0;
    let max_x = positions.values().map(|(x, _)| *x).fold(f32::NEG_INFINITY, f32::max) + NODE_WIDTH + 100.0;
    let min_y = positions.values().map(|(_, y)| *y).fold(f32::INFINITY, f32::min) - 50.0;
    let max_y = positions.values().map(|(_, y)| *y).fold(f32::NEG_INFINITY, f32::max) + NODE_HEIGHT + 100.0;

    let width = (max_x - min_x).max(800.0);
    let height = (max_y - min_y).max(600.0);

    (format!("{} {} {} {}", min_x.min(0.0), min_y.min(0.0), width, height), width, height)
}
