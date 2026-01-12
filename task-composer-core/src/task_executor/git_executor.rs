//! Git Executor - Git operations using git2-rs
//!
//! Supports local Git repository operations.
//! Note: git2 is a synchronous library, so operations are wrapped with spawn_blocking.

use async_trait::async_trait;
use git2::{
    BranchType, Cred, FetchOptions, IndexAddOption, PushOptions, RemoteCallbacks, Repository,
    Signature, StatusOptions,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

use crate::types::Task;

use super::{ExecutionContext, ExecutionResult, ExecutionStatus, TaskExecutor};

/// Git Executor for local repository operations
pub struct GitExecutor;

impl GitExecutor {
    /// Create a new GitExecutor
    pub fn new() -> Self {
        GitExecutor
    }
}

impl Default for GitExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Arguments for Git operations
#[derive(Debug, Deserialize)]
struct GitArgs {
    /// Action to perform
    action: GitAction,
}

/// Supported Git actions
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum GitAction {
    // Repository operations
    Clone(CloneArgs),
    Open(OpenArgs),
    Init(InitArgs),

    // Commit operations
    Commit(CommitArgs),

    // Branch operations
    CreateBranch(CreateBranchArgs),
    Checkout(CheckoutArgs),
    ListBranches(ListBranchesArgs),
    DeleteBranch(DeleteBranchArgs),

    // Remote operations
    Fetch(FetchArgs),
    Push(PushArgs),

    // Status and Diff
    Status(StatusArgs),
    Diff(DiffArgs),
    Log(LogArgs),
}

// Repository operation arguments

#[derive(Debug, Deserialize)]
struct CloneArgs {
    url: String,
    path: String,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    auth: Option<AuthConfig>,
}

#[derive(Debug, Deserialize)]
struct OpenArgs {
    path: String,
}

#[derive(Debug, Deserialize)]
struct InitArgs {
    path: String,
    #[serde(default)]
    bare: bool,
}

// Commit operation arguments

#[derive(Debug, Deserialize)]
struct CommitArgs {
    path: String,
    message: String,
    #[serde(default)]
    author_name: Option<String>,
    #[serde(default)]
    author_email: Option<String>,
    #[serde(default)]
    add_all: bool,
}

// Branch operation arguments

#[derive(Debug, Deserialize)]
struct CreateBranchArgs {
    path: String,
    name: String,
    #[serde(default)]
    checkout: bool,
}

#[derive(Debug, Deserialize)]
struct CheckoutArgs {
    path: String,
    branch: String,
}

#[derive(Debug, Deserialize)]
struct ListBranchesArgs {
    path: String,
    #[serde(default)]
    branch_type: Option<BranchTypeArg>,
}

#[derive(Debug, Deserialize)]
struct DeleteBranchArgs {
    path: String,
    name: String,
}

// Remote operation arguments

#[derive(Debug, Deserialize)]
struct FetchArgs {
    path: String,
    #[serde(default)]
    remote: Option<String>,
    #[serde(default)]
    auth: Option<AuthConfig>,
}

#[derive(Debug, Deserialize)]
struct PushArgs {
    path: String,
    #[serde(default)]
    remote: Option<String>,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    auth: Option<AuthConfig>,
}

// Status and Diff arguments

#[derive(Debug, Deserialize)]
struct StatusArgs {
    path: String,
    #[serde(default)]
    include_untracked: bool,
}

#[derive(Debug, Deserialize)]
struct DiffArgs {
    path: String,
    #[serde(default)]
    staged: bool,
}

#[derive(Debug, Deserialize)]
struct LogArgs {
    path: String,
    #[serde(default)]
    max_count: Option<usize>,
}

// Authentication configuration

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AuthConfig {
    SshAgent {
        #[serde(default = "default_ssh_username")]
        username: String,
    },
    SshKey {
        #[serde(default = "default_ssh_username")]
        username: String,
        private_key_path: String,
        #[serde(default)]
        passphrase: Option<String>,
    },
    UserPassword {
        username: String,
        password: String,
    },
}

fn default_ssh_username() -> String {
    "git".to_string()
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum BranchTypeArg {
    Local,
    Remote,
    All,
}

impl From<BranchTypeArg> for Option<BranchType> {
    fn from(bt: BranchTypeArg) -> Self {
        match bt {
            BranchTypeArg::Local => Some(BranchType::Local),
            BranchTypeArg::Remote => Some(BranchType::Remote),
            BranchTypeArg::All => None,
        }
    }
}

#[async_trait]
impl TaskExecutor for GitExecutor {
    fn name(&self) -> &str {
        "git"
    }

    async fn execute_task(
        &self,
        task: &Task,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        let args: GitArgs = serde_json::from_value(ctx.args.clone())
            .map_err(|e| format!("Failed to parse Git args: {}", e))?;

        let output = match args.action {
            GitAction::Clone(a) => execute_clone(a).await?,
            GitAction::Open(a) => execute_open(a).await?,
            GitAction::Init(a) => execute_init(a).await?,
            GitAction::Commit(a) => execute_commit(a).await?,
            GitAction::CreateBranch(a) => execute_create_branch(a).await?,
            GitAction::Checkout(a) => execute_checkout(a).await?,
            GitAction::ListBranches(a) => execute_list_branches(a).await?,
            GitAction::DeleteBranch(a) => execute_delete_branch(a).await?,
            GitAction::Fetch(a) => execute_fetch(a).await?,
            GitAction::Push(a) => execute_push(a).await?,
            GitAction::Status(a) => execute_status(a).await?,
            GitAction::Diff(a) => execute_diff(a).await?,
            GitAction::Log(a) => execute_log(a).await?,
        };

        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            status: ExecutionStatus::Success,
            output,
        })
    }
}

