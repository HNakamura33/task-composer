//! パス参照を解決するモジュール
//!
//! `$.{task_id}.output.{field_path}` 形式のパスを解決する
//!
//! ## 構文
//! - `$.{task_id}.output.{field}` - 基本的なフィールドアクセス
//! - `$.{task_id}.output.{field}.{nested}` - ネストしたフィールド
//! - `$.{task_id}.output.{field}[{index}]` - 配列インデックスアクセス
//! - `$.self.{field}` - 自身のタスクのフィールドを参照
//!
//! ## 埋め込み参照
//! 文字列内に `${...}` 形式で参照を埋め込むことができる
//! - `"Hello ${$.1.output.name}"` - 依存タスクの出力を埋め込み
//! - `"Prompt: ${$.self.prompt}"` - 自身のフィールドを埋め込み
//!
//! ## 例
//! - `$.1.output.user_id` - task "1" の output.user_id
//! - `$.001-101.output.name` - task "001-101" の output.name
//! - `$.1.output.items[0]` - task "1" の output.items の最初の要素
//! - `$.self.prompt` - 現在のタスクの prompt フィールド

use std::collections::HashMap;
use regex::Regex;
use crate::task_executor::{ExecutionResult, ExecutionStatus};
use crate::types::Task;

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
    /// $.self参照でフィールドが見つからない
    SelfFieldNotFound(String),
    /// $.self参照にcurrent_taskが必要
    SelfReferenceWithoutContext,
}

/// パス解決のコンテキスト
///
/// `$.self` 参照を解決するために現在のタスク情報を保持する
pub struct ResolveContext<'a> {
    /// 依存タスクの実行結果
    pub previous_results: &'a HashMap<String, ExecutionResult>,
    /// 現在実行中のタスク（$.self参照用）
    pub current_task: Option<&'a Task>,
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
            PathResolveError::SelfFieldNotFound(field) => {
                write!(f, "Self field not found: {}", field)
            }
            PathResolveError::SelfReferenceWithoutContext => {
                write!(f, "$.self reference requires current_task context")
            }
        }
    }
}

impl std::error::Error for PathResolveError {}

/// inputs内のパス参照を解決する
///
/// `$`で始まる文字列をパス参照として解釈し、対応する値に置換する。
/// また、`${...}` 形式の埋め込み参照も解決する。
/// オブジェクトや配列は再帰的に処理される。
///
/// # 引数
/// - `inputs`: 解決対象の値
/// - `ctx`: 解決コンテキスト（依存タスクの結果と現在のタスク情報）
pub fn resolve_inputs(
    inputs: &serde_json::Value,
    ctx: &ResolveContext,
) -> Result<serde_json::Value, PathResolveError> {
    match inputs {
        serde_json::Value::String(s) => resolve_string_value(s, ctx),
        serde_json::Value::Object(map) => {
            let mut resolved = serde_json::Map::new();
            // サブDAG定義内のパス参照は解決しない（サブDAG実行時に解決される）
            const SKIP_KEYS: &[&str] = &["dag"];
            for (key, value) in map {
                let resolved_value = if SKIP_KEYS.contains(&key.as_str()) {
                    value.clone()
                } else {
                    resolve_inputs(value, ctx)?
                };
                resolved.insert(key.clone(), resolved_value);
            }
            Ok(serde_json::Value::Object(resolved))
        }
        serde_json::Value::Array(arr) => {
            let resolved: Result<Vec<_>, _> = arr
                .iter()
                .map(|v| resolve_inputs(v, ctx))
                .collect();
            Ok(serde_json::Value::Array(resolved?))
        }
        // Null, Bool, Number はそのまま返す
        _ => Ok(inputs.clone()),
    }
}

/// 文字列値を解決する
///
/// 以下のパターンを処理:
/// 1. `$` で始まる文字列 → 全体をパス参照として解決
/// 2. `${...}` を含む文字列 → 埋め込み参照を置換
/// 3. その他 → そのまま返す
fn resolve_string_value(
    s: &str,
    ctx: &ResolveContext,
) -> Result<serde_json::Value, PathResolveError> {
    // パターン1: 全体がパス参照（$で始まり ${} でない）
    if s.starts_with("$.") {
        return resolve_path(s, ctx);
    }

    // パターン2: 埋め込み参照を含む
    if s.contains("${") {
        return resolve_embedded_references(s, ctx);
    }

    // パターン3: 通常の文字列
    Ok(serde_json::Value::String(s.to_string()))
}

