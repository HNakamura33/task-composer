//! コンフリクト検出モジュール
//!
//! DAG内の並行実行可能なタスク間でファイルアクセスの競合を検出します。
//!
//! # 主な機能
//! - 書き込み-書き込み競合の検出
//! - 書き込み-読み取り競合の検出
//! - ディレクトリ境界を考慮したパスのprefix matching
//!
//! # 使用例
//! ```ignore
//! let dag = DAG::from_json(&json)?;
//! let detector = ConflictDetector::new(dag);
//! let conflicts = detector.check_file_conflicts()?;
//! ```

use std::collections::HashSet;
use crate::types::{Task, FileConflict, FileConflictType};
use crate::dag::DAG;

/// 2つのパスが重複するかチェック（ディレクトリ境界を考慮）
///
/// どちらかのパスがもう一方の親ディレクトリである場合、重複とみなす。
/// 単純な prefix マッチではなく、ディレクトリ境界（`/`）を考慮する。
///
/// # Examples
/// - "/src" と "/src/api" → true（親子関係）
/// - "/src" と "/src2" → false（別ディレクトリ）
/// - "/src" と "/test" → false（無関係）
pub fn paths_overlap(path_a: &str, path_b: &str) -> bool {
    if path_a == path_b {
        return true;
    }

    // path_a が path_b の親ディレクトリか
    let a_is_parent = path_b.starts_with(path_a)
        && (path_a.ends_with('/') || path_b[path_a.len()..].starts_with('/'));

    // path_b が path_a の親ディレクトリか
    let b_is_parent = path_a.starts_with(path_b)
        && (path_b.ends_with('/') || path_a[path_b.len()..].starts_with('/'));

    a_is_parent || b_is_parent
}

/// 2つのパス集合から重複するパスペアを見つける
///
/// # Returns
/// 重複するパスのペア (set_a のパス, set_b のパス) のリスト
fn find_overlapping_paths(set_a: &HashSet<String>, set_b: &HashSet<String>) -> Vec<(String, String)> {
    let mut overlaps = Vec::new();
    for path_a in set_a {
        for path_b in set_b {
            if paths_overlap(path_a, path_b) {
                overlaps.push((path_a.clone(), path_b.clone()));
            }
        }
    }
    overlaps
}

/// コンフリクト検出モジュール
//// DAG内のタスク間でのファイルアクセス権限の競合を検出します。
pub struct ConflictDetector {
    /// DAG内のタスク
    pub dag: DAG,
}

impl Default for ConflictDetector {
    fn default() -> Self {
        ConflictDetector {
            dag: DAG::default(),
        }
    }
}

impl ConflictDetector {
    /// 新しいConflictDetectorを作成する
    ///
    /// # Arguments
    /// * `dag` - コンフリクトを検出するDAG
    ///
    /// # Returns
    /// ConflictDetectorインスタンス
    pub fn new(dag: DAG) -> Self {
        ConflictDetector { dag }
    }

    /// タスクの書き込み可能パスを取得する
    ///
    /// 許可されたパスから、拒否されたパスと読み取り専用パスを除外して
    /// 実際に書き込み可能なパスの集合を返します。
    ///
    /// # Arguments
    /// * `task` - 対象のタスク
    ///
    /// # Returns
    /// 書き込み可能なパスの集合（allowed - denied - read_only）
    fn get_writable_paths(task: &Task) -> HashSet<String> {
        let file_perm = &task.role.file_permissions;

        let allowed: HashSet<String> = file_perm.allowed_paths.iter().cloned().collect();

        let denied: HashSet<String> = file_perm.denied_paths.iter().cloned().collect();

        let read_only: HashSet<String> = file_perm.read_only_paths.iter().cloned().collect();

        allowed
            .difference(&denied)
            .cloned()
            .collect::<HashSet<_>>()
            .difference(&read_only)
            .cloned()
            .collect()
    }

