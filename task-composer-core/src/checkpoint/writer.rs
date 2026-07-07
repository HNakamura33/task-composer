//! チェックポイント書き込みモジュール
//!
//! チェックポイントファイルの読み書きを担当します。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::Checkpoint;

/// チェックポイントの読み書きを行うトレイト
pub trait CheckpointWriter: Send + Sync {
    /// チェックポイントを保存
    fn save(&self, checkpoint: &Checkpoint) -> Result<(), io::Error>;

    /// チェックポイントを読み込み
    fn load(&self) -> Result<Option<Checkpoint>, io::Error>;

    /// チェックポイントファイルのパスを取得
    fn path(&self) -> &Path;

    /// チェックポイントファイルを削除
    fn delete(&self) -> Result<(), io::Error>;
}

/// JSONファイルベースのチェックポイントライター
///
/// チェックポイントをJSONファイルとして保存します。
/// atomic writeをサポートし、書き込み中のクラッシュに対して安全です。
#[derive(Debug, Clone)]
pub struct JsonCheckpointWriter {
    /// チェックポイントファイルのパス
    path: PathBuf,
}

impl JsonCheckpointWriter {
    /// 新しいJsonCheckpointWriterを作成
    ///
    /// # Arguments
    ///
    /// * `path` - チェックポイントファイルのパス
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// DAGファイルパスからチェックポイントファイルパスを生成
    ///
    /// `dag.json` -> `dag.json.checkpoint.json`
    pub fn from_dag_file(dag_file: impl AsRef<Path>) -> Self {
        let dag_path = dag_file.as_ref();
        let checkpoint_path = PathBuf::from(format!(
            "{}.checkpoint.json",
            dag_path.display()
        ));
        Self::new(checkpoint_path)
    }

    /// atomic writeを使用してファイルを書き込み
    ///
    /// 一時ファイルに書き込んでからリネームすることで、
    /// 書き込み中のクラッシュに対して安全に保存します。
    fn atomic_write(&self, content: &str) -> Result<(), io::Error> {
        let temp_path = self.path.with_extension("tmp");

        // 一時ファイルに書き込み
        fs::write(&temp_path, content)?;

        // リネーム（atomic operation）
        fs::rename(&temp_path, &self.path)?;

        Ok(())
    }
}

impl CheckpointWriter for JsonCheckpointWriter {
    fn save(&self, checkpoint: &Checkpoint) -> Result<(), io::Error> {
        let json = serde_json::to_string_pretty(checkpoint)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        self.atomic_write(&json)
    }

    fn load(&self) -> Result<Option<Checkpoint>, io::Error> {
        if !self.path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.path)?;
        let checkpoint: Checkpoint = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(Some(checkpoint))
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn delete(&self) -> Result<(), io::Error> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::CheckpointState;
    use tempfile::tempdir;

    #[test]
    fn test_json_checkpoint_writer_save_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.checkpoint.json");
        let writer = JsonCheckpointWriter::new(&path);

        let checkpoint = Checkpoint::new("test.json", "sha256:abc123");

        // 保存
        writer.save(&checkpoint).unwrap();
        assert!(path.exists());

        // 読み込み
        let loaded = writer.load().unwrap().unwrap();
        assert_eq!(loaded.dag_file, "test.json");
        assert_eq!(loaded.dag_hash, "sha256:abc123");
    }

    #[test]
    fn test_json_checkpoint_writer_load_nonexistent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.checkpoint.json");
        let writer = JsonCheckpointWriter::new(&path);

        let result = writer.load().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_json_checkpoint_writer_delete() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.checkpoint.json");
        let writer = JsonCheckpointWriter::new(&path);

        // ファイルを作成
        let checkpoint = Checkpoint::new("test.json", "sha256:abc123");
        writer.save(&checkpoint).unwrap();
        assert!(path.exists());

        // 削除
        writer.delete().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_from_dag_file() {
        let writer = JsonCheckpointWriter::from_dag_file("samples/dag.json");
        assert_eq!(
            writer.path().to_str().unwrap(),
            "samples/dag.json.checkpoint.json"
        );
    }

    #[test]
    fn test_checkpoint_state_serialization() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.checkpoint.json");
        let writer = JsonCheckpointWriter::new(&path);

        let mut checkpoint = Checkpoint::new("test.json", "sha256:abc123");
        checkpoint.set_state(CheckpointState::Failed {
            failed_task: "task_1".to_string(),
            error: "Test error".to_string(),
        });

        writer.save(&checkpoint).unwrap();
        let loaded = writer.load().unwrap().unwrap();

        match loaded.state {
            CheckpointState::Failed { failed_task, error } => {
                assert_eq!(failed_task, "task_1");
                assert_eq!(error, "Test error");
            }
            _ => panic!("Expected Failed state"),
        }
    }
}
