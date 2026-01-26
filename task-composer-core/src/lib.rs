//! Task Composer Core Library
//!
//! DAGベースのタスク管理ライブラリ

pub mod types;
pub mod dag;
pub mod task_executor;
pub mod path_resolver;
pub mod analysis;
pub mod checkpoint;

// Re-export commonly used types
pub use types::{
    Task, Role, Status, Config, LoopConfig, LoopContext,
    OnErrorMode, ResultFormat, MapContext, ReduceContext,
};
pub use dag::DAG;
pub use task_executor::{ExecutionResult, ExecutionContext, TaskExecutor, ExecutorRegistry};
pub use analysis::{StaticAnalyzer, AnalysisResult, AnalysisLevel, AnalysisItem, DagStructureAnalysis, ConflictDetector};
pub use checkpoint::{Checkpoint, CheckpointState, TaskCheckpoint, CheckpointValidation, compute_dag_hash};
pub use checkpoint::writer::{CheckpointWriter, JsonCheckpointWriter};