/// 文字列内の `${...}` 埋め込み参照を解決する
///
/// # 例
/// - `"Hello ${$.1.output.name}"` → `"Hello John"`
/// - `"Count: ${$.2.output.count}"` → `"Count: 42"`
fn resolve_embedded_references(
    s: &str,
    ctx: &ResolveContext,
) -> Result<serde_json::Value, PathResolveError> {
    // ${...} パターンにマッチする正規表現
    // [^}]+ は } 以外の1文字以上にマッチ
    let re = Regex::new(r"\$\{([^}]+)\}").unwrap();

    let mut result = s.to_string();

    // 全てのマッチを処理
    for cap in re.captures_iter(s) {
        let full_match = cap.get(0).unwrap().as_str(); // "${$.1.output.name}"
        let path = cap.get(1).unwrap().as_str();       // "$.1.output.name"

        // パスを解決
        let resolved_value = resolve_path(path, ctx)?;

        // 値を文字列に変換して置換
        let replacement = value_to_string(&resolved_value);
        result = result.replace(full_match, &replacement);
    }

    Ok(serde_json::Value::String(result))
}

/// JSON値を文字列に変換する
///
/// 埋め込み参照の置換に使用
fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        // オブジェクトや配列はJSON文字列として埋め込む
        _ => value.to_string(),
    }
}

/// パス文字列を解決して値を取得する
///
/// パス形式:
/// - `$.{task_id}.output.{field_path}` - 依存タスクの出力を参照
/// - `$.self.{field}` - 現在のタスクのフィールドを参照
fn resolve_path(
    path: &str,
    ctx: &ResolveContext,
) -> Result<serde_json::Value, PathResolveError> {
    // 1. パスが "$." で始まることを確認
    let path = path.strip_prefix("$.").ok_or_else(|| {
        PathResolveError::InvalidPathSyntax(format!("Path must start with '$.' : {}", path))
    })?;

    // 2. $.self 参照の場合
    if path.starts_with("self.") {
        let field_path = path.strip_prefix("self.").unwrap();
        return resolve_self_reference(field_path, ctx);
    }

    // 3. 依存タスク参照: ".output." を探してtask_idを抽出
    let output_marker = ".output.";
    let output_pos = path.find(output_marker).ok_or_else(|| {
        PathResolveError::InvalidPathSyntax(format!("Path must contain '.output.' : $.{}", path))
    })?;

    let task_id = &path[..output_pos];
    let field_path = &path[output_pos + output_marker.len()..];

    // 4. previous_resultsからtaskの出力を取得
    let result = ctx.previous_results.get(task_id).ok_or_else(|| {
        PathResolveError::TaskNotFound(task_id.to_string())
    })?;

    // 5. フィールドパスに従って値を取得
    get_value_by_field_path(&result.output, field_path)
}

/// $.self 参照を解決する
///
/// 現在のタスクのフィールドを取得する
fn resolve_self_reference(
    field_path: &str,
    ctx: &ResolveContext,
) -> Result<serde_json::Value, PathResolveError> {
    let task = ctx.current_task.ok_or(PathResolveError::SelfReferenceWithoutContext)?;

    // 最初のセグメントを取得
    let (first_field, rest) = match field_path.find('.') {
        Some(pos) => (&field_path[..pos], Some(&field_path[pos + 1..])),
        None => (field_path, None),
    };

    // Taskの各フィールドにアクセス
    let value = match first_field {
        "task_id" => serde_json::Value::String(task.task_id.clone()),
        "name" => serde_json::Value::String(task.name.clone()),
        "description" => serde_json::Value::String(task.description.clone()),
        "priority" => serde_json::Value::Number(task.priority.into()),
        "prompt" => serde_json::Value::String(task.prompt.clone()),
        "executor" => serde_json::Value::String(task.executor.clone()),
        "dependencies" => serde_json::to_value(&task.dependencies).unwrap(),
        "inputs" => task.inputs.clone(),
        "args" => task.args.clone(),
        "role" => serde_json::to_value(&task.role).unwrap(),
        _ => return Err(PathResolveError::SelfFieldNotFound(first_field.to_string())),
    };

    // ネストしたフィールドがある場合は再帰的に取得
    match rest {
        Some(remaining_path) => get_value_by_field_path(&value, remaining_path),
        None => Ok(value),
    }
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

/// 条件式評価のエラー
#[derive(Debug, PartialEq)]
pub enum ConditionError {
    /// パス解決に失敗
    PathResolveError(PathResolveError),
    /// 構文エラー
    SyntaxError(String),
    /// 型エラー（比較できない型）
    TypeError(String),
}

impl std::fmt::Display for ConditionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConditionError::PathResolveError(e) => write!(f, "Path resolve error: {}", e),
            ConditionError::SyntaxError(msg) => write!(f, "Syntax error: {}", msg),
            ConditionError::TypeError(msg) => write!(f, "Type error: {}", msg),
        }
    }
}

