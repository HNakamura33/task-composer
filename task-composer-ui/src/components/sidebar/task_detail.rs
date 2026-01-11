//! タスク詳細・編集コンポーネント

use dioxus::prelude::*;
use crate::state::{use_app_state, UiStatus};
use task_composer_core::types::{Role, ToolPermission, BashPermission, WritePermission, FilePermission};

/// タスク詳細・編集パネル（全パラメータ対応）
#[component]
pub fn TaskDetail(task_id: String) -> Element {
    let app_state = use_app_state();
    let mut dag = app_state.dag;
    let mut ui = app_state.ui;

    let task = dag.read().tasks.get(&task_id).cloned();
    let Some(task) = task else {
        return rsx! {
            div {
                style: "padding: 16px; color: #666;",
                "Task not found"
            }
        };
    };

    // 基本フィールド
    let mut name = use_signal(|| task.name.clone());
    let mut description = use_signal(|| task.description.clone());
    let mut executor = use_signal(|| task.executor.clone());
    let mut prompt = use_signal(|| task.prompt.clone());
    let mut priority = use_signal(|| task.priority.to_string());
    let mut status = use_signal(|| format!("{:?}", task.status));
    let mut inputs_json = use_signal(|| serde_json::to_string_pretty(&task.inputs).unwrap_or_default());
    let mut args_json = use_signal(|| serde_json::to_string_pretty(&task.args).unwrap_or_default());

    // Roleフィールド
    let mut role_id = use_signal(|| task.role.role_id.clone());
    let mut role_name = use_signal(|| task.role.name.clone());
    let mut role_description = use_signal(|| task.role.description.clone());
    let mut subagents = use_signal(|| task.role.subagents.join(", "));
    let mut skills = use_signal(|| task.role.skills.join(", "));

    // BashPermission
    let mut bash_allowed = use_signal(|| task.role.tool_permissions.bash.allowed_commands.join(", "));
    let mut bash_blocked = use_signal(|| task.role.tool_permissions.bash.blocked_commands.join(", "));
    let mut bash_confirm = use_signal(|| task.role.tool_permissions.bash.require_confirmation.join(", "));

    // WritePermission
    let mut write_max_size = use_signal(|| task.role.tool_permissions.write.max_file_size_mb.map(|v| v.to_string()).unwrap_or_default());
    let mut write_extensions = use_signal(|| task.role.tool_permissions.write.allowed_extensions.join(", "));

    // FilePermission
    let mut file_allowed = use_signal(|| task.role.file_permissions.allowed_paths.join(", "));
    let mut file_denied = use_signal(|| task.role.file_permissions.denied_paths.join(", "));
    let mut file_readonly = use_signal(|| task.role.file_permissions.read_only_paths.join(", "));

    // 折りたたみ状態
    let mut role_expanded = use_signal(|| false);
    let mut permissions_expanded = use_signal(|| false);

    let task_id_for_effect = task_id.clone();
    use_effect(move || {
        if let Some(t) = dag.read().tasks.get(&task_id_for_effect) {
            name.set(t.name.clone());
            description.set(t.description.clone());
            executor.set(t.executor.clone());
            prompt.set(t.prompt.clone());
            priority.set(t.priority.to_string());
            status.set(format!("{:?}", t.status));
            inputs_json.set(serde_json::to_string_pretty(&t.inputs).unwrap_or_default());
            args_json.set(serde_json::to_string_pretty(&t.args).unwrap_or_default());
            // Role
            role_id.set(t.role.role_id.clone());
            role_name.set(t.role.name.clone());
            role_description.set(t.role.description.clone());
            subagents.set(t.role.subagents.join(", "));
            skills.set(t.role.skills.join(", "));
            // Permissions
            bash_allowed.set(t.role.tool_permissions.bash.allowed_commands.join(", "));
            bash_blocked.set(t.role.tool_permissions.bash.blocked_commands.join(", "));
            bash_confirm.set(t.role.tool_permissions.bash.require_confirmation.join(", "));
            write_max_size.set(t.role.tool_permissions.write.max_file_size_mb.map(|v| v.to_string()).unwrap_or_default());
            write_extensions.set(t.role.tool_permissions.write.allowed_extensions.join(", "));
            file_allowed.set(t.role.file_permissions.allowed_paths.join(", "));
            file_denied.set(t.role.file_permissions.denied_paths.join(", "));
            file_readonly.set(t.role.file_permissions.read_only_paths.join(", "));
        }
    });

    let task_id_for_save = task_id.clone();
    let task_id_for_delete = task_id.clone();

    rsx! {
        div {
            style: "padding: 16px; display: flex; flex-direction: column; height: 100%;",

            // ヘッダー
            div {
                style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; padding-bottom: 12px; border-bottom: 1px solid #333;",
                h3 {
                    style: "margin: 0; font-size: 14px; color: #fff;",
                    "Edit Task"
                }
                button {
                    style: "background: none; border: none; color: #666; cursor: pointer; font-size: 18px; padding: 0;",
                    onclick: move |_| ui.write().select_task(None),
                    "×"
                }
            }

            // スクロール可能なコンテンツ
            div {
                style: "flex: 1; overflow-y: auto; padding-right: 8px;",

                // === 基本情報 ===
                SectionHeader { title: "Basic Info" }

                ReadOnlyField { label: "Task ID", value: task_id.clone() }

                EditField {
                    label: "Name",
                    value: name(),
                    on_change: move |v: String| name.set(v),
                }

                // Executor
                SelectField {
                    label: "Executor",
                    value: executor(),
                    options: vec![("log", "log"), ("mcp", "mcp")],
                    on_change: move |v: String| executor.set(v),
                }

                // Status
                SelectField {
                    label: "Status",
                    value: status(),
                    options: vec![
                        ("Pending", "Pending"),
                        ("InProgress", "InProgress"),
                        ("Completed", "Completed"),
                        ("Failed", "Failed"),
                    ],
                    on_change: move |v: String| status.set(v),
                }

                NumberField {
                    label: "Priority",
                    value: priority(),
                    min: 1,
                    max: 100,
                    on_change: move |v: String| priority.set(v),
                }

                EditTextArea {
                    label: "Description",
                    value: description(),
                    rows: 2,
                    on_change: move |v: String| description.set(v),
                }

                EditTextArea {
                    label: "Prompt",
                    value: prompt(),
                    rows: 4,
                    on_change: move |v: String| prompt.set(v),
                }

                // Dependencies
                div {
                    style: "margin-bottom: 12px;",
                    label {
                        style: "display: block; font-size: 11px; color: #888; margin-bottom: 4px; text-transform: uppercase;",
                        "Dependencies (via connections)"
                    }
                    div {
                        style: "display: flex; flex-wrap: wrap; gap: 4px; min-height: 28px; padding: 8px; background: #1a1a2e; border-radius: 4px;",
                        if task.dependencies.is_empty() {
                            span { style: "color: #666; font-size: 12px;", "None" }
                        } else {
                            for (idx, dep) in task.dependencies.iter().enumerate() {
                                span {
                                    key: "{idx}-{dep}",
                                    style: "padding: 4px 8px; background: #3a3a5a; border-radius: 4px; font-size: 11px; color: #aaa;",
                                    "{dep}"
                                }
                            }
                        }
                    }
                }

                // === Inputs / Args ===
                SectionHeader { title: "Data" }

                EditTextArea {
                    label: "Inputs (JSON)",
                    value: inputs_json(),
                    rows: 3,
                    on_change: move |v: String| inputs_json.set(v),
                }

                EditTextArea {
                    label: "Args (JSON)",
                    value: args_json(),
                    rows: 3,
                    on_change: move |v: String| args_json.set(v),
                }

                // === Role Section ===
                CollapsibleSection {
                    title: "Role",
                    expanded: role_expanded(),
                    on_toggle: move |_| role_expanded.set(!role_expanded()),

                    EditField {
                        label: "Role ID",
                        value: role_id(),
                        on_change: move |v: String| role_id.set(v),
                    }
                    EditField {
                        label: "Role Name",
                        value: role_name(),
                        on_change: move |v: String| role_name.set(v),
                    }
                    EditTextArea {
                        label: "Role Description",
                        value: role_description(),
                        rows: 2,
                        on_change: move |v: String| role_description.set(v),
                    }
                    EditField {
                        label: "Subagents (comma-separated)",
                        value: subagents(),
                        on_change: move |v: String| subagents.set(v),
                    }
                    EditField {
                        label: "Skills (comma-separated)",
                        value: skills(),
                        on_change: move |v: String| skills.set(v),
                    }
                }

                // === Permissions Section ===
                CollapsibleSection {
                    title: "Permissions",
                    expanded: permissions_expanded(),
                    on_toggle: move |_| permissions_expanded.set(!permissions_expanded()),

                    // Bash Permissions
                    SubSectionHeader { title: "Bash" }
                    EditField {
                        label: "Allowed Commands",
                        value: bash_allowed(),
                        on_change: move |v: String| bash_allowed.set(v),
                    }
                    EditField {
                        label: "Blocked Commands",
                        value: bash_blocked(),
                        on_change: move |v: String| bash_blocked.set(v),
                    }
                    EditField {
                        label: "Require Confirmation",
                        value: bash_confirm(),
                        on_change: move |v: String| bash_confirm.set(v),
                    }

                    // Write Permissions
                    SubSectionHeader { title: "Write" }
                    EditField {
                        label: "Max File Size (MB)",
                        value: write_max_size(),
                        on_change: move |v: String| write_max_size.set(v),
                    }
                    EditField {
                        label: "Allowed Extensions",
                        value: write_extensions(),
                        on_change: move |v: String| write_extensions.set(v),
                    }

                    // File Permissions
                    SubSectionHeader { title: "File Access" }
                    EditField {
                        label: "Allowed Paths",
                        value: file_allowed(),
                        on_change: move |v: String| file_allowed.set(v),
                    }
                    EditField {
                        label: "Denied Paths",
                        value: file_denied(),
                        on_change: move |v: String| file_denied.set(v),
                    }
                    EditField {
                        label: "Read-Only Paths",
                        value: file_readonly(),
                        on_change: move |v: String| file_readonly.set(v),
                    }
                }

                // Position (読み取り専用)
                if let Some((x, y)) = task.position {
                    ReadOnlyField { label: "Position", value: format!("({:.0}, {:.0})", x, y) }
                }
            }

            // ボタンエリア
            div {
                style: "padding-top: 12px; border-top: 1px solid #333; margin-top: 12px;",

                button {
                    style: "width: 100%; padding: 10px; background: #6366f1; color: #fff; border: none; border-radius: 4px; cursor: pointer; font-size: 13px; font-weight: 500;",
                    onclick: move |_| {
                        let mut dag_write = dag.write();
                        if let Some(t) = dag_write.tasks.get_mut(&task_id_for_save) {
                            // Basic fields
                            t.name = name();
                            t.description = description();
                            t.executor = executor();
                            t.prompt = prompt();
                            t.priority = priority().parse().unwrap_or(1);
                            t.status = match status().as_str() {
                                "Pending" => UiStatus::Pending,
                                "InProgress" => UiStatus::InProgress,
                                "Completed" => UiStatus::Completed,
                                "Failed" => UiStatus::Failed,
                                _ => UiStatus::Pending,
                            };
                            // JSON fields
                            if let Ok(v) = serde_json::from_str(&inputs_json()) {
                                t.inputs = v;
                            }
                            if let Ok(v) = serde_json::from_str(&args_json()) {
                                t.args = v;
                            }
                            // Role
                            t.role = Role {
                                role_id: role_id(),
                                name: role_name(),
                                description: role_description(),
                                subagents: parse_csv(&subagents()),
                                skills: parse_csv(&skills()),
                                tool_permissions: ToolPermission {
                                    bash: BashPermission {
                                        allowed_commands: parse_csv(&bash_allowed()),
                                        blocked_commands: parse_csv(&bash_blocked()),
                                        require_confirmation: parse_csv(&bash_confirm()),
                                    },
                                    write: WritePermission {
                                        max_file_size_mb: write_max_size().parse().ok(),
                                        allowed_extensions: parse_csv(&write_extensions()),
                                    },
                                },
                                file_permissions: FilePermission {
                                    allowed_paths: parse_csv(&file_allowed()),
                                    denied_paths: parse_csv(&file_denied()),
                                    read_only_paths: parse_csv(&file_readonly()),
                                },
                            };
                            dag_write.is_dirty = true;
                        }
                    },
                    "Save Changes"
                }

                button {
                    style: "width: 100%; padding: 10px; background: transparent; color: #ef4444; border: 1px solid #ef4444; border-radius: 4px; cursor: pointer; font-size: 13px; margin-top: 8px;",
                    onclick: move |_| {
                        let mut dag_write = dag.write();
                        dag_write.tasks.remove(&task_id_for_delete);
                        dag_write.edges.remove(&task_id_for_delete);
                        for deps in dag_write.edges.values_mut() {
                            deps.retain(|id| id != &task_id_for_delete);
                        }
                        for task in dag_write.tasks.values_mut() {
                            task.dependencies.retain(|id| id != &task_id_for_delete);
                        }
                        dag_write.is_dirty = true;
                        drop(dag_write);
                        ui.write().select_task(None);
                    },
                    "Delete Task"
                }
            }
        }
    }
}

