use super::*;
use crate::types::{Role, FilePermission, ToolPermission};

/// テスト用のTaskを作成するヘルパー関数
fn create_task_with_permissions(
    task_id: &str,
    allowed: Vec<&str>,
    denied: Vec<&str>,
    read_only: Vec<&str>,
) -> Task {
    Task {
        task_id: task_id.to_string(),
        name: Some(format!("Task {}", task_id)),
        description: Some(String::new()),
        priority: 0,
        prompt: Some(String::new()),
        dependencies: vec![],
        executor: String::from("log"),
        role: Role {
            role_id: format!("role_{}", task_id),
            name: String::new(),
            subagents: vec![],
            skills: vec![],
            description: String::new(),
            tool_permissions: ToolPermission::default(),
            file_permissions: FilePermission {
                allowed_paths: allowed.iter().map(|s| s.to_string()).collect(),
                denied_paths: denied.iter().map(|s| s.to_string()).collect(),
                read_only_paths: read_only.iter().map(|s| s.to_string()).collect(),
            },
        },
        args: serde_json::Value::Null,
        if_condition: None,
        else_condition: None,
        timeout_secs: None,
    }
}

#[test]
fn test_get_writable_paths_basic() {
    let task = create_task_with_permissions(
        "1",
        vec!["/src", "/test"],
        vec![],
        vec![],
    );
    let writable = ConflictDetector::get_writable_paths(&task);

    assert!(writable.contains("/src"));
    assert!(writable.contains("/test"));
    assert_eq!(writable.len(), 2);
}

#[test]
fn test_get_writable_paths_with_denied() {
    let task = create_task_with_permissions(
        "1",
        vec!["/src", "/test", "/secrets"],
        vec!["/secrets"],
        vec![],
    );
    let writable = ConflictDetector::get_writable_paths(&task);

    assert!(writable.contains("/src"));
    assert!(writable.contains("/test"));
    assert!(!writable.contains("/secrets"));
    assert_eq!(writable.len(), 2);
}

#[test]
fn test_get_writable_paths_with_read_only() {
    let task = create_task_with_permissions(
        "1",
        vec!["/src", "/vendor"],
        vec![],
        vec!["/vendor"],
    );
    let writable = ConflictDetector::get_writable_paths(&task);

    assert!(writable.contains("/src"));
    assert!(!writable.contains("/vendor"));
    assert_eq!(writable.len(), 1);
}

#[test]
fn test_get_readable_paths_basic() {
    let task = create_task_with_permissions(
        "1",
        vec!["/src"],
        vec![],
        vec!["/vendor"],
    );
    let readable = ConflictDetector::get_readable_paths(&task);

    assert!(readable.contains("/src"));
    assert!(readable.contains("/vendor"));
    assert_eq!(readable.len(), 2);
}

#[test]
fn test_get_readable_paths_with_denied() {
    let task = create_task_with_permissions(
        "1",
        vec!["/src", "/secrets"],
        vec!["/secrets"],
        vec!["/vendor"],
    );
    let readable = ConflictDetector::get_readable_paths(&task);

    assert!(readable.contains("/src"));
    assert!(readable.contains("/vendor"));
    assert!(!readable.contains("/secrets"));
    assert_eq!(readable.len(), 2);
}

#[test]
fn test_check_file_conflicts_write_write() {
    let mut dag = DAG::new();

    let task1 = create_task_with_permissions("1", vec![], vec![], vec![]);
    let task2 = create_task_with_permissions("2", vec!["/src"], vec![], vec![]);
    let task3 = create_task_with_permissions("3", vec!["/src"], vec![], vec![]);

    dag.add_task(task1);
    dag.add_task(task2);
    dag.add_task(task3);
    dag.add_edge("1", "2");
    dag.add_edge("1", "3");

    let detector = ConflictDetector::new(dag);
    let conflicts = detector.check_file_conflicts().unwrap();

    assert_eq!(conflicts.len(), 1);
    assert!(matches!(conflicts[0].conflict_type, FileConflictType::WriteWrite));
    assert_eq!(conflicts[0].file_path, "/src");
}

#[test]
fn test_check_file_conflicts_write_read() {
    let mut dag = DAG::new();

    let task1 = create_task_with_permissions("1", vec![], vec![], vec![]);
    let task2 = create_task_with_permissions("2", vec!["/src"], vec![], vec![]);
    let task3 = create_task_with_permissions("3", vec![], vec![], vec!["/src"]);

    dag.add_task(task1);
    dag.add_task(task2);
    dag.add_task(task3);
    dag.add_edge("1", "2");
    dag.add_edge("1", "3");

    let detector = ConflictDetector::new(dag);
    let conflicts = detector.check_file_conflicts().unwrap();

    assert_eq!(conflicts.len(), 1);
    assert!(matches!(conflicts[0].conflict_type, FileConflictType::WriteRead));
}