// Helper function to create remote callbacks with authentication
fn create_remote_callbacks(auth: Option<AuthConfig>) -> RemoteCallbacks<'static> {
    let mut callbacks = RemoteCallbacks::new();

    if let Some(auth_config) = auth {
        let auth_config = auth_config.clone();
        callbacks.credentials(move |_url, username_from_url, allowed_types| {
            let username = username_from_url.unwrap_or("git");

            match &auth_config {
                AuthConfig::SshAgent { username: u } => {
                    if allowed_types.contains(git2::CredentialType::SSH_KEY) {
                        Cred::ssh_key_from_agent(u)
                    } else {
                        Cred::default()
                    }
                }
                AuthConfig::SshKey {
                    username: u,
                    private_key_path,
                    passphrase,
                } => {
                    if allowed_types.contains(git2::CredentialType::SSH_KEY) {
                        Cred::ssh_key(
                            u,
                            None,
                            Path::new(private_key_path),
                            passphrase.as_deref(),
                        )
                    } else {
                        Cred::default()
                    }
                }
                AuthConfig::UserPassword { username, password } => {
                    if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
                        Cred::userpass_plaintext(username, password)
                    } else {
                        Cred::default()
                    }
                }
            }
        });
    } else {
        // Default: try SSH agent
        callbacks.credentials(|_url, username_from_url, allowed_types| {
            if allowed_types.contains(git2::CredentialType::SSH_KEY) {
                Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
            } else {
                Cred::default()
            }
        });
    }

    callbacks
}

// Repository operations

