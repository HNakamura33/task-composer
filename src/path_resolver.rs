//! パス参照を解決するモジュール
//!
//! `$.{task_id}.output.{field_path}` 形式のパスを解決する
//!
//! ## 構文
//! - `$.{task_id}.output.{field}` - 基本的なフィールドアクセス
//! - `$.{task_id}.output.{field}.{nested}` - ネストしたフィールド
//! - `$.{task_id}.output.{field}[{index}]` - 配列インデックスアクセス
//!
//! ## 例
//! - `$.1.output.user_id` - task "1" の output.user_id
//! - `$.001-101.output.name` - task "001-101" の output.name
//! - `$.1.output.items[0]` - task "1" の output.items の最初の要素

use std::collections::HashMap;
use crate::types::ExecutionResult;

/// パス解決時のエラー
#[derive(Debug, PartialEq)]
pub enum PathResolveError {
    /// タスクが見つからない
    TaskNotFound(String),
    /// フィールドが見つからない
    FieldNotFound(String),
    /// 配列インデックスが範囲外
    IndexOutOfBounds { index: usize, len: usize },
    /// パス構文が不正
    InvalidPathSyntax(String),
}

impl std::fmt::Display for PathResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathResolveError::TaskNotFound(id) => write!(f, "Task not found: {}", id),
            PathResolveError::FieldNotFound(field) => write!(f, "Field not found: {}", field),
            PathResolveError::IndexOutOfBounds { index, len } => {
                write!(f, "Index {} out of bounds (len: {})", index, len)
            }
            PathResolveError::InvalidPathSyntax(msg) => write!(f, "Invalid path syntax: {}", msg),
        }
    }
}

impl std::error::Error for PathResolveError {}

/// inputs内のパス参照を解決する
///
/// `$`で始まる文字列をパス参照として解釈し、対応する値に置換する。
/// オブジェクトや配列は再帰的に処理される。
pub fn resolve_inputs(
    inputs: &serde_json::Value,
    previous_results: &HashMap<String, ExecutionResult>,
) -> Result<serde_json::Value, PathResolveError> {
    match inputs {
        serde_json::Value::String(s) if s.starts_with('$') => {
            resolve_path(s, previous_results)
        }
        serde_json::Value::String(_) => Ok(inputs.clone()),
        serde_json::Value::Object(map) => {
            let mut resolved = serde_json::Map::new();
            for (key, value) in map {
                resolved.insert(key.clone(), resolve_inputs(value, previous_results)?);
            }
            Ok(serde_json::Value::Object(resolved))
        }
        serde_json::Value::Array(arr) => {
            let resolved: Result<Vec<_>, _> = arr
                .iter()
                .map(|v| resolve_inputs(v, previous_results))
                .collect();
            Ok(serde_json::Value::Array(resolved?))
        }
        // Null, Bool, Number はそのまま返す
        _ => Ok(inputs.clone()),
    }
}

/// パス文字列を解決して値を取得する
///
/// パス形式: `$.{task_id}.output.{field_path}`
fn resolve_path(
    path: &str,
    previous_results: &HashMap<String, ExecutionResult>,
) -> Result<serde_json::Value, PathResolveError> {
    // 1. パスが "$." で始まることを確認
    let path = path.strip_prefix("$.").ok_or_else(|| {
        PathResolveError::InvalidPathSyntax(format!("Path must start with '$.' : {}", path))
    })?;

    // 2. ".output." を探してtask_idを抽出
    let output_marker = ".output.";
    let output_pos = path.find(output_marker).ok_or_else(|| {
        PathResolveError::InvalidPathSyntax(format!("Path must contain '.output.' : $.{}", path))
    })?;

    let task_id = &path[..output_pos];
    let field_path = &path[output_pos + output_marker.len()..];

    // 3. previous_resultsからtaskの出力を取得
    let result = previous_results.get(task_id).ok_or_else(|| {
        PathResolveError::TaskNotFound(task_id.to_string())
    })?;

    // 4. フィールドパスに従って値を取得
    get_value_by_field_path(&result.output, field_path)
}

/// フィールドパスに従ってJSON値から値を取得する
///
/// フィールドパス例: "config.host", "items[0].name"
fn get_value_by_field_path(
    value: &serde_json::Value,
    field_path: &str,
) -> Result<serde_json::Value, PathResolveError> {
    if field_path.is_empty() {
        return Ok(value.clone());
    }

    let mut current = value;

    // フィールドパスをセグメントに分割して処理
    for segment in parse_field_path(field_path) {
        current = access_value(current, &segment)?;
    }

    Ok(current.clone())
}

/// フィールドパスをセグメントに分割する
///
/// "config.host" -> ["config", "host"]
/// "items[0].name" -> ["items", "[0]", "name"]
/// "data[1].values[2]" -> ["data", "[1]", "values", "[2]"]
fn parse_field_path(field_path: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();

    let mut chars = field_path.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '.' => {
                if !current.is_empty() {
                    segments.push(current);
                    current = String::new();
                }
            }
            '[' => {
                // '[' の前にフィールド名があればpush
                if !current.is_empty() {
                    segments.push(current);
                    current = String::new();
                }
                // '[' から ']' までを1つのセグメントとして取得
                current.push('[');
                while let Some(&next_ch) = chars.peek() {
                    current.push(chars.next().unwrap());
                    if next_ch == ']' {
                        break;
                    }
                }
                segments.push(current);
                current = String::new();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        segments.push(current);
    }

    segments
}