    /// タスクの読み取り可能パスを取得する
    ///
    /// 許可されたパスと読み取り専用パスの和集合から、
    /// 拒否されたパスを除外して実際に読み取り可能なパスの集合を返します。
    ///
    /// # Arguments
    /// * `task` - 対象のタスク
    ///
    /// # Returns
    /// 読み取り可能なパスの集合（(allowed ∪ read_only) - denied）
    fn get_readable_paths(task: &Task) -> HashSet<String> {
        let file_perm = &task.role.file_permissions;

        let allowed: HashSet<String> = file_perm.allowed_paths.iter().cloned().collect();

        let denied: HashSet<String> = file_perm.denied_paths.iter().cloned().collect();

        let read_only: HashSet<String> = file_perm.read_only_paths.iter().cloned().collect();

        read_only
            .union(&allowed)
            .cloned()
            .collect::<HashSet<_>>()
            .difference(&denied)
            .cloned()
            .collect()
    }

    /// DAG内のタスク間でファイルアクセスの競合を検出する
    ///
    /// 並行実行可能なタスクペアを取得し、各ペアについて
    /// ファイルパスの重複（prefix matching含む）をチェックします。
    ///
    /// # Returns
    /// * `Ok(Vec<FileConflict>)` - 検出された競合のリスト
    /// * `Err(String)` - DAGに循環が含まれている場合
    ///
    /// # 競合の種類
    /// - `WriteWrite`: 両タスクが同じパスに書き込む
    /// - `WriteRead`: 一方が書き込み、他方が読み取る
    ///
    /// # Example
    /// ```ignore
    /// let detector = ConflictDetector::new(dag);
    /// let conflicts = detector.check_file_conflicts()?;
    /// for conflict in conflicts {
    ///     println!("{} と {} が {} で競合", conflict.task_a, conflict.task_b, conflict.file_path);
    /// }
    /// ```
    pub fn check_file_conflicts(&self) -> Result<Vec<FileConflict>, String> {
        let tasks = &self.dag.nodes;
        let concurrent_pairs = self.dag.get_all_parallel_pairs()?;

        let mut conflicts: Vec<FileConflict> = Vec::new();
        for (task_id_a, task_id_b) in concurrent_pairs {
            let task_a = tasks.get(&task_id_a).unwrap();
            let task_b = tasks.get(&task_id_b).unwrap();

            let writable_a = Self::get_writable_paths(task_a);
            let writable_b = Self::get_writable_paths(task_b);
            let readable_a = Self::get_readable_paths(task_a);
            let readable_b = Self::get_readable_paths(task_b);

            // 書き込み-書き込みの競合をチェック（prefix matching）
            let write_write_overlaps = find_overlapping_paths(&writable_a, &writable_b);
            let mut write_write_paths_a: HashSet<String> = HashSet::new();

            for (path_a, _path_b) in &write_write_overlaps {
                write_write_paths_a.insert(path_a.clone());
                conflicts.push(FileConflict {
                    task_a: task_id_a.clone(),
                    task_b: task_id_b.clone(),
                    file_path: path_a.clone(),
                    conflict_type: FileConflictType::WriteWrite,
                });
            }

            // 書き込み-読み取りの競合をチェック（WriteWriteで報告済みのパスは除外）
            let write_read_overlaps_a = find_overlapping_paths(&writable_a, &readable_b);

            for (path_a, _path_b) in write_read_overlaps_a {
                // WriteWriteで報告済みのパスと重複しない場合のみ追加
                let already_reported = write_write_paths_a.iter()
                    .any(|ww_path| paths_overlap(&path_a, ww_path));
                if !already_reported {
                    conflicts.push(FileConflict {
                        task_a: task_id_a.clone(),
                        task_b: task_id_b.clone(),
                        file_path: path_a.clone(),
                        conflict_type: FileConflictType::WriteRead,
                    });
                }
            }

            let write_read_overlaps_b = find_overlapping_paths(&writable_b, &readable_a);

            for (path_b, _path_a) in write_read_overlaps_b {
                // WriteWriteで報告済みのパスと重複しない場合のみ追加
                let already_reported = write_write_paths_a.iter()
                    .any(|ww_path| paths_overlap(&path_b, ww_path));
                if !already_reported {
                    conflicts.push(FileConflict {
                        task_a: task_id_a.clone(),
                        task_b: task_id_b.clone(),
                        file_path: path_b.clone(),
                        conflict_type: FileConflictType::WriteRead,
                    });
                }
            }
        }

        Ok(conflicts)
    }
}

#[cfg(test)]
mod tests;