/// カンマ区切り文字列をVecに変換
fn parse_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// === UI Components ===

#[component]
fn SectionHeader(title: &'static str) -> Element {
    rsx! {
        div {
            style: "margin: 16px 0 8px 0; padding-bottom: 4px; border-bottom: 1px solid #333;",
            span {
                style: "font-size: 12px; color: #6366f1; font-weight: 600; text-transform: uppercase;",
                "{title}"
            }
        }
    }
}

#[component]
fn SubSectionHeader(title: &'static str) -> Element {
    rsx! {
        div {
            style: "margin: 12px 0 6px 0;",
            span {
                style: "font-size: 11px; color: #888; font-weight: 500;",
                "{title}"
            }
        }
    }
}

#[component]
fn CollapsibleSection(
    title: &'static str,
    expanded: bool,
    on_toggle: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    rsx! {
        div {
            style: "margin: 12px 0; border: 1px solid #333; border-radius: 4px;",

            // Header
            div {
                style: "display: flex; justify-content: space-between; align-items: center; padding: 10px; background: #1a1a2e; cursor: pointer; border-radius: 4px;",
                onclick: move |e| on_toggle.call(e),
                span {
                    style: "font-size: 12px; color: #fff; font-weight: 500;",
                    "{title}"
                }
                span {
                    style: "color: #666; font-size: 12px;",
                    if expanded { "▼" } else { "▶" }
                }
            }

            // Content
            if expanded {
                div {
                    style: "padding: 12px; background: #0f0f1a;",
                    {children}
                }
            }
        }
    }
}

