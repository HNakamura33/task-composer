//! サイドバーコンポーネント - モダンデザイン

mod task_list;
mod task_detail;
mod node_palette;

pub use task_list::TaskList;
pub use task_detail::TaskDetail;
pub use node_palette::NodePalette;

use dioxus::prelude::*;
use crate::state::use_app_state;

/// サイドバーコンポーネント（n8n/Dify風）
#[component]
pub fn Sidebar() -> Element {
    let app_state = use_app_state();
    let ui = app_state.ui.read();

    if !ui.sidebar_open {
        return rsx! {};
    }

    rsx! {
        aside {
            style: "width: 260px; background: linear-gradient(180deg, #0f172a 0%, #0a0f1a 100%); border-right: 1px solid #1e293b; display: flex; flex-direction: column; overflow: hidden;",

            // ノードパレット
            div {
                style: "border-bottom: 1px solid #1e293b;",
                NodePalette {}
            }

            // タスク一覧
            div {
                style: "flex: 1; overflow-y: auto; padding: 12px;",
                TaskList {}
            }
        }
    }
}
