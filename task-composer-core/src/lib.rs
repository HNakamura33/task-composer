//! Task Composer Core Library
//!
//! DAGベースのタスク管理ライブラリ

pub mod types;
pub mod dag;
pub mod task_executor;
pub mod path_resolver;
pub mod analysis;

// Re-export commonly used types
pub use types::{
    Task, Role, Status, Config, LoopConfig, LoopContext,
    OnErrorMode, ResultFormat, MapContext, ReduceContext,
};
pub use dag::DAG;
pub use task_executor::{ExecutionResult, ExecutionContext, TaskExecutor, ExecutorRegistry};
pub use analysis::{StaticAnalyzer, AnalysisResult, AnalysisLevel, AnalysisItem, DagStructureAnalysis, ConflictDetector};