/// 1つのセグメントに基づいて値にアクセスする
fn access_value<'a>(
    value: &'a serde_json::Value,
    segment: &str,
) -> Result<&'a serde_json::Value, PathResolveError> {
    // 配列インデックスアクセス: "[n]"
    if segment.starts_with('[') && segment.ends_with(']') {
        let index_str = &segment[1..segment.len() - 1];
        let index: usize = index_str.parse().map_err(|_| {
            PathResolveError::InvalidPathSyntax(format!("Invalid array index: {}", segment))
        })?;

        match value {
            serde_json::Value::Array(arr) => {
                arr.get(index).ok_or(PathResolveError::IndexOutOfBounds {
                    index,
                    len: arr.len(),
                })
            }
            _ => Err(PathResolveError::InvalidPathSyntax(format!(
                "Cannot use array index on non-array value: {}",
                segment
            ))),
        }
    } else {
        // オブジェクトフィールドアクセス
        match value {
            serde_json::Value::Object(map) => {
                map.get(segment).ok_or_else(|| {
                    PathResolveError::FieldNotFound(segment.to_string())
                })
            }
            _ => Err(PathResolveError::InvalidPathSyntax(format!(
                "Cannot access field '{}' on non-object value",
                segment
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// テスト用のprevious_resultsを作成
    fn create_test_results() -> HashMap<String, ExecutionResult> {
        let mut results = HashMap::new();

        // Task "1" の結果
        results.insert(
            "1".to_string(),
            ExecutionResult {
                task_id: "1".to_string(),
                success: true,
                output: json!({
                    "user_id": "u123",
                    "config": {
                        "host": "localhost",
                        "port": 8080
                    },
                    "items": ["apple", "banana", "cherry"],
                    "data": [
                        {"name": "first", "value": 1},
                        {"name": "second", "value": 2}
                    ]
                }),
            },
        );

        // Task "001-101" の結果（ハイフン付きID）
        results.insert(
            "001-101".to_string(),
            ExecutionResult {
                task_id: "001-101".to_string(),
                success: true,
                output: json!({
                    "name": "hyphenated-task",
                    "status": "complete"
                }),
            },
        );

        results
    }

    #[test]
    fn test_simple_path() {
        let results = create_test_results();
        let input = json!("$.1.output.user_id");

        let resolved = resolve_inputs(&input, &results).unwrap();

        assert_eq!(resolved, json!("u123"));
    }

    #[test]
    fn test_nested_path() {
        let results = create_test_results();
        let input = json!("$.1.output.config.host");

        let resolved = resolve_inputs(&input, &results).unwrap();

        assert_eq!(resolved, json!("localhost"));
    }

    #[test]
    fn test_normal_string_unchanged() {
        let results = create_test_results();
        let input = json!("hello world");

        let resolved = resolve_inputs(&input, &results).unwrap();

        assert_eq!(resolved, json!("hello world"));
    }

    #[test]
    fn test_object_with_path() {
        let results = create_test_results();
        let input = json!({
            "user": "$.1.output.user_id",
            "host": "$.1.output.config.host",
            "static_value": "unchanged"
        });

        let resolved = resolve_inputs(&input, &results).unwrap();

        assert_eq!(
            resolved,
            json!({
                "user": "u123",
                "host": "localhost",
                "static_value": "unchanged"
            })
        );
    }

    #[test]
    fn test_array_with_path() {
        let results = create_test_results();
        let input = json!(["$.1.output.user_id", "normal", "$.1.output.config.port"]);

        let resolved = resolve_inputs(&input, &results).unwrap();

        assert_eq!(resolved, json!(["u123", "normal", 8080]));
    }

    #[test]
    fn test_task_id_with_hyphen() {
        let results = create_test_results();
        let input = json!("$.001-101.output.name");

        let resolved = resolve_inputs(&input, &results).unwrap();

        assert_eq!(resolved, json!("hyphenated-task"));
    }

    #[test]
    fn test_array_index_access() {
        let results = create_test_results();
        let input = json!("$.1.output.items[0]");

        let resolved = resolve_inputs(&input, &results).unwrap();

        assert_eq!(resolved, json!("apple"));
    }

    #[test]
    fn test_array_index_with_nested_field() {
        let results = create_test_results();
        let input = json!("$.1.output.data[1].name");

        let resolved = resolve_inputs(&input, &results).unwrap();

        assert_eq!(resolved, json!("second"));
    }

    #[test]
    fn test_nonexistent_task_returns_error() {
        let results = create_test_results();
        let input = json!("$.999.output.x");

        let result = resolve_inputs(&input, &results);

        assert!(matches!(result, Err(PathResolveError::TaskNotFound(_))));
    }

    #[test]
    fn test_nonexistent_field_returns_error() {
        let results = create_test_results();
        let input = json!("$.1.output.unknown_field");

        let result = resolve_inputs(&input, &results);

        assert!(matches!(result, Err(PathResolveError::FieldNotFound(_))));
    }

    #[test]
    fn test_index_out_of_bounds_returns_error() {
        let results = create_test_results();
        let input = json!("$.1.output.items[999]");

        let result = resolve_inputs(&input, &results);

        assert!(matches!(result, Err(PathResolveError::IndexOutOfBounds { .. })));
    }

    #[test]
    fn test_invalid_path_syntax() {
        let results = create_test_results();
        // "output" がない不正なパス
        let input = json!("$.1.user_id");

        let result = resolve_inputs(&input, &results);

        assert!(matches!(result, Err(PathResolveError::InvalidPathSyntax(_))));
    }

    #[test]
    fn test_deeply_nested_object() {
        let results = create_test_results();
        let input = json!({
            "level1": {
                "level2": {
                    "value": "$.1.output.user_id"
                }
            }
        });

        let resolved = resolve_inputs(&input, &results).unwrap();

        assert_eq!(
            resolved,
            json!({
                "level1": {
                    "level2": {
                        "value": "u123"
                    }
                }
            })
        );
    }
}
