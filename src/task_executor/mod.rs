pub mod log_executor;

use std::collections::HashMap;
use crate::types::Task;
use crate::types::ExecutionResult;

pub use log_executor::LogExecutor;


pub struct ExecutionContext {
    pub args: serde_json::Value,
    pub env_vars: std::collections::HashMap<String, String>,
}

pub enum ExecutionError {
    TaskNotFound(String),
    ExecutionFailed(String),
    InvalidInput(String),
    Other(String),
}


pub trait TaskExecutor {
    
    fn name(&self) -> &str;
    
    fn execute_task(&self, task: &Task, ctx: &ExecutionContext) -> Result<ExecutionResult, String>;
}

pub struct ExecutorRegistry{
    executors: HashMap<String, Box<dyn TaskExecutor>>,
}

impl ExecutorRegistry {
    pub fn new() -> Self {
        ExecutorRegistry {
            executors: HashMap::new(),
        }
    }

    pub fn register(&mut self, executor: Box<dyn TaskExecutor>) {
        self.executors.insert(executor.name().to_string(), executor);
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn TaskExecutor>> {
        self.executors.get(name)
    }

}

pub struct TaskManager {
    pub queue: Vec<Task>,
    pub registry: ExecutorRegistry,
}

impl TaskManager {
    pub fn new(registry: ExecutorRegistry) -> Self {
        TaskManager {
            queue: Vec::new(),
            registry,
        }
    }

    pub fn add_task(&mut self, task: Task, ctx: ExecutionContext) -> Result<ExecutionResult, String> {
        self.registry.get(&task.executor)
            .ok_or_else(|| format!("Executor not found: {}", task.executor))?
            .execute_task(&task, &ctx)
    }
}