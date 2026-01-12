//! GitHub Executor - GitHub API operations using octocrab
//!
//! Supports Issue and Pull Request operations via GitHub REST API.

use async_trait::async_trait;
use octocrab::models;
use octocrab::params;
use octocrab::Octocrab;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::types::Task;

use super::{ExecutionContext, ExecutionResult, ExecutionStatus, TaskExecutor};

/// GitHub Executor for Issue and Pull Request operations
pub struct GitHubExecutor {
    default_token: Option<String>,
}

impl GitHubExecutor {
    /// Create a new GitHubExecutor without default authentication
    pub fn new() -> Self {
        GitHubExecutor { default_token: None }
    }

    /// Create a new GitHubExecutor with a default Personal Access Token
    pub fn with_token(token: String) -> Self {
        GitHubExecutor {
            default_token: Some(token),
        }
    }

    /// Build an Octocrab client with the given token
    fn build_client(&self, token: Option<&str>) -> Result<Octocrab, String> {
        let token = token
            .or(self.default_token.as_deref())
            .ok_or_else(|| "GitHub token is required. Provide 'token' in args or set default token.".to_string())?;

        Octocrab::builder()
            .personal_token(token.to_string())
            .build()
            .map_err(|e| format!("Failed to build GitHub client: {}", e))
    }
}

impl Default for GitHubExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Arguments for GitHub operations
#[derive(Debug, Deserialize)]
struct GitHubArgs {
    /// GitHub Personal Access Token (optional if default is set)
    token: Option<String>,
    /// Repository owner
    owner: String,
    /// Repository name
    repo: String,
    /// Action to perform
    action: GitHubAction,
}

/// Supported GitHub actions
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum GitHubAction {
    // Issue operations
    CreateIssue(CreateIssueArgs),
    GetIssue(GetIssueArgs),
    ListIssues(ListIssuesArgs),
    UpdateIssue(UpdateIssueArgs),
    CloseIssue(CloseIssueArgs),
    CreateComment(CreateCommentArgs),
    DeleteComment(DeleteCommentArgs),

    // Pull Request operations
    CreatePr(CreatePrArgs),
    GetPr(GetPrArgs),
    ListPrs(ListPrsArgs),
    MergePr(MergePrArgs),
    RequestReview(RequestReviewArgs),
}

// Issue operation arguments

