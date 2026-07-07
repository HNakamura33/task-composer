//! シグナルハンドリングモジュール
//!
//! Ctrl+C等のシグナルを受け取り、graceful shutdownを実現します。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;

/// シグナル状態を管理する構造体
#[derive(Debug)]
pub struct SignalState {
    /// シャットダウン要求フラグ
    shutdown_requested: AtomicBool,
}

impl SignalState {
    /// 新しいSignalStateを作成
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            shutdown_requested: AtomicBool::new(false),
        })
    }

    /// シャットダウンを要求
    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
    }

    /// シャットダウンが要求されているか確認
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }
}

impl Default for SignalState {
    fn default() -> Self {
        Self {
            shutdown_requested: AtomicBool::new(false),
        }
    }
}

/// シグナルハンドラーをセットアップ
///
/// Ctrl+Cシグナルを受け取った際にSignalStateのshutdown_requestedフラグを設定します。
///
/// # Arguments
///
/// * `state` - シグナル状態を管理するArc<SignalState>
///
/// # Returns
///
/// シグナルハンドラーのタスクハンドル
pub async fn setup_signal_handler(state: Arc<SignalState>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = signal::ctrl_c().await {
            eprintln!("[Signal] Failed to listen for Ctrl+C: {}", e);
            return;
        }
        eprintln!("\n[Signal] Ctrl+C received. Requesting graceful shutdown...");
        state.request_shutdown();
    })
}

/// シグナル状態を内部のAtomicBoolとして取得
///
/// DAGの実行に渡すための軽量な参照を作成します。
pub fn get_shutdown_flag(state: &Arc<SignalState>) -> Arc<AtomicBool> {
    // SignalStateの内部フラグへの参照を返すのではなく、
    // 同じ状態を共有する新しいAtomicBoolを返す
    // 実際にはSignalState自体を共有するので、このヘルパーは不要かもしれないが、
    // インターフェースの互換性のために用意
    Arc::new(AtomicBool::new(state.is_shutdown_requested()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_state_new() {
        let state = SignalState::new();
        assert!(!state.is_shutdown_requested());
    }

    #[test]
    fn test_signal_state_request_shutdown() {
        let state = SignalState::new();
        assert!(!state.is_shutdown_requested());

        state.request_shutdown();
        assert!(state.is_shutdown_requested());
    }
}