impl std::error::Error for ConditionError {}

impl From<PathResolveError> for ConditionError {
    fn from(e: PathResolveError) -> Self {
        ConditionError::PathResolveError(e)
    }
}

/// 条件式を評価する
///
/// # 対応する構文
/// - パス参照: `$.task_id.output.field`
/// - 比較演算: `==`, `!=`, `>`, `<`, `>=`, `<=`
/// - 論理演算: `&&`, `||`
/// - 否定: `!`（前置）
/// - リテラル: `true`, `false`, `"string"`, `123`, `null`
///
/// # 例
/// ```ignore
/// evaluate_condition("$.validate.output.ok == true", &ctx)
/// evaluate_condition("$.router.output.value != \"a\"", &ctx)
/// evaluate_condition("$.task.output.count > 10", &ctx)
/// ```
pub fn evaluate_condition(
    condition: &str,
    ctx: &ResolveContext,
) -> Result<bool, ConditionError> {
    let condition = condition.trim();

    // 空の条件はtrue
    if condition.is_empty() {
        return Ok(true);
    }

    // 否定演算子
    if condition.starts_with('!') {
        let inner = condition[1..].trim();
        // !(...) の場合
        if inner.starts_with('(') && inner.ends_with(')') {
            return Ok(!evaluate_condition(&inner[1..inner.len()-1], ctx)?);
        }
        // !$.path の場合
        if inner.starts_with("$.") {
            let value = resolve_path(inner, ctx)?;
            return Ok(!value_to_bool(&value));
        }
        return Ok(!evaluate_condition(inner, ctx)?);
    }

    // 括弧で囲まれた式
    if condition.starts_with('(') && condition.ends_with(')') {
        return evaluate_condition(&condition[1..condition.len()-1], ctx);
    }

    // 論理演算子（&&と||）を探す - 括弧のネストを考慮
    if let Some((left, op, right)) = split_logical_operator(condition) {
        let left_result = evaluate_condition(left, ctx)?;
        match op {
            "&&" => {
                // 短絡評価
                if !left_result {
                    return Ok(false);
                }
                return evaluate_condition(right, ctx);
            }
            "||" => {
                // 短絡評価
                if left_result {
                    return Ok(true);
                }
                return evaluate_condition(right, ctx);
            }
            _ => unreachable!(),
        }
    }

    // 比較演算子を探す
    if let Some((left, op, right)) = split_comparison_operator(condition) {
        let left_value = parse_value(left.trim(), ctx)?;
        let right_value = parse_value(right.trim(), ctx)?;
        return compare_values(&left_value, op, &right_value);
    }

    // 単独の値（bool評価）
    let value = parse_value(condition, ctx)?;
    Ok(value_to_bool(&value))
}

/// 論理演算子で分割（括弧のネストを考慮）
fn split_logical_operator(s: &str) -> Option<(&str, &str, &str)> {
    let mut depth = 0;
    let bytes = s.as_bytes();

    // ||を先に探す（優先度が低い）
    for i in 0..bytes.len().saturating_sub(1) {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'|' if depth == 0 && bytes.get(i + 1) == Some(&b'|') => {
                return Some((&s[..i], "||", &s[i + 2..]));
            }
            _ => {}
        }
    }

    // &&を探す
    depth = 0;
    for i in 0..bytes.len().saturating_sub(1) {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'&' if depth == 0 && bytes.get(i + 1) == Some(&b'&') => {
                return Some((&s[..i], "&&", &s[i + 2..]));
            }
            _ => {}
        }
    }

    None
}