#[component]
fn ReadOnlyField(label: &'static str, value: String) -> Element {
    rsx! {
        div {
            style: "margin-bottom: 10px;",
            label {
                style: "display: block; font-size: 10px; color: #666; margin-bottom: 3px; text-transform: uppercase;",
                "{label}"
            }
            div {
                style: "padding: 6px 8px; background: #1a1a2e; color: #888; border-radius: 4px; font-size: 12px;",
                "{value}"
            }
        }
    }
}

#[component]
fn EditField(
    label: &'static str,
    value: String,
    on_change: EventHandler<String>,
) -> Element {
    rsx! {
        div {
            style: "margin-bottom: 10px;",
            label {
                style: "display: block; font-size: 10px; color: #666; margin-bottom: 3px; text-transform: uppercase;",
                "{label}"
            }
            input {
                r#type: "text",
                style: "width: 100%; padding: 6px 8px; background: #2a2a4a; color: #fff; border: 1px solid #3a3a5a; border-radius: 4px; font-size: 12px; box-sizing: border-box;",
                value: "{value}",
                oninput: move |evt| on_change.call(evt.value()),
            }
        }
    }
}

#[component]
fn NumberField(
    label: &'static str,
    value: String,
    min: i32,
    max: i32,
    on_change: EventHandler<String>,
) -> Element {
    rsx! {
        div {
            style: "margin-bottom: 10px;",
            label {
                style: "display: block; font-size: 10px; color: #666; margin-bottom: 3px; text-transform: uppercase;",
                "{label}"
            }
            input {
                r#type: "number",
                style: "width: 80px; padding: 6px 8px; background: #2a2a4a; color: #fff; border: 1px solid #3a3a5a; border-radius: 4px; font-size: 12px;",
                value: "{value}",
                min: "{min}",
                max: "{max}",
                onchange: move |evt| on_change.call(evt.value()),
            }
        }
    }
}

