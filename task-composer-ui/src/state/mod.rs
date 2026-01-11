//! 状態管理モジュール

pub mod dag_state;
pub mod ui_state;
pub mod analysis_state;

pub use dag_state::*;
pub use ui_state::{UiState, ViewMode, DragState, PortType};
pub use analysis_state::*;

use dioxus::prelude::*;

/// アプリケーション全体の状態
#[derive(Clone)]
pub struct AppState {
    pub dag: Signal<DagState>,
    pub ui: Signal<UiState>,
    pub analysis: Signal<AnalysisState>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            dag: Signal::new(DagState::default()),
            ui: Signal::new(UiState::new()),
            analysis: Signal::new(AnalysisState::new()),
        }
    }
}

/// AppStateへのアクセスを提供するコンテキストフック
pub fn use_app_state() -> AppState {
    use_context::<AppState>()
}
