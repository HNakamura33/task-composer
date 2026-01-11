//! タスク定義検証

use std::collections::HashSet;
use crate::dag::DAG;
use crate::types::Task;
use super::{AnalysisItem, AnalysisLevel};

/// 有効なExecutor一覧
const VALID_EXECUTORS: &[&str] = &["log", "mcp"];

/// 全タスクを検証
pub fn validate_all_tasks(dag: &DAG) -> Vec<AnalysisItem> {
    let mut items = Vec::new();

    for (task_id, task) in &dag.nodes {
        items.extend(validate_task(task_id, task, dag));
    }

    items
}

/// 単一タスクを検証
pub fn validate_task(task_id: &str, task: &Task, dag: &DAG) -> Vec<AnalysisItem> {
    let mut items = Vec::new();

    // 必須フィールドチェック
    items.extend(validate_required_fields(task_id, task));

    // Executor検証
    items.extend(validate_executor(task_id, task));

    // 依存関係検証
    items.extend(validate_dependencies(task_id, task, dag));

    // FilePermission検証
    items.extend(validate_file_permissions(task_id, task));

    items
}

/// 必須フィールドの検証
fn validate_required_fields(task_id: &str, task: &Task) -> Vec<AnalysisItem> {
    let mut items = Vec::new();

    // prompt が空（警告）- Optional なので未設定または空文字列の場合に警告
    let prompt_is_empty = task.prompt.as_ref()
        .map(|p| p.trim().is_empty())
        .unwrap_or(true);

    if prompt_is_empty {
        items.push(AnalysisItem {
            level: AnalysisLevel::Warning,
            category: "推奨フィールド".to_string(),
            message: format!("タスク {} の prompt が空です", task_id),
            related_tasks: vec![task_id.to_string()],
        });
    }

    items
}

/// Executorの検証
fn validate_executor(task_id: &str, task: &Task) -> Vec<AnalysisItem> {
    let mut items = Vec::new();

    if !VALID_EXECUTORS.contains(&task.executor.as_str()) {
        items.push(AnalysisItem {
            level: AnalysisLevel::Warning,
            category: "Executor".to_string(),
            message: format!(
                "タスク {} の executor '{}' は未知です（有効: {}）",
                task_id, task.executor, VALID_EXECUTORS.join(", ")
            ),
            related_tasks: vec![task_id.to_string()],
        });
    }

    items
}

/// 依存関係の検証
fn validate_dependencies(task_id: &str, task: &Task, dag: &DAG) -> Vec<AnalysisItem> {
    let mut items = Vec::new();

    // 存在しない依存先
    for dep in &task.dependencies {
        if !dag.nodes.contains_key(dep) {
            items.push(AnalysisItem {
                level: AnalysisLevel::Error,
                category: "依存関係".to_string(),
                message: format!(
                    "タスク {} が存在しない依存先 '{}' を参照しています",
                    task_id, dep
                ),
                related_tasks: vec![task_id.to_string(), dep.clone()],
            });
        }
    }

    // 自己依存
    if task.dependencies.contains(&task_id.to_string()) {
        items.push(AnalysisItem {
            level: AnalysisLevel::Error,
            category: "依存関係".to_string(),
            message: format!("タスク {} が自分自身に依存しています", task_id),
            related_tasks: vec![task_id.to_string()],
        });
    }

    // 重複依存
    let mut seen = HashSet::new();
    for dep in &task.dependencies {
        if !seen.insert(dep.clone()) {
            items.push(AnalysisItem {
                level: AnalysisLevel::Warning,
                category: "依存関係".to_string(),
                message: format!(
                    "タスク {} に重複した依存先 '{}' があります",
                    task_id, dep
                ),
                related_tasks: vec![task_id.to_string()],
            });
        }
    }

    items
}