#[component]
fn SelectField(
    label: &'static str,
    value: String,
    options: Vec<(&'static str, &'static str)>,
    on_change: EventHandler<String>,
) -> Element {
    rsx! {
        div {
            style: "margin-bottom: 10px;",
            label {
                style: "display: block; font-size: 10px; color: #666; margin-bottom: 3px; text-transform: uppercase;",
                "{label}"
            }
            select {
                style: "width: 100%; padding: 6px 8px; background: #2a2a4a; color: #fff; border: 1px solid #3a3a5a; border-radius: 4px; font-size: 12px;",
                value: "{value}",
                onchange: move |evt| on_change.call(evt.value()),
                for (val, display) in options {
                    option { value: "{val}", "{display}" }
                }
            }
        }
    }
}

#[component]
fn EditTextArea(
    label: &'static str,
    value: String,
    rows: i32,
    on_change: EventHandler<String>,
) -> Element {
    rsx! {
        div {
            style: "margin-bottom: 10px;",
            label {
                style: "display: block; font-size: 10px; color: #666; margin-bottom: 3px; text-transform: uppercase;",
                "{label}"
            }
            textarea {
                style: "width: 100%; padding: 6px 8px; background: #2a2a4a; color: #fff; border: 1px solid #3a3a5a; border-radius: 4px; font-size: 11px; resize: vertical; box-sizing: border-box; font-family: monospace; line-height: 1.4;",
                rows: "{rows}",
                value: "{value}",
                oninput: move |evt| on_change.call(evt.value()),
            }
        }
    }
}