#[derive(Debug, Deserialize)]
struct CreateIssueArgs {
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    labels: Option<Vec<String>>,
    #[serde(default)]
    assignees: Option<Vec<String>>,
    #[serde(default)]
    milestone: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GetIssueArgs {
    number: u64,
}

#[derive(Debug, Deserialize)]
struct ListIssuesArgs {
    #[serde(default)]
    state: Option<IssueState>,
    #[serde(default)]
    labels: Option<Vec<String>>,
    #[serde(default)]
    assignee: Option<String>,
    #[serde(default)]
    per_page: Option<u8>,
    #[serde(default)]
    page: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct UpdateIssueArgs {
    number: u64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    state: Option<IssueState>,
    #[serde(default)]
    labels: Option<Vec<String>>,
    #[serde(default)]
    assignees: Option<Vec<String>>,
    #[serde(default)]
    milestone: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CloseIssueArgs {
    number: u64,
    #[serde(default)]
    reason: Option<IssueStateReason>,
}

#[derive(Debug, Deserialize)]
struct CreateCommentArgs {
    number: u64,
    body: String,
}

#[derive(Debug, Deserialize)]
struct DeleteCommentArgs {
    comment_id: u64,
}

// Pull Request operation arguments

#[derive(Debug, Deserialize)]
struct CreatePrArgs {
    title: String,
    head: String,
    base: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: Option<bool>,
    #[serde(default)]
    maintainer_can_modify: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct GetPrArgs {
    number: u64,
}

#[derive(Debug, Deserialize)]
struct ListPrsArgs {
    #[serde(default)]
    state: Option<PrState>,
    #[serde(default)]
    head: Option<String>,
    #[serde(default)]
    base: Option<String>,
    #[serde(default)]
    sort: Option<PrSort>,
    #[serde(default)]
    direction: Option<SortDirection>,
    #[serde(default)]
    per_page: Option<u8>,
    #[serde(default)]
    page: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct MergePrArgs {
    number: u64,
    #[serde(default)]
    commit_title: Option<String>,
    #[serde(default)]
    commit_message: Option<String>,
    #[serde(default)]
    merge_method: Option<MergeMethod>,
    #[serde(default)]
    sha: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RequestReviewArgs {
    number: u64,
    #[serde(default)]
    reviewers: Option<Vec<String>>,
    #[serde(default)]
    team_reviewers: Option<Vec<String>>,
}

// Enums for API parameters

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum IssueState {
    Open,
    Closed,
    All,
}

impl From<IssueState> for params::State {
    fn from(state: IssueState) -> Self {
        match state {
            IssueState::Open => params::State::Open,
            IssueState::Closed => params::State::Closed,
            IssueState::All => params::State::All,
        }
    }
}

impl From<IssueState> for models::IssueState {
    fn from(state: IssueState) -> Self {
        match state {
            IssueState::Open => models::IssueState::Open,
            IssueState::Closed => models::IssueState::Closed,
            IssueState::All => models::IssueState::Open, // All is not supported, default to Open
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum IssueStateReason {
    Completed,
    NotPlanned,
}

impl From<IssueStateReason> for models::issues::IssueStateReason {
    fn from(reason: IssueStateReason) -> Self {
        match reason {
            IssueStateReason::Completed => models::issues::IssueStateReason::Completed,
            IssueStateReason::NotPlanned => models::issues::IssueStateReason::NotPlanned,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum PrState {
    Open,
    Closed,
    All,
}

impl From<PrState> for params::State {
    fn from(state: PrState) -> Self {
        match state {
            PrState::Open => params::State::Open,
            PrState::Closed => params::State::Closed,
            PrState::All => params::State::All,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum PrSort {
    Created,
    Updated,
    Popularity,
    LongRunning,
}

impl From<PrSort> for params::pulls::Sort {
    fn from(sort: PrSort) -> Self {
        match sort {
            PrSort::Created => params::pulls::Sort::Created,
            PrSort::Updated => params::pulls::Sort::Updated,
            PrSort::Popularity => params::pulls::Sort::Popularity,
            PrSort::LongRunning => params::pulls::Sort::LongRunning,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum SortDirection {
    Asc,
    Desc,
}

impl From<SortDirection> for params::Direction {
    fn from(dir: SortDirection) -> Self {
        match dir {
            SortDirection::Asc => params::Direction::Ascending,
            SortDirection::Desc => params::Direction::Descending,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

impl From<MergeMethod> for params::pulls::MergeMethod {
    fn from(method: MergeMethod) -> Self {
        match method {
            MergeMethod::Merge => params::pulls::MergeMethod::Merge,
            MergeMethod::Squash => params::pulls::MergeMethod::Squash,
            MergeMethod::Rebase => params::pulls::MergeMethod::Rebase,
        }
    }
}

#[async_trait]
impl TaskExecutor for GitHubExecutor {
    fn name(&self) -> &str {
        "github"
    }

    async fn execute_task(
        &self,
        task: &Task,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult, String> {
        let args: GitHubArgs = serde_json::from_value(ctx.args.clone())
            .map_err(|e| format!("Failed to parse GitHub args: {}", e))?;

        let client = self.build_client(args.token.as_deref())?;
        let owner = &args.owner;
        let repo = &args.repo;

        let output = match args.action {
            // Issue operations
            GitHubAction::CreateIssue(a) => {
                self.create_issue(&client, owner, repo, a).await?
            }
            GitHubAction::GetIssue(a) => {
                self.get_issue(&client, owner, repo, a).await?
            }
            GitHubAction::ListIssues(a) => {
                self.list_issues(&client, owner, repo, a).await?
            }
            GitHubAction::UpdateIssue(a) => {
                self.update_issue(&client, owner, repo, a).await?
            }
            GitHubAction::CloseIssue(a) => {
                self.close_issue(&client, owner, repo, a).await?
            }
            GitHubAction::CreateComment(a) => {
                self.create_comment(&client, owner, repo, a).await?
            }
            GitHubAction::DeleteComment(a) => {
                self.delete_comment(&client, owner, repo, a).await?
            }

            // Pull Request operations
            GitHubAction::CreatePr(a) => {
                self.create_pr(&client, owner, repo, a).await?
            }
            GitHubAction::GetPr(a) => {
                self.get_pr(&client, owner, repo, a).await?
            }
            GitHubAction::ListPrs(a) => {
                self.list_prs(&client, owner, repo, a).await?
            }
            GitHubAction::MergePr(a) => {
                self.merge_pr(&client, owner, repo, a).await?
            }
            GitHubAction::RequestReview(a) => {
                self.request_review(&client, owner, repo, a).await?
            }
        };

        Ok(ExecutionResult {
            task_id: task.task_id.clone(),
            status: ExecutionStatus::Success,
            output,
        })
    }
}

impl GitHubExecutor {
    // Issue operations

    async fn create_issue(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: CreateIssueArgs,
    ) -> Result<Value, String> {
        let issues_handler = client.issues(owner, repo);
        let mut builder = issues_handler.create(&args.title);

        if let Some(body) = args.body {
            builder = builder.body(body);
        }
        if let Some(labels) = args.labels {
            builder = builder.labels(labels);
        }
        if let Some(assignees) = args.assignees {
            builder = builder.assignees(assignees);
        }
        if let Some(milestone) = args.milestone {
            builder = builder.milestone(milestone);
        }

        let issue = builder
            .send()
            .await
            .map_err(|e| format!("Failed to create issue: {}", e))?;

        Ok(json!({
            "action": "create_issue",
            "number": issue.number,
            "title": issue.title,
            "state": format!("{:?}", issue.state),
            "html_url": issue.html_url,
            "created_at": issue.created_at.to_string(),
        }))
    }

    async fn get_issue(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: GetIssueArgs,
    ) -> Result<Value, String> {
        let issue = client
            .issues(owner, repo)
            .get(args.number)
            .await
            .map_err(|e| format!("Failed to get issue: {}", e))?;

        Ok(json!({
            "action": "get_issue",
            "number": issue.number,
            "title": issue.title,
            "body": issue.body,
            "state": format!("{:?}", issue.state),
            "html_url": issue.html_url,
            "user": issue.user.login,
            "labels": issue.labels.iter().map(|l| &l.name).collect::<Vec<_>>(),
            "assignees": issue.assignees.iter().map(|a| &a.login).collect::<Vec<_>>(),
            "created_at": issue.created_at.to_string(),
            "updated_at": issue.updated_at.to_string(),
        }))
    }

    async fn list_issues(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: ListIssuesArgs,
    ) -> Result<Value, String> {
        let issues_handler = client.issues(owner, repo);
        let mut builder = issues_handler.list();

        if let Some(state) = args.state {
            let state: params::State = state.into();
            builder = builder.state(state);
        }
        if let Some(labels) = &args.labels {
            builder = builder.labels(labels);
        }
        if let Some(assignee) = &args.assignee {
            builder = builder.assignee(assignee.as_str());
        }
        if let Some(per_page) = args.per_page {
            builder = builder.per_page(per_page);
        }
        if let Some(page) = args.page {
            builder = builder.page(page);
        }

        let issues = builder
            .send()
            .await
            .map_err(|e| format!("Failed to list issues: {}", e))?;

        let items: Vec<Value> = issues
            .items
            .iter()
            .map(|issue| {
                json!({
                    "number": issue.number,
                    "title": issue.title,
                    "state": format!("{:?}", issue.state),
                    "html_url": issue.html_url,
                    "user": issue.user.login,
                })
            })
            .collect();

        Ok(json!({
            "action": "list_issues",
            "count": items.len(),
            "issues": items,
        }))
    }

    async fn update_issue(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: UpdateIssueArgs,
    ) -> Result<Value, String> {
        let issues_handler = client.issues(owner, repo);
        let mut builder = issues_handler.update(args.number);

        // Store these outside to extend their lifetime
        let labels_storage = args.labels;
        let assignees_storage = args.assignees;

        if let Some(title) = &args.title {
            builder = builder.title(title);
        }
        if let Some(body) = &args.body {
            builder = builder.body(body);
        }
        if let Some(state) = args.state {
            let state: models::IssueState = state.into();
            builder = builder.state(state);
        }
        if let Some(ref labels) = labels_storage {
            builder = builder.labels(labels);
        }
        if let Some(ref assignees) = assignees_storage {
            builder = builder.assignees(assignees);
        }
        if let Some(milestone) = args.milestone {
            builder = builder.milestone(milestone);
        }

        let issue = builder
            .send()
            .await
            .map_err(|e| format!("Failed to update issue: {}", e))?;

        Ok(json!({
            "action": "update_issue",
            "number": issue.number,
            "title": issue.title,
            "state": format!("{:?}", issue.state),
            "html_url": issue.html_url,
            "updated_at": issue.updated_at.to_string(),
        }))
    }

    async fn close_issue(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: CloseIssueArgs,
    ) -> Result<Value, String> {
        let issues_handler = client.issues(owner, repo);
        let mut builder = issues_handler
            .update(args.number)
            .state(models::IssueState::Closed);

        if let Some(reason) = args.reason {
            let reason: models::issues::IssueStateReason = reason.into();
            builder = builder.state_reason(reason);
        }

        let issue = builder
            .send()
            .await
            .map_err(|e| format!("Failed to close issue: {}", e))?;

        Ok(json!({
            "action": "close_issue",
            "number": issue.number,
            "title": issue.title,
            "state": format!("{:?}", issue.state),
            "html_url": issue.html_url,
            "closed_at": issue.closed_at.map(|t| t.to_string()),
        }))
    }

    async fn create_comment(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: CreateCommentArgs,
    ) -> Result<Value, String> {
        let comment = client
            .issues(owner, repo)
            .create_comment(args.number, &args.body)
            .await
            .map_err(|e| format!("Failed to create comment: {}", e))?;

        Ok(json!({
            "action": "create_comment",
            "id": comment.id,
            "body": comment.body,
            "html_url": comment.html_url,
            "user": comment.user.login,
            "created_at": comment.created_at.to_string(),
        }))
    }

    async fn delete_comment(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: DeleteCommentArgs,
    ) -> Result<Value, String> {
        client
            .issues(owner, repo)
            .delete_comment(args.comment_id.into())
            .await
            .map_err(|e| format!("Failed to delete comment: {}", e))?;

        Ok(json!({
            "action": "delete_comment",
            "comment_id": args.comment_id,
            "deleted": true,
        }))
    }

    // Pull Request operations

    async fn create_pr(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: CreatePrArgs,
    ) -> Result<serde_json::Value, String> {
        let pulls_handler = client.pulls(owner, repo);
        let mut builder = pulls_handler.create(&args.title, &args.head, &args.base);

        if let Some(body) = args.body {
            builder = builder.body(body);
        }
        if let Some(draft) = args.draft {
            builder = builder.draft(draft);
        }
        if let Some(maintainer_can_modify) = args.maintainer_can_modify {
            builder = builder.maintainer_can_modify(maintainer_can_modify);
        }

        let pr = builder
            .send()
            .await
            .map_err(|e| format!("Failed to create PR: {}", e))?;

        Ok(json!({
            "action": "create_pr",
            "number": pr.number,
            "title": pr.title.unwrap_or_default(),
            "state": format!("{:?}", pr.state.unwrap_or(models::IssueState::Open)),
            "html_url": pr.html_url.map(|u| u.to_string()),
            "head": pr.head.ref_field,
            "base": pr.base.ref_field,
            "draft": pr.draft,
            "created_at": pr.created_at.map(|t| t.to_string()),
        }))
    }

    async fn get_pr(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: GetPrArgs,
    ) -> Result<Value, String> {
        let pr = client
            .pulls(owner, repo)
            .get(args.number)
            .await
            .map_err(|e| format!("Failed to get PR: {}", e))?;

        Ok(json!({
            "action": "get_pr",
            "number": pr.number,
            "title": pr.title,
            "body": pr.body,
            "state": format!("{:?}", pr.state),
            "html_url": pr.html_url.map(|u| u.to_string()),
            "user": pr.user.map(|u| u.login),
            "head": pr.head.ref_field,
            "base": pr.base.ref_field,
            "draft": pr.draft,
            "mergeable": pr.mergeable,
            "merged": pr.merged,
            "created_at": pr.created_at.map(|t| t.to_string()),
            "updated_at": pr.updated_at.map(|t| t.to_string()),
        }))
    }

    async fn list_prs(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: ListPrsArgs,
    ) -> Result<Value, String> {
        let pulls_handler = client.pulls(owner, repo);
        let mut builder = pulls_handler.list();

        if let Some(state) = args.state {
            let state: params::State = state.into();
            builder = builder.state(state);
        }
        if let Some(head) = &args.head {
            builder = builder.head(head);
        }
        if let Some(base) = &args.base {
            builder = builder.base(base);
        }
        if let Some(sort) = args.sort {
            let sort: params::pulls::Sort = sort.into();
            builder = builder.sort(sort);
        }
        if let Some(direction) = args.direction {
            let direction: params::Direction = direction.into();
            builder = builder.direction(direction);
        }
        if let Some(per_page) = args.per_page {
            builder = builder.per_page(per_page);
        }
        if let Some(page) = args.page {
            builder = builder.page(page);
        }

        let prs = builder
            .send()
            .await
            .map_err(|e| format!("Failed to list PRs: {}", e))?;

        let items: Vec<Value> = prs
            .items
            .iter()
            .map(|pr| {
                json!({
                    "number": pr.number,
                    "title": pr.title,
                    "state": format!("{:?}", pr.state),
                    "html_url": pr.html_url.as_ref().map(|u| u.to_string()),
                    "user": pr.user.as_ref().map(|u| &u.login),
                    "head": pr.head.ref_field,
                    "base": pr.base.ref_field,
                    "draft": pr.draft,
                })
            })
            .collect();

        Ok(json!({
            "action": "list_prs",
            "count": items.len(),
            "pull_requests": items,
        }))
    }

    async fn merge_pr(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: MergePrArgs,
    ) -> Result<Value, String> {
        let pulls_handler = client.pulls(owner, repo);
        let mut builder = pulls_handler.merge(args.number);

        if let Some(title) = args.commit_title {
            builder = builder.title(title);
        }
        if let Some(message) = args.commit_message {
            builder = builder.message(message);
        }
        if let Some(method) = args.merge_method {
            let method: params::pulls::MergeMethod = method.into();
            builder = builder.method(method);
        }
        if let Some(sha) = args.sha {
            builder = builder.sha(sha);
        }

        let merge = builder
            .send()
            .await
            .map_err(|e| format!("Failed to merge PR: {}", e))?;

        Ok(json!({
            "action": "merge_pr",
            "number": args.number,
            "sha": merge.sha,
            "merged": merge.merged,
            "message": merge.message,
        }))
    }

    async fn request_review(
        &self,
        client: &Octocrab,
        owner: &str,
        repo: &str,
        args: RequestReviewArgs,
    ) -> Result<Value, String> {
        let reviewers = args.reviewers.unwrap_or_default();
        let team_reviewers = args.team_reviewers.unwrap_or_default();

        let pr = client
            .pulls(owner, repo)
            .request_reviews(args.number, reviewers.clone(), team_reviewers.clone())
            .await
            .map_err(|e| format!("Failed to request review: {}", e))?;

        Ok(json!({
            "action": "request_review",
            "number": args.number,
            "requested_reviewers": reviewers,
            "requested_teams": team_reviewers,
            "html_url": pr.html_url,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_executor_name() {
        let executor = GitHubExecutor::new();
        assert_eq!(executor.name(), "github");
    }

    #[test]
    fn test_parse_create_issue_args() {
        let json = serde_json::json!({
            "owner": "test-owner",
            "repo": "test-repo",
            "action": {
                "type": "create_issue",
                "title": "Test Issue",
                "body": "Test body",
                "labels": ["bug", "help wanted"]
            }
        });

        let args: Result<GitHubArgs, _> = serde_json::from_value(json);
        assert!(args.is_ok());
    }

    #[test]
    fn test_parse_merge_pr_args() {
        let json = serde_json::json!({
            "owner": "test-owner",
            "repo": "test-repo",
            "action": {
                "type": "merge_pr",
                "number": 42,
                "merge_method": "squash"
            }
        });

        let args: Result<GitHubArgs, _> = serde_json::from_value(json);
        assert!(args.is_ok());
    }
}