/// FilePermissionの検証
fn validate_file_permissions(task_id: &str, task: &Task) -> Vec<AnalysisItem> {
    let mut items = Vec::new();
    let perms = &task.role.file_permissions;

    // allowed_paths と denied_paths のコンフリクト
    for allowed in &perms.allowed_paths {
        if perms.denied_paths.contains(allowed) {
            items.push(AnalysisItem {
                level: AnalysisLevel::Error,
                category: "FilePermission".to_string(),
                message: format!(
                    "タスク {} のパス '{}' が allowed_paths と denied_paths の両方に存在します",
                    task_id, allowed
                ),
                related_tasks: vec![task_id.to_string()],
            });
        }
    }

    // allowed_paths と read_only_paths のコンフリクト
    for allowed in &perms.allowed_paths {
        if perms.read_only_paths.contains(allowed) {
            items.push(AnalysisItem {
                level: AnalysisLevel::Warning,
                category: "FilePermission".to_string(),
                message: format!(
                    "タスク {} のパス '{}' が allowed_paths と read_only_paths の両方に存在します",
                    task_id, allowed
                ),
                related_tasks: vec![task_id.to_string()],
            });
        }
    }

    // denied_paths と read_only_paths のコンフリクト
    for denied in &perms.denied_paths {
        if perms.read_only_paths.contains(denied) {
            items.push(AnalysisItem {
                level: AnalysisLevel::Warning,
                category: "FilePermission".to_string(),
                message: format!(
                    "タスク {} のパス '{}' が denied_paths と read_only_paths の両方に存在します",
                    task_id, denied
                ),
                related_tasks: vec![task_id.to_string()],
            });
        }
    }

    // 階層的なパスのコンフリクト
    for allowed in &perms.allowed_paths {
        for denied in &perms.denied_paths {
            if allowed != denied && paths_overlap(allowed, denied) {
                items.push(AnalysisItem {
                    level: AnalysisLevel::Warning,
                    category: "FilePermission".to_string(),
                    message: format!(
                        "タスク {} のパス '{}' と '{}' が階層的にオーバーラップしています",
                        task_id, allowed, denied
                    ),
                    related_tasks: vec![task_id.to_string()],
                });
            }
        }
    }

    items
}

/// パスが階層的にオーバーラップするか判定
fn paths_overlap(path_a: &str, path_b: &str) -> bool {
    let a = path_a.trim_end_matches('/');
    let b = path_b.trim_end_matches('/');

    if a == b {
        return true;
    }

    // ディレクトリ境界を考慮したprefix判定
    let a_prefix = format!("{}/", a);
    let b_prefix = format!("{}/", b);

    b.starts_with(&a_prefix) || a.starts_with(&b_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Role, FilePermission};

    fn create_test_task(task_id: &str) -> Task {
        Task {
            task_id: task_id.to_string(),
            name: Some("Test Task".to_string()),
            description: Some("".to_string()),
            priority: 1,
            prompt: Some("Test prompt".to_string()),
            executor: "log".to_string(),
            dependencies: vec![],
            role: Default::default(),
            args: serde_json::Value::Null,
            if_condition: None,
            else_condition: None,
            timeout_secs: None,
        }
    }

    #[test]
    fn test_validate_empty_name() {
        let mut task = create_test_task("1");
        task.name = Some("".to_string());

        let dag = DAG::new();
        let items = validate_task("1", &task, &dag);

        // name is optional, so empty name should not cause error
        // This test verifies that empty name does not cause a crash
        assert!(items.iter().all(|i| !i.message.contains("name が空")));
    }

    #[test]
    fn test_validate_empty_prompt() {
        let mut task = create_test_task("1");
        task.prompt = Some("".to_string());

        let dag = DAG::new();
        let items = validate_task("1", &task, &dag);

        assert!(items.iter().any(|i| i.level == AnalysisLevel::Warning && i.message.contains("prompt")));
    }

    #[test]
    fn test_validate_unknown_executor() {
        let mut task = create_test_task("1");
        task.executor = "unknown".to_string();

        let dag = DAG::new();
        let items = validate_task("1", &task, &dag);

        assert!(items.iter().any(|i| i.level == AnalysisLevel::Warning && i.message.contains("executor")));
    }

    #[test]
    fn test_validate_self_dependency() {
        let mut task = create_test_task("1");
        task.dependencies = vec!["1".to_string()];

        let mut dag = DAG::new();
        dag.add_task(task.clone());

        let items = validate_task("1", &task, &dag);

        assert!(items.iter().any(|i| i.level == AnalysisLevel::Error && i.message.contains("自分自身")));
    }

    #[test]
    fn test_validate_duplicate_dependency() {
        let mut task = create_test_task("2");
        task.dependencies = vec!["1".to_string(), "1".to_string()];

        let mut dag = DAG::new();
        dag.add_task(create_test_task("1"));
        dag.add_task(task.clone());

        let items = validate_task("2", &task, &dag);

        assert!(items.iter().any(|i| i.level == AnalysisLevel::Warning && i.message.contains("重複")));
    }

    #[test]
    fn test_validate_file_permission_conflict() {
        let mut task = create_test_task("1");
        task.role.file_permissions = FilePermission {
            allowed_paths: vec!["/src".to_string()],
            denied_paths: vec!["/src".to_string()],
            read_only_paths: vec![],
        };

        let dag = DAG::new();
        let items = validate_task("1", &task, &dag);

        assert!(items.iter().any(|i|
            i.level == AnalysisLevel::Error &&
            i.category == "FilePermission"
        ));
    }

    #[test]
    fn test_paths_overlap() {
        assert!(paths_overlap("/src", "/src"));
        assert!(paths_overlap("/src", "/src/api"));
        assert!(paths_overlap("/src/api", "/src"));
        assert!(!paths_overlap("/src", "/src2"));
        assert!(!paths_overlap("/src", "/srcfoo"));
    }
}