/// 比較演算子で分割
fn split_comparison_operator(s: &str) -> Option<(&str, &str, &str)> {
    // 2文字の演算子を先にチェック
    for op in &["==", "!=", ">=", "<="] {
        if let Some(pos) = s.find(op) {
            return Some((&s[..pos], op, &s[pos + 2..]));
        }
    }

    // 1文字の演算子
    for op in &[">", "<"] {
        if let Some(pos) = s.find(op) {
            // >= や <= の一部でないことを確認
            let next_char = s.chars().nth(pos + 1);
            if next_char != Some('=') {
                return Some((&s[..pos], op, &s[pos + 1..]));
            }
        }
    }

    None
}

/// 値をパースする（パス参照またはリテラル）
fn parse_value(s: &str, ctx: &ResolveContext) -> Result<serde_json::Value, ConditionError> {
    let s = s.trim();

    // パス参照
    if s.starts_with("$.") {
        return Ok(resolve_path(s, ctx)?);
    }

    // ブールリテラル
    if s == "true" {
        return Ok(serde_json::Value::Bool(true));
    }
    if s == "false" {
        return Ok(serde_json::Value::Bool(false));
    }

    // null
    if s == "null" {
        return Ok(serde_json::Value::Null);
    }

    // 文字列リテラル（"..."）
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len()-1];
        // エスケープシーケンスを処理
        let unescaped = inner.replace("\\\"", "\"").replace("\\\\", "\\");
        return Ok(serde_json::Value::String(unescaped));
    }

    // 数値
    if let Ok(n) = s.parse::<i64>() {
        return Ok(serde_json::json!(n));
    }
    if let Ok(n) = s.parse::<f64>() {
        return Ok(serde_json::json!(n));
    }

    Err(ConditionError::SyntaxError(format!("Cannot parse value: {}", s)))
}

/// 2つの値を比較
fn compare_values(
    left: &serde_json::Value,
    op: &str,
    right: &serde_json::Value,
) -> Result<bool, ConditionError> {
    match op {
        "==" => Ok(values_equal(left, right)),
        "!=" => Ok(!values_equal(left, right)),
        ">" | "<" | ">=" | "<=" => compare_ordered(left, op, right),
        _ => Err(ConditionError::SyntaxError(format!("Unknown operator: {}", op))),
    }
}

/// 値が等しいか比較
fn values_equal(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    match (left, right) {
        (serde_json::Value::Null, serde_json::Value::Null) => true,
        (serde_json::Value::Bool(a), serde_json::Value::Bool(b)) => a == b,
        (serde_json::Value::Number(a), serde_json::Value::Number(b)) => {
            // 数値比較（整数と浮動小数点を適切に処理）
            if let (Some(ai), Some(bi)) = (a.as_i64(), b.as_i64()) {
                ai == bi
            } else if let (Some(af), Some(bf)) = (a.as_f64(), b.as_f64()) {
                (af - bf).abs() < f64::EPSILON
            } else {
                false
            }
        }
        (serde_json::Value::String(a), serde_json::Value::String(b)) => a == b,
        _ => false,
    }
}

/// 順序比較（数値のみ）
fn compare_ordered(
    left: &serde_json::Value,
    op: &str,
    right: &serde_json::Value,
) -> Result<bool, ConditionError> {
    let left_num = value_to_f64(left).ok_or_else(|| {
        ConditionError::TypeError(format!("Cannot compare non-numeric value: {:?}", left))
    })?;
    let right_num = value_to_f64(right).ok_or_else(|| {
        ConditionError::TypeError(format!("Cannot compare non-numeric value: {:?}", right))
    })?;

    Ok(match op {
        ">" => left_num > right_num,
        "<" => left_num < right_num,
        ">=" => left_num >= right_num,
        "<=" => left_num <= right_num,
        _ => unreachable!(),
    })
}

