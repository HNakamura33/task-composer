//! エッジ（矢印）コンポーネント - モダンデザイン

use dioxus::prelude::*;

/// DAGエッジ（依存関係の矢印）- n8n風スタイル
#[component]
pub fn Edge(
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    from_id: String,
    to_id: String,
    on_delete: EventHandler<(String, String)>,
) -> Element {
    let mut hovered = use_signal(|| false);

    // スムーズなベジェ曲線の制御点を計算
    let dy = to_y - from_y;
    let dx = (to_x - from_x).abs();

    // 距離に応じて制御点のオフセットを調整
    let ctrl_offset = (dy.abs() / 2.0).max(50.0).min(100.0);
    let horizontal_adjust = dx * 0.1;

    let path = format!(
        "M {} {} C {} {}, {} {}, {} {}",
        from_x, from_y,
        from_x + horizontal_adjust, from_y + ctrl_offset,
        to_x - horizontal_adjust, to_y - ctrl_offset,
        to_x, to_y
    );

    // 中点を計算（削除ボタンの位置）
    let mid_x = (from_x + to_x) / 2.0;
    let mid_y = (from_y + to_y) / 2.0;

    let is_hovered = *hovered.read();
    let stroke_color = if is_hovered { "#ef4444" } else { "url(#edge-gradient)" };
    let stroke_width = if is_hovered { "3" } else { "2" };

    let from_id_clone = from_id.clone();
    let to_id_clone = to_id.clone();

    rsx! {
        g {
            class: "edge",
            onmouseenter: move |_| { hovered.set(true); },
            onmouseleave: move |_| { hovered.set(false); },

            // シャドウレイヤー（深み）
            path {
                d: "{path}",
                fill: "none",
                stroke: "#000",
                stroke_width: "4",
                stroke_linecap: "round",
                opacity: "0.2",
                transform: "translate(1, 2)",
            }

            // メインの曲線（グラデーション or ホバー時赤）
            path {
                d: "{path}",
                fill: "none",
                stroke: "{stroke_color}",
                stroke_width: "{stroke_width}",
                stroke_linecap: "round",
            }

            // クリック用の透明な太いパス（ヒット領域拡大）
            path {
                d: "{path}",
                fill: "none",
                stroke: "transparent",
                stroke_width: "15",
                stroke_linecap: "round",
                style: "cursor: pointer;",
                onclick: move |evt| {
                    evt.stop_propagation();
                    on_delete.call((from_id_clone.clone(), to_id_clone.clone()));
                },
            }

            // 終点の矢印マーカー
            polygon {
                points: "{to_x},{to_y - 2.0} {to_x - 5.0},{to_y - 10.0} {to_x + 5.0},{to_y - 10.0}",
                fill: if is_hovered { "#ef4444" } else { "#818cf8" },
            }

            // ホバー時に削除アイコンを表示
            if is_hovered {
                g {
                    transform: "translate({mid_x}, {mid_y})",
                    // 背景円
                    circle {
                        cx: "0",
                        cy: "0",
                        r: "12",
                        fill: "#ef4444",
                        style: "cursor: pointer;",
                        onclick: move |evt| {
                            evt.stop_propagation();
                            on_delete.call((from_id.clone(), to_id.clone()));
                        },
                    }
                    // X アイコン
                    text {
                        x: "0",
                        y: "1",
                        text_anchor: "middle",
                        dominant_baseline: "middle",
                        fill: "white",
                        font_size: "14",
                        font_weight: "bold",
                        style: "pointer-events: none;",
                        "×"
                    }
                }
            }
        }
    }
}
