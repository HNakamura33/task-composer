//! UI状態管理

/// ビューモード
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ViewMode {
    #[default]
    Graph,
    List,
    Json,
}

/// ドラッグ操作の種類
#[derive(Debug, Clone, PartialEq)]
pub enum DragState {
    /// ドラッグなし
    None,
    /// ノードをドラッグ中
    DraggingNode {
        task_id: String,
        offset_x: f32,
        offset_y: f32,
    },
    /// 接続を作成中
    Connecting {
        from_task_id: String,
        from_port: PortType,
        mouse_x: f32,
        mouse_y: f32,
    },
    /// パレットからノードをドラッグ中
    DraggingFromPalette {
        executor: String,
        mouse_x: f32,
        mouse_y: f32,
    },
}

impl Default for DragState {
    fn default() -> Self {
        DragState::None
    }
}

/// ポートの種類
#[derive(Debug, Clone, PartialEq)]
pub enum PortType {
    Input,
    Output,
}

/// UI全体の状態
#[derive(Debug, Clone, Default)]
pub struct UiState {
    /// 選択中のタスクID
    pub selected_task: Option<String>,
    /// 表示モード
    pub view_mode: ViewMode,
    /// サイドバーが開いているか
    pub sidebar_open: bool,
    /// ズームレベル (1.0 = 100%)
    pub zoom: f32,
    /// パン位置
    pub pan: (f32, f32),
    /// ドラッグ状態
    pub drag_state: DragState,
    /// 編集モード中か
    pub editing: bool,
    /// 解析パネルが開いているか
    pub analysis_panel_open: bool,
}

impl UiState {
    pub fn new() -> Self {
        UiState {
            sidebar_open: true,
            zoom: 1.0,
            pan: (0.0, 0.0),
            ..Default::default()
        }
    }

    /// タスクを選択
    pub fn select_task(&mut self, task_id: Option<String>) {
        self.selected_task = task_id;
    }

    /// ビューモードを切り替え
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }

    /// ズームイン
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.2).min(3.0);
    }

    /// ズームアウト
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.2).max(0.3);
    }

    /// ズームリセット
    pub fn zoom_reset(&mut self) {
        self.zoom = 1.0;
        self.pan = (0.0, 0.0);
    }

    /// ノードのドラッグを開始
    pub fn start_dragging_node(&mut self, task_id: String, offset_x: f32, offset_y: f32) {
        self.drag_state = DragState::DraggingNode {
            task_id,
            offset_x,
            offset_y,
        };
    }

    /// 接続の作成を開始
    pub fn start_connecting(&mut self, task_id: String, port: PortType, x: f32, y: f32) {
        self.drag_state = DragState::Connecting {
            from_task_id: task_id,
            from_port: port,
            mouse_x: x,
            mouse_y: y,
        };
    }

    /// パレットからのドラッグを開始
    pub fn start_dragging_from_palette(&mut self, executor: String, x: f32, y: f32) {
        self.drag_state = DragState::DraggingFromPalette {
            executor,
            mouse_x: x,
            mouse_y: y,
        };
    }

    /// ドラッグを終了
    pub fn end_drag(&mut self) {
        self.drag_state = DragState::None;
    }

    /// ドラッグ中かどうか
    pub fn is_dragging(&self) -> bool {
        !matches!(self.drag_state, DragState::None)
    }

    /// 解析パネルの表示を切り替え
    pub fn toggle_analysis_panel(&mut self) {
        self.analysis_panel_open = !self.analysis_panel_open;
    }
}