/// 値を数値に変換
fn value_to_f64(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// 値をboolに変換（JavaScript風のtruthy/falsy）
fn value_to_bool(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i != 0
            } else if let Some(f) = n.as_f64() {
                f != 0.0
            } else {
                false
            }
        }
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Array(arr) => !arr.is_empty(),
        serde_json::Value::Object(obj) => !obj.is_empty(),
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
                status: ExecutionStatus::Success,
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
                status: ExecutionStatus::Success,
                output: json!({
                    "name": "hyphenated-task",
                    "status": "complete"
                }),
            },
        );

        results
    }

    /// テスト用のTaskを作成
    fn create_test_task() -> Task {
        use crate::types::{Role, ToolPermission, FilePermission, BashPermission, WritePermission};

        Task {
            task_id: "test-task".to_string(),
            name: "Test Task".to_string(),
            description: "A test task".to_string(),
            priority: 5,
            prompt: "Do something".to_string(),
            executor: "test-executor".to_string(),
            dependencies: vec!["1".to_string(), "2".to_string()],
            args: json!({"key": "value"}),
            role: Role {
                role_id: "test-role".to_string(),
                name: "Test Role".to_string(),
                subagents: vec!["agent1".to_string()],
                skills: vec!["coding".to_string(), "testing".to_string()],
                description: "A test role".to_string(),
                tool_permissions: ToolPermission {
                    bash: BashPermission {
                        allowed_commands: vec!["git".to_string(), "cargo".to_string()],
                        blocked_commands: vec![],
                        require_confirmation: vec![],
                    },
                    write: WritePermission {
                        max_file_size_mb: Some(10),
                        allowed_extensions: vec![".rs".to_string()],
                    },
                },
                file_permissions: FilePermission {
                    allowed_paths: vec!["src/".to_string()],
                    denied_paths: vec![".env".to_string()],
                    read_only_paths: vec![],
                },
            },
            ..Default::default()
        }
    }

    /// コンテキストなしのテスト用ヘルパー
    fn ctx_without_task(results: &HashMap<String, ExecutionResult>) -> ResolveContext {
        ResolveContext {
            previous_results: results,
            current_task: None,
        }
    }

    /// コンテキストありのテスト用ヘルパー
    fn ctx_with_task<'a>(
        results: &'a HashMap<String, ExecutionResult>,
        task: &'a Task,
    ) -> ResolveContext<'a> {
        ResolveContext {
            previous_results: results,
            current_task: Some(task),
        }
    }

    #[test]
    fn test_simple_path() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("$.1.output.user_id");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("u123"));
    }

    #[test]
    fn test_nested_path() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("$.1.output.config.host");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("localhost"));
    }

    #[test]
    fn test_normal_string_unchanged() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("hello world");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("hello world"));
    }

    #[test]
    fn test_object_with_path() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!({
            "user": "$.1.output.user_id",
            "host": "$.1.output.config.host",
            "static_value": "unchanged"
        });

        let resolved = resolve_inputs(&input, &ctx).unwrap();

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
        let ctx = ctx_without_task(&results);
        let input = json!(["$.1.output.user_id", "normal", "$.1.output.config.port"]);

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!(["u123", "normal", 8080]));
    }

    #[test]
    fn test_task_id_with_hyphen() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("$.001-101.output.name");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("hyphenated-task"));
    }

    #[test]
    fn test_array_index_access() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("$.1.output.items[0]");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("apple"));
    }

    #[test]
    fn test_array_index_with_nested_field() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("$.1.output.data[1].name");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("second"));
    }

    #[test]
    fn test_nonexistent_task_returns_error() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("$.999.output.x");

        let result = resolve_inputs(&input, &ctx);

        assert!(matches!(result, Err(PathResolveError::TaskNotFound(_))));
    }

    #[test]
    fn test_nonexistent_field_returns_error() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("$.1.output.unknown_field");

        let result = resolve_inputs(&input, &ctx);

        assert!(matches!(result, Err(PathResolveError::FieldNotFound(_))));
    }

    #[test]
    fn test_index_out_of_bounds_returns_error() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("$.1.output.items[999]");

        let result = resolve_inputs(&input, &ctx);

        assert!(matches!(result, Err(PathResolveError::IndexOutOfBounds { .. })));
    }

    #[test]
    fn test_invalid_path_syntax() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        // "output" がない不正なパス
        let input = json!("$.1.user_id");

        let result = resolve_inputs(&input, &ctx);

        assert!(matches!(result, Err(PathResolveError::InvalidPathSyntax(_))));
    }

    #[test]
    fn test_deeply_nested_object() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!({
            "level1": {
                "level2": {
                    "value": "$.1.output.user_id"
                }
            }
        });

        let resolved = resolve_inputs(&input, &ctx).unwrap();

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

    // ============================================
    // 埋め込み参照 ${...} のテスト
    // ============================================

    #[test]
    fn test_embedded_reference_simple() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("Hello ${$.1.output.user_id}!");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("Hello u123!"));
    }

    #[test]
    fn test_embedded_reference_multiple() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("User: ${$.1.output.user_id}, Host: ${$.1.output.config.host}");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("User: u123, Host: localhost"));
    }

    #[test]
    fn test_embedded_reference_with_number() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("Port is ${$.1.output.config.port}");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("Port is 8080"));
    }

    #[test]
    fn test_embedded_reference_in_object() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!({
            "message": "Welcome ${$.1.output.user_id}!",
            "plain": "no reference here"
        });

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(
            resolved,
            json!({
                "message": "Welcome u123!",
                "plain": "no reference here"
            })
        );
    }

    // ============================================
    // $.self 参照のテスト
    // ============================================

    #[test]
    fn test_self_reference_task_id() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("$.self.task_id");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("test-task"));
    }

    #[test]
    fn test_self_reference_name() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("$.self.name");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("Test Task"));
    }

    #[test]
    fn test_self_reference_prompt() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("$.self.prompt");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("Do something"));
    }

    #[test]
    fn test_self_reference_priority() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("$.self.priority");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!(5));
    }

    #[test]
    fn test_self_reference_nested_args() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("$.self.args.key");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("value"));
    }

    #[test]
    fn test_self_reference_dependencies() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("$.self.dependencies");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!(["1", "2"]));
    }

    #[test]
    fn test_self_reference_embedded() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("Task: ${$.self.name}, Prompt: ${$.self.prompt}");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("Task: Test Task, Prompt: Do something"));
    }

    #[test]
    fn test_self_reference_with_dependency_reference() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("${$.self.name} received ${$.1.output.user_id}");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("Test Task received u123"));
    }

    #[test]
    fn test_self_reference_without_context_returns_error() {
        let results = create_test_results();
        let ctx = ctx_without_task(&results);
        let input = json!("$.self.name");

        let result = resolve_inputs(&input, &ctx);

        assert!(matches!(result, Err(PathResolveError::SelfReferenceWithoutContext)));
    }

    #[test]
    fn test_self_reference_unknown_field_returns_error() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("$.self.unknown_field");

        let result = resolve_inputs(&input, &ctx);

        assert!(matches!(result, Err(PathResolveError::SelfFieldNotFound(_))));
    }

    // ============================================
    // $.self.role 参照のテスト
    // ============================================

    #[test]
    fn test_self_reference_role_name() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("$.self.role.name");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("Test Role"));
    }

    #[test]
    fn test_self_reference_role_skills() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("$.self.role.skills");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!(["coding", "testing"]));
    }

    #[test]
    fn test_self_reference_role_full() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("$.self.role");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        // roleオブジェクト全体が取得できることを確認
        assert_eq!(resolved["name"], json!("Test Role"));
        assert_eq!(resolved["role_id"], json!("test-role"));
        assert_eq!(resolved["skills"], json!(["coding", "testing"]));
    }

    #[test]
    fn test_self_reference_role_nested_permissions() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("$.self.role.tool_permissions.bash.allowed_commands");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!(["git", "cargo"]));
    }

    #[test]
    fn test_self_reference_role_embedded() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!("Role: ${$.self.role.name}, Skills: ${$.self.role.skills}");

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved, json!("Role: Test Role, Skills: [\"coding\",\"testing\"]"));
    }

    #[test]
    fn test_self_reference_role_in_object() {
        let results = create_test_results();
        let task = create_test_task();
        let ctx = ctx_with_task(&results, &task);
        let input = json!({
            "agent_role": "$.self.role.name",
            "agent_skills": "$.self.role.skills",
            "permissions": "$.self.role.tool_permissions"
        });

        let resolved = resolve_inputs(&input, &ctx).unwrap();

        assert_eq!(resolved["agent_role"], json!("Test Role"));
        assert_eq!(resolved["agent_skills"], json!(["coding", "testing"]));
        assert!(resolved["permissions"]["bash"].is_object());
    }

    // ============================================
    // 条件式評価のテスト
    // ============================================

    /// 条件式評価テスト用のresultsを作成
    fn create_condition_test_results() -> HashMap<String, ExecutionResult> {
        let mut results = HashMap::new();

        results.insert(
            "validate".to_string(),
            ExecutionResult {
                task_id: "validate".to_string(),
                status: ExecutionStatus::Success,
                output: json!({
                    "ok": true,
                    "count": 42,
                    "status": "success",
                    "value": "a"
                }),
            },
        );

        results.insert(
            "router".to_string(),
            ExecutionResult {
                task_id: "router".to_string(),
                status: ExecutionStatus::Success,
                output: json!({
                    "value": "b",
                    "count": 0
                }),
            },
        );

        results
    }

    #[test]
    fn test_evaluate_condition_equality_true() {
        let results = create_condition_test_results();
        let ctx = ctx_without_task(&results);

        let result = evaluate_condition("$.validate.output.ok == true", &ctx);
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn test_evaluate_condition_equality_false() {
        let results = create_condition_test_results();
        let ctx = ctx_without_task(&results);

        let result = evaluate_condition("$.validate.output.ok == false", &ctx);
        assert_eq!(result, Ok(false));
    }

    #[test]
    fn test_evaluate_condition_string_equality() {
        let results = create_condition_test_results();
        let ctx = ctx_without_task(&results);

        let result = evaluate_condition("$.validate.output.status == \"success\"", &ctx);
        assert_eq!(result, Ok(true));

        let result = evaluate_condition("$.validate.output.value == \"a\"", &ctx);
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn test_evaluate_condition_number_comparison() {
        let results = create_condition_test_results();
        let ctx = ctx_without_task(&results);

        assert_eq!(evaluate_condition("$.validate.output.count > 10", &ctx), Ok(true));
        assert_eq!(evaluate_condition("$.validate.output.count < 100", &ctx), Ok(true));
        assert_eq!(evaluate_condition("$.validate.output.count >= 42", &ctx), Ok(true));
        assert_eq!(evaluate_condition("$.validate.output.count <= 42", &ctx), Ok(true));
        assert_eq!(evaluate_condition("$.validate.output.count == 42", &ctx), Ok(true));
    }

    #[test]
    fn test_evaluate_condition_not_equal() {
        let results = create_condition_test_results();
        let ctx = ctx_without_task(&results);

        let result = evaluate_condition("$.router.output.value != \"a\"", &ctx);
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn test_evaluate_condition_and() {
        let results = create_condition_test_results();
        let ctx = ctx_without_task(&results);

        let result = evaluate_condition("$.validate.output.ok == true && $.validate.output.count > 10", &ctx);
        assert_eq!(result, Ok(true));

        let result = evaluate_condition("$.validate.output.ok == true && $.validate.output.count < 10", &ctx);
        assert_eq!(result, Ok(false));
    }

    #[test]
    fn test_evaluate_condition_or() {
        let results = create_condition_test_results();
        let ctx = ctx_without_task(&results);

        let result = evaluate_condition("$.validate.output.ok == false || $.validate.output.count > 10", &ctx);
        assert_eq!(result, Ok(true));

        let result = evaluate_condition("$.validate.output.ok == false || $.validate.output.count < 10", &ctx);
        assert_eq!(result, Ok(false));
    }

    #[test]
    fn test_evaluate_condition_negation() {
        let results = create_condition_test_results();
        let ctx = ctx_without_task(&results);

        let result = evaluate_condition("!$.router.output.count", &ctx);
        assert_eq!(result, Ok(true)); // count is 0, which is falsy

        let result = evaluate_condition("!$.validate.output.ok", &ctx);
        assert_eq!(result, Ok(false)); // ok is true, so !true = false
    }

    #[test]
    fn test_evaluate_condition_truthy_value() {
        let results = create_condition_test_results();
        let ctx = ctx_without_task(&results);

        // truthy values
        let result = evaluate_condition("$.validate.output.ok", &ctx);
        assert_eq!(result, Ok(true));

        let result = evaluate_condition("$.validate.output.count", &ctx);
        assert_eq!(result, Ok(true)); // 42 is truthy

        // falsy value
        let result = evaluate_condition("$.router.output.count", &ctx);
        assert_eq!(result, Ok(false)); // 0 is falsy
    }

    #[test]
    fn test_evaluate_condition_empty_is_true() {
        let results = create_condition_test_results();
        let ctx = ctx_without_task(&results);

        let result = evaluate_condition("", &ctx);
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn test_evaluate_condition_complex_expression() {
        let results = create_condition_test_results();
        let ctx = ctx_without_task(&results);

        // (A && B) || C
        let result = evaluate_condition(
            "$.validate.output.ok == true && $.router.output.value == \"b\"",
            &ctx
        );
        assert_eq!(result, Ok(true));
    }
}