async fn execute_clone(args: CloneArgs) -> Result<Value, String> {
    let url = args.url;
    let path = args.path;
    let branch = args.branch;
    let auth = args.auth;

    tokio::task::spawn_blocking(move || {
        let callbacks = create_remote_callbacks(auth);
        let mut fo = FetchOptions::new();
        fo.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);

        if let Some(ref b) = branch {
            builder.branch(b);
        }

        let repo = builder
            .clone(&url, Path::new(&path))
            .map_err(|e| format!("Failed to clone repository: {}", e))?;

        let head = repo.head().map_err(|e| format!("Failed to get HEAD: {}", e))?;
        let branch_name = head.shorthand().unwrap_or("HEAD").to_string();

        Ok(json!({
            "action": "clone",
            "url": url,
            "path": path,
            "branch": branch_name,
            "success": true,
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

async fn execute_open(args: OpenArgs) -> Result<Value, String> {
    let path = args.path;

    tokio::task::spawn_blocking(move || {
        let repo =
            Repository::open(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let head = repo.head().ok();
        let branch_name = head
            .as_ref()
            .and_then(|h| h.shorthand())
            .unwrap_or("HEAD")
            .to_string();

        let is_bare = repo.is_bare();

        Ok(json!({
            "action": "open",
            "path": path,
            "branch": branch_name,
            "is_bare": is_bare,
            "success": true,
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

async fn execute_init(args: InitArgs) -> Result<Value, String> {
    let path = args.path;
    let bare = args.bare;

    tokio::task::spawn_blocking(move || {
        let _repo = if bare {
            Repository::init_bare(&path)
        } else {
            Repository::init(&path)
        }
        .map_err(|e| format!("Failed to init repository: {}", e))?;

        Ok(json!({
            "action": "init",
            "path": path,
            "bare": bare,
            "success": true,
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

// Commit operations

async fn execute_commit(args: CommitArgs) -> Result<Value, String> {
    let path = args.path;
    let message = args.message;
    let author_name = args.author_name.unwrap_or_else(|| "Task Composer".to_string());
    let author_email = args
        .author_email
        .unwrap_or_else(|| "task-composer@example.com".to_string());
    let add_all = args.add_all;

    tokio::task::spawn_blocking(move || {
        let repo =
            Repository::open(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let mut index = repo
            .index()
            .map_err(|e| format!("Failed to get index: {}", e))?;

        if add_all {
            index
                .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
                .map_err(|e| format!("Failed to add files: {}", e))?;
            index
                .write()
                .map_err(|e| format!("Failed to write index: {}", e))?;
        }

        let tree_id = index
            .write_tree()
            .map_err(|e| format!("Failed to write tree: {}", e))?;
        let tree = repo
            .find_tree(tree_id)
            .map_err(|e| format!("Failed to find tree: {}", e))?;

        let sig = Signature::now(&author_name, &author_email)
            .map_err(|e| format!("Failed to create signature: {}", e))?;

        let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();

        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, &message, &tree, &parents)
            .map_err(|e| format!("Failed to create commit: {}", e))?;

        Ok(json!({
            "action": "commit",
            "path": path,
            "oid": oid.to_string(),
            "message": message,
            "author": format!("{} <{}>", author_name, author_email),
            "success": true,
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

// Branch operations

async fn execute_create_branch(args: CreateBranchArgs) -> Result<Value, String> {
    let path = args.path;
    let name = args.name;
    let checkout = args.checkout;

    tokio::task::spawn_blocking(move || {
        let repo =
            Repository::open(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let head = repo
            .head()
            .map_err(|e| format!("Failed to get HEAD: {}", e))?;
        let commit = head
            .peel_to_commit()
            .map_err(|e| format!("Failed to get commit: {}", e))?;

        repo.branch(&name, &commit, false)
            .map_err(|e| format!("Failed to create branch: {}", e))?;

        if checkout {
            let refname = format!("refs/heads/{}", name);
            repo.set_head(&refname)
                .map_err(|e| format!("Failed to set HEAD: {}", e))?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                .map_err(|e| format!("Failed to checkout: {}", e))?;
        }

        Ok(json!({
            "action": "create_branch",
            "path": path,
            "name": name,
            "checked_out": checkout,
            "success": true,
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

async fn execute_checkout(args: CheckoutArgs) -> Result<Value, String> {
    let path = args.path;
    let branch = args.branch;

    tokio::task::spawn_blocking(move || {
        let repo =
            Repository::open(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let refname = format!("refs/heads/{}", branch);
        repo.set_head(&refname)
            .map_err(|e| format!("Failed to set HEAD: {}", e))?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| format!("Failed to checkout: {}", e))?;

        Ok(json!({
            "action": "checkout",
            "path": path,
            "branch": branch,
            "success": true,
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

async fn execute_list_branches(args: ListBranchesArgs) -> Result<Value, String> {
    let path = args.path;
    let branch_type = args.branch_type;

    tokio::task::spawn_blocking(move || {
        let repo =
            Repository::open(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let filter: Option<BranchType> = branch_type.map(|bt| bt.into()).flatten();

        let branches = repo
            .branches(filter)
            .map_err(|e| format!("Failed to list branches: {}", e))?;

        let mut branch_list = Vec::new();
        for branch_result in branches {
            let (branch, branch_type) =
                branch_result.map_err(|e| format!("Failed to get branch: {}", e))?;
            if let Some(name) = branch.name().ok().flatten() {
                branch_list.push(json!({
                    "name": name,
                    "is_head": branch.is_head(),
                    "type": match branch_type {
                        BranchType::Local => "local",
                        BranchType::Remote => "remote",
                    },
                }));
            }
        }

        Ok(json!({
            "action": "list_branches",
            "path": path,
            "branches": branch_list,
            "count": branch_list.len(),
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

async fn execute_delete_branch(args: DeleteBranchArgs) -> Result<Value, String> {
    let path = args.path;
    let name = args.name;

    tokio::task::spawn_blocking(move || {
        let repo =
            Repository::open(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let mut branch = repo
            .find_branch(&name, BranchType::Local)
            .map_err(|e| format!("Failed to find branch: {}", e))?;

        branch
            .delete()
            .map_err(|e| format!("Failed to delete branch: {}", e))?;

        Ok(json!({
            "action": "delete_branch",
            "path": path,
            "name": name,
            "success": true,
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

// Remote operations

async fn execute_fetch(args: FetchArgs) -> Result<Value, String> {
    let path = args.path;
    let remote_name = args.remote.unwrap_or_else(|| "origin".to_string());
    let auth = args.auth;

    tokio::task::spawn_blocking(move || {
        let repo =
            Repository::open(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let mut remote = repo
            .find_remote(&remote_name)
            .map_err(|e| format!("Failed to find remote: {}", e))?;

        let callbacks = create_remote_callbacks(auth);
        let mut fo = FetchOptions::new();
        fo.remote_callbacks(callbacks);

        remote
            .fetch(&[] as &[&str], Some(&mut fo), None)
            .map_err(|e| format!("Failed to fetch: {}", e))?;

        Ok(json!({
            "action": "fetch",
            "path": path,
            "remote": remote_name,
            "success": true,
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

async fn execute_push(args: PushArgs) -> Result<Value, String> {
    let path = args.path;
    let remote_name = args.remote.unwrap_or_else(|| "origin".to_string());
    let branch = args.branch;
    let auth = args.auth;

    tokio::task::spawn_blocking(move || {
        let repo =
            Repository::open(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let mut remote = repo
            .find_remote(&remote_name)
            .map_err(|e| format!("Failed to find remote: {}", e))?;

        let branch_name = match branch {
            Some(b) => b,
            None => {
                let head = repo
                    .head()
                    .map_err(|e| format!("Failed to get HEAD: {}", e))?;
                head.shorthand().unwrap_or("main").to_string()
            }
        };

        let callbacks = create_remote_callbacks(auth);
        let mut po = PushOptions::new();
        po.remote_callbacks(callbacks);

        let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);
        remote
            .push(&[&refspec], Some(&mut po))
            .map_err(|e| format!("Failed to push: {}", e))?;

        Ok(json!({
            "action": "push",
            "path": path,
            "remote": remote_name,
            "branch": branch_name,
            "success": true,
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

// Status and Diff

async fn execute_status(args: StatusArgs) -> Result<Value, String> {
    let path = args.path;
    let include_untracked = args.include_untracked;

    tokio::task::spawn_blocking(move || {
        let repo =
            Repository::open(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let mut opts = StatusOptions::new();
        opts.include_untracked(include_untracked)
            .recurse_untracked_dirs(include_untracked);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| format!("Failed to get status: {}", e))?;

        let mut staged = Vec::new();
        let mut modified = Vec::new();
        let mut untracked = Vec::new();

        for entry in statuses.iter() {
            let status = entry.status();
            let file_path = entry.path().unwrap_or("").to_string();

            if status.contains(git2::Status::INDEX_NEW)
                || status.contains(git2::Status::INDEX_MODIFIED)
                || status.contains(git2::Status::INDEX_DELETED)
            {
                staged.push(file_path.clone());
            }
            if status.contains(git2::Status::WT_MODIFIED)
                || status.contains(git2::Status::WT_DELETED)
            {
                modified.push(file_path.clone());
            }
            if status.contains(git2::Status::WT_NEW) {
                untracked.push(file_path);
            }
        }

        let head = repo.head().ok();
        let branch = head
            .as_ref()
            .and_then(|h| h.shorthand())
            .unwrap_or("HEAD")
            .to_string();

        Ok(json!({
            "action": "status",
            "path": path,
            "branch": branch,
            "staged": staged,
            "modified": modified,
            "untracked": untracked,
            "is_clean": staged.is_empty() && modified.is_empty() && untracked.is_empty(),
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

async fn execute_diff(args: DiffArgs) -> Result<Value, String> {
    let path = args.path;
    let staged = args.staged;

    tokio::task::spawn_blocking(move || {
        let repo =
            Repository::open(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let diff = if staged {
            let head = repo
                .head()
                .map_err(|e| format!("Failed to get HEAD: {}", e))?;
            let tree = head
                .peel_to_tree()
                .map_err(|e| format!("Failed to get tree: {}", e))?;
            repo.diff_tree_to_index(Some(&tree), None, None)
                .map_err(|e| format!("Failed to get diff: {}", e))?
        } else {
            repo.diff_index_to_workdir(None, None)
                .map_err(|e| format!("Failed to get diff: {}", e))?
        };

        let stats = diff
            .stats()
            .map_err(|e| format!("Failed to get diff stats: {}", e))?;

        let mut files_changed = Vec::new();
        for delta in diff.deltas() {
            let old_file = delta.old_file().path().map(|p| p.to_string_lossy().to_string());
            let new_file = delta.new_file().path().map(|p| p.to_string_lossy().to_string());
            files_changed.push(json!({
                "old_file": old_file,
                "new_file": new_file,
                "status": format!("{:?}", delta.status()),
            }));
        }

        Ok(json!({
            "action": "diff",
            "path": path,
            "staged": staged,
            "files_changed": stats.files_changed(),
            "insertions": stats.insertions(),
            "deletions": stats.deletions(),
            "files": files_changed,
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

async fn execute_log(args: LogArgs) -> Result<Value, String> {
    let path = args.path;
    let max_count = args.max_count.unwrap_or(10);

    tokio::task::spawn_blocking(move || {
        let repo =
            Repository::open(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let mut revwalk = repo
            .revwalk()
            .map_err(|e| format!("Failed to create revwalk: {}", e))?;
        revwalk
            .push_head()
            .map_err(|e| format!("Failed to push head: {}", e))?;

        let mut commits = Vec::new();
        for (i, oid_result) in revwalk.enumerate() {
            if i >= max_count {
                break;
            }

            let oid = oid_result.map_err(|e| format!("Failed to get oid: {}", e))?;
            let commit = repo
                .find_commit(oid)
                .map_err(|e| format!("Failed to find commit: {}", e))?;

            commits.push(json!({
                "oid": oid.to_string(),
                "message": commit.message().unwrap_or("").trim(),
                "author": commit.author().name().unwrap_or("unknown"),
                "email": commit.author().email().unwrap_or("unknown"),
                "time": commit.time().seconds(),
            }));
        }

        Ok(json!({
            "action": "log",
            "path": path,
            "commits": commits,
            "count": commits.len(),
        }))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_executor_name() {
        let executor = GitExecutor::new();
        assert_eq!(executor.name(), "git");
    }

    #[test]
    fn test_parse_clone_args() {
        let json = serde_json::json!({
            "action": {
                "type": "clone",
                "url": "https://github.com/user/repo.git",
                "path": "/tmp/repo"
            }
        });

        let args: Result<GitArgs, _> = serde_json::from_value(json);
        assert!(args.is_ok());
    }

    #[test]
    fn test_parse_commit_args() {
        let json = serde_json::json!({
            "action": {
                "type": "commit",
                "path": "/tmp/repo",
                "message": "Initial commit",
                "add_all": true
            }
        });

        let args: Result<GitArgs, _> = serde_json::from_value(json);
        assert!(args.is_ok());
    }

    #[test]
    fn test_parse_push_with_auth() {
        let json = serde_json::json!({
            "action": {
                "type": "push",
                "path": "/tmp/repo",
                "auth": {
                    "type": "ssh_agent",
                    "username": "git"
                }
            }
        });

        let args: Result<GitArgs, _> = serde_json::from_value(json);
        assert!(args.is_ok());
    }
}