#[test]
fn test_check_file_conflicts_no_conflict() {
    let mut dag = DAG::new();

    let task1 = create_task_with_permissions("1", vec![], vec![], vec![]);
    let task2 = create_task_with_permissions("2", vec!["/src"], vec![], vec![]);
    let task3 = create_task_with_permissions("3", vec!["/test"], vec![], vec![]);

    dag.add_task(task1);
    dag.add_task(task2);
    dag.add_task(task3);
    dag.add_edge("1", "2");
    dag.add_edge("1", "3");

    let detector = ConflictDetector::new(dag);
    let conflicts = detector.check_file_conflicts().unwrap();

    assert_eq!(conflicts.len(), 0);
}

#[test]
fn test_check_file_conflicts_dependent_tasks_no_conflict() {
    let mut dag = DAG::new();

    let task1 = create_task_with_permissions("1", vec![], vec![], vec![]);
    let task2 = create_task_with_permissions("2", vec!["/src"], vec![], vec![]);
    let task3 = create_task_with_permissions("3", vec!["/src"], vec![], vec![]);

    dag.add_task(task1);
    dag.add_task(task2);
    dag.add_task(task3);
    dag.add_edge("1", "2");
    dag.add_edge("2", "3");

    let detector = ConflictDetector::new(dag);
    let conflicts = detector.check_file_conflicts().unwrap();

    assert_eq!(conflicts.len(), 0);
}

// prefix matching テスト
#[test]
fn test_paths_overlap() {
    // 完全一致
    assert!(paths_overlap("/src", "/src"));

    // 親子関係（重複あり）
    assert!(paths_overlap("/src", "/src/api"));
    assert!(paths_overlap("/src/api", "/src"));
    assert!(paths_overlap("/src", "/src/api/v1"));
    assert!(paths_overlap("/src/", "/src/api"));  // 末尾スラッシュ

    // 別ディレクトリ（重複なし）- ディレクトリ境界を考慮
    assert!(!paths_overlap("/src", "/src2"));      // 重要: /src は /src2 の親ではない
    assert!(!paths_overlap("/src", "/srcfoo"));
    assert!(!paths_overlap("/src/api", "/src/api2"));

    // 無関係なパス（重複なし）
    assert!(!paths_overlap("/src", "/test"));
    assert!(!paths_overlap("/foo", "/bar"));
}

#[test]
fn test_find_overlapping_paths() {
    let set_a: HashSet<String> = vec!["/src".to_string(), "/test".to_string()]
        .into_iter().collect();
    let set_b: HashSet<String> = vec!["/src/api".to_string(), "/other".to_string()]
        .into_iter().collect();

    let overlaps = find_overlapping_paths(&set_a, &set_b);

    assert_eq!(overlaps.len(), 1);
    assert_eq!(overlaps[0], ("/src".to_string(), "/src/api".to_string()));
}

#[test]
fn test_check_file_conflicts_prefix_write_write() {
    // Task 2: /src に書き込み
    // Task 3: /src/api に書き込み → prefix 競合
    let mut dag = DAG::new();

    let task1 = create_task_with_permissions("1", vec![], vec![], vec![]);
    let task2 = create_task_with_permissions("2", vec!["/src"], vec![], vec![]);
    let task3 = create_task_with_permissions("3", vec!["/src/api"], vec![], vec![]);

    dag.add_task(task1);
    dag.add_task(task2);
    dag.add_task(task3);
    dag.add_edge("1", "2");
    dag.add_edge("1", "3");

    let detector = ConflictDetector::new(dag);
    let conflicts = detector.check_file_conflicts().unwrap();

    assert_eq!(conflicts.len(), 1);
    assert!(matches!(conflicts[0].conflict_type, FileConflictType::WriteWrite));
}

#[test]
fn test_check_file_conflicts_prefix_write_read() {
    // Task 2: /src に書き込み
    // Task 3: /src/api を読み取り → prefix 競合
    let mut dag = DAG::new();

    let task1 = create_task_with_permissions("1", vec![], vec![], vec![]);
    let task2 = create_task_with_permissions("2", vec!["/src"], vec![], vec![]);
    let task3 = create_task_with_permissions("3", vec![], vec![], vec!["/src/api"]);

    dag.add_task(task1);
    dag.add_task(task2);
    dag.add_task(task3);
    dag.add_edge("1", "2");
    dag.add_edge("1", "3");

    let detector = ConflictDetector::new(dag);
    let conflicts = detector.check_file_conflicts().unwrap();

    assert_eq!(conflicts.len(), 1);
    assert!(matches!(conflicts[0].conflict_type, FileConflictType::WriteRead));
}

#[test]
fn test_check_file_conflicts_no_prefix_overlap() {
    // Task 2: /src に書き込み
    // Task 3: /test に書き込み → 競合なし
    let mut dag = DAG::new();

    let task1 = create_task_with_permissions("1", vec![], vec![], vec![]);
    let task2 = create_task_with_permissions("2", vec!["/src"], vec![], vec![]);
    let task3 = create_task_with_permissions("3", vec!["/test"], vec![], vec![]);

    dag.add_task(task1);
    dag.add_task(task2);
    dag.add_task(task3);
    dag.add_edge("1", "2");
    dag.add_edge("1", "3");

    let detector = ConflictDetector::new(dag);
    let conflicts = detector.check_file_conflicts().unwrap();

    assert_eq!(conflicts.len(), 0);
}
