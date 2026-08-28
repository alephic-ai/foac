use std::process::Command as ProcessCommand;

use clap::{Args, Subcommand, ValueEnum};
use reqwest::Method;
use serde_json::{Map, Value, json};

use crate::outdoc;
use crate::pipe::{self, FromFlag};
use crate::rest::{self, Api, Auth, BodyInput, insert_opt, push_query};

const API_URL: &str = "https://api.github.com/";
const API_VERSION: &str = "2026-03-10";
const HEADERS: &[(&str, &str)] = &[
    ("Accept", "application/vnd.github+json"),
    ("X-GitHub-Api-Version", API_VERSION),
];

#[derive(Args)]
pub struct Cmd {
    /// Repository as OWNER/NAME; defaults to the current GitHub git remote
    #[arg(long, global = true)]
    repo: Option<String>,
    #[command(subcommand)]
    command: Resource,
}

#[derive(Subcommand)]
enum Resource {
    /// Repositories
    #[command(subcommand)]
    Repo(RepoCmd),
    /// Issues (pull requests are excluded from list results)
    #[command(subcommand)]
    Issue(IssueCmd),
    /// Conversation comments on issues or pull requests
    #[command(subcommand)]
    Comment(CommentCmd),
    /// Pull requests
    #[command(subcommand)]
    Pull(PullCmd),
    /// Pull request reviews
    #[command(subcommand)]
    Review(ReviewCmd),
    /// GitHub Actions workflows
    #[command(subcommand)]
    Workflow(WorkflowCmd),
    /// GitHub Actions workflow runs
    #[command(subcommand)]
    Run(RunCmd),
    /// Branches
    #[command(subcommand)]
    Branch(BranchCmd),
    /// Git references
    #[command(subcommand)]
    Ref(RefCmd),
    /// Branch protection rules
    #[command(subcommand)]
    BranchProtection(BranchProtectionCmd),
    /// Commits
    #[command(subcommand)]
    Commit(CommitCmd),
    /// Commit comments
    #[command(subcommand)]
    CommitComment(CommitCommentCmd),
    /// Commit statuses
    #[command(subcommand)]
    Status(StatusCmd),
    /// Check runs
    #[command(subcommand)]
    CheckRun(CheckRunCmd),
    /// Check suites
    #[command(subcommand)]
    CheckSuite(CheckSuiteCmd),
    /// Releases
    #[command(subcommand)]
    Release(ReleaseCmd),
    /// Release asset metadata
    #[command(subcommand)]
    ReleaseAsset(ReleaseAssetCmd),
    /// GitHub Actions artifact metadata
    #[command(subcommand)]
    Artifact(ArtifactCmd),
    /// Issue and pull request labels
    #[command(subcommand)]
    Label(LabelCmd),
    /// Repository collaborators
    #[command(subcommand)]
    Collaborator(CollaboratorCmd),
}

#[derive(Args, Clone)]
struct Page {
    /// Results per page
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u8).range(1..=100))]
    limit: u8,
    /// One-based page number
    #[arg(long, default_value_t = 1)]
    page: u32,
}

#[derive(Subcommand)]
enum RepoCmd {
    /// List repositories accessible to the authenticated user
    #[command(after_long_help = outdoc::rest_list("raw GitHub repository objects", &[], &outdoc::GITHUB_PAGES))]
    List {
        #[arg(long)]
        visibility: Option<String>,
        #[arg(long)]
        affiliation: Option<String>,
        #[arg(long)]
        sort: Option<String>,
        #[arg(long)]
        direction: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get the selected repository
    #[command(after_long_help = outdoc::rest_obj("raw GitHub repository object", "full_name (OWNER/NAME); id is numeric"))]
    Get,
}

#[derive(Subcommand)]
enum IssueCmd {
    /// List issues
    #[command(after_long_help = outdoc::rest_list("raw GitHub issue objects (pull requests are filtered out)", &["number"], &outdoc::GITHUB_PAGES))]
    List {
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        assignee: Option<String>,
        #[arg(long)]
        creator: Option<String>,
        #[arg(long)]
        mentioned: Option<String>,
        #[arg(long)]
        milestone: Option<String>,
        #[arg(long = "label")]
        labels: Vec<String>,
        #[arg(long)]
        sort: Option<String>,
        #[arg(long)]
        direction: Option<String>,
        /// ISO 8601 timestamp
        #[arg(long)]
        since: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get an issue by number
    #[command(after_long_help = outdoc::rest_obj("raw GitHub issue object", "number"))]
    Get {
        number: Option<u64>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Create an issue
    #[command(after_long_help = outdoc::rest_obj("raw GitHub issue object", "number"))]
    Create {
        #[arg(long)]
        title: String,
        #[command(flatten)]
        body: BodyInput,
        #[arg(long = "assignee")]
        assignees: Vec<String>,
        #[arg(long = "label")]
        labels: Vec<String>,
        #[arg(long)]
        milestone: Option<u64>,
    },
    /// Update an issue; only supplied fields are changed
    #[command(after_long_help = outdoc::rest_obj("raw GitHub issue object", "number"))]
    Update {
        number: u64,
        #[arg(long)]
        title: Option<String>,
        #[command(flatten)]
        body: BodyInput,
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        state_reason: Option<String>,
        #[arg(long = "assignee")]
        assignees: Vec<String>,
        /// Remove all assignees
        #[arg(long, conflicts_with = "assignees")]
        clear_assignees: bool,
        #[arg(long = "label")]
        labels: Vec<String>,
        /// Remove all labels
        #[arg(long, conflicts_with = "labels")]
        clear_labels: bool,
        #[arg(long)]
        milestone: Option<u64>,
        /// Remove the milestone
        #[arg(long, conflicts_with = "milestone")]
        clear_milestone: bool,
    },
}

#[derive(Subcommand)]
enum CommentCmd {
    /// List conversation comments
    #[command(after_long_help = outdoc::rest_list("raw GitHub issue comment objects", &[], &outdoc::GITHUB_PAGES))]
    List {
        #[arg(long)]
        issue: u64,
        #[command(flatten)]
        page: Page,
    },
    /// Add a conversation comment
    #[command(after_long_help = outdoc::rest_obj("raw GitHub issue comment object", "id"))]
    Create {
        #[arg(long)]
        issue: u64,
        #[command(flatten)]
        body: BodyInput,
    },
    /// Update a conversation comment
    #[command(after_long_help = outdoc::rest_obj("raw GitHub issue comment object", "id"))]
    Update {
        id: u64,
        #[command(flatten)]
        body: BodyInput,
    },
    /// Delete a conversation comment
    #[command(after_long_help = outdoc::rest_delete())]
    Delete { id: u64 },
}

#[derive(Subcommand)]
enum PullCmd {
    /// List pull requests
    #[command(after_long_help = outdoc::rest_list("raw GitHub pull request objects", &["number"], &outdoc::GITHUB_PAGES))]
    List {
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        head: Option<String>,
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        sort: Option<String>,
        #[arg(long)]
        direction: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get a pull request
    #[command(after_long_help = outdoc::rest_obj("raw GitHub pull request object", "number"))]
    Get {
        number: Option<u64>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Create a pull request
    #[command(after_long_help = outdoc::rest_obj("raw GitHub pull request object", "number"))]
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        head: String,
        #[arg(long)]
        base: String,
        #[command(flatten)]
        body: BodyInput,
        #[arg(long)]
        draft: bool,
        #[arg(long)]
        maintainer_can_modify: bool,
    },
    /// Update a pull request; only supplied fields are changed
    #[command(after_long_help = outdoc::rest_obj("raw GitHub pull request object", "number"))]
    Update {
        number: u64,
        #[arg(long)]
        title: Option<String>,
        #[command(flatten)]
        body: BodyInput,
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        maintainer_can_modify: Option<bool>,
    },
    /// List files changed by a pull request
    #[command(after_long_help = outdoc::rest_list("raw GitHub pull request file objects", &[], &outdoc::GITHUB_PAGES))]
    Files {
        number: u64,
        #[command(flatten)]
        page: Page,
    },
    /// Merge a pull request
    #[command(after_long_help = outdoc::rest_obj("raw GitHub merge result object", ""))]
    Merge {
        number: u64,
        #[arg(long)]
        method: Option<String>,
        #[arg(long)]
        commit_title: Option<String>,
        #[arg(long)]
        commit_message: Option<String>,
        #[arg(long)]
        sha: Option<String>,
    },
}

#[derive(Clone, ValueEnum)]
enum ReviewEvent {
    Approve,
    #[value(name = "request-changes")]
    RequestChanges,
    Comment,
}

impl ReviewEvent {
    fn api_value(&self) -> &'static str {
        match self {
            Self::Approve => "APPROVE",
            Self::RequestChanges => "REQUEST_CHANGES",
            Self::Comment => "COMMENT",
        }
    }
}

#[derive(Subcommand)]
enum ReviewCmd {
    /// List reviews on a pull request
    #[command(after_long_help = outdoc::rest_list("raw GitHub review objects", &[], &outdoc::GITHUB_PAGES))]
    List {
        #[arg(long)]
        pull: u64,
        #[command(flatten)]
        page: Page,
    },
    /// Create a submitted or pending review
    #[command(after_long_help = outdoc::rest_obj("raw GitHub review object", "id"))]
    Create {
        #[arg(long)]
        pull: u64,
        #[arg(long)]
        event: Option<ReviewEvent>,
        #[command(flatten)]
        body: BodyInput,
        #[arg(long)]
        commit_id: Option<String>,
        /// JSON array of GitHub inline review comment objects
        #[arg(long)]
        comments_json: Option<String>,
    },
    /// Submit a pending review
    #[command(after_long_help = outdoc::rest_obj("raw GitHub review object", "id"))]
    Submit {
        #[arg(long)]
        pull: u64,
        id: u64,
        #[arg(long)]
        event: ReviewEvent,
        #[command(flatten)]
        body: BodyInput,
    },
    /// Delete a pending review
    #[command(after_long_help = outdoc::rest_obj("raw GitHub review object", "id"))]
    Delete {
        #[arg(long)]
        pull: u64,
        id: u64,
    },
}

#[derive(Subcommand)]
enum WorkflowCmd {
    /// List workflows
    #[command(after_long_help = outdoc::rest_list("raw GitHub workflow objects", &["id"], &outdoc::GITHUB_PAGES))]
    List {
        #[command(flatten)]
        page: Page,
    },
    /// Get a workflow by numeric ID or file name
    #[command(after_long_help = outdoc::rest_obj("raw GitHub workflow object", "id (numeric); the workflow file name also selects one"))]
    Get {
        id: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Dispatch a workflow
    #[command(after_long_help = outdoc::rest_delete())]
    Dispatch {
        id: String,
        #[arg(long)]
        r#ref: String,
        /// JSON object containing workflow inputs
        #[arg(long)]
        inputs_json: Option<String>,
    },
    /// Enable a workflow
    #[command(after_long_help = outdoc::rest_delete())]
    Enable { id: String },
    /// Disable a workflow
    #[command(after_long_help = outdoc::rest_delete())]
    Disable { id: String },
}

#[derive(Subcommand)]
enum RunCmd {
    /// List workflow runs
    #[command(after_long_help = outdoc::rest_list("raw GitHub workflow run objects", &["id"], &outdoc::GITHUB_PAGES))]
    List {
        /// Limit results to one workflow ID or file name
        #[arg(long)]
        workflow: Option<String>,
        #[arg(long)]
        actor: Option<String>,
        #[arg(long)]
        branch: Option<String>,
        #[arg(long)]
        event: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get a workflow run
    #[command(after_long_help = outdoc::rest_obj("raw GitHub workflow run object", "id"))]
    Get {
        id: Option<u64>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// List jobs in a workflow run
    #[command(after_long_help = outdoc::rest_list("raw GitHub workflow job objects", &[], &outdoc::GITHUB_PAGES))]
    Jobs {
        id: u64,
        #[arg(long)]
        filter: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Re-run a workflow run
    #[command(after_long_help = outdoc::rest_delete())]
    Rerun {
        id: u64,
        /// Re-run only failed jobs and their dependents
        #[arg(long)]
        failed: bool,
        /// Enable runner diagnostic logging
        #[arg(long)]
        debug: bool,
    },
    /// Cancel a workflow run
    #[command(after_long_help = outdoc::rest_delete())]
    Cancel {
        id: u64,
        /// Bypass conditions that would otherwise keep the run active
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum BranchCmd {
    /// List branches
    #[command(after_long_help = outdoc::rest_list("raw GitHub branch objects", &["name"], &outdoc::GITHUB_PAGES))]
    List {
        #[arg(long)]
        protected: Option<bool>,
        #[command(flatten)]
        page: Page,
    },
    /// Get a branch
    #[command(after_long_help = outdoc::rest_obj("raw GitHub branch object", "name"))]
    Get {
        name: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

#[derive(Subcommand)]
enum RefCmd {
    /// Create a git reference; bare names are created under refs/heads
    #[command(after_long_help = outdoc::rest_obj("raw GitHub git reference object", "ref"))]
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        sha: String,
    },
    /// Delete a git reference; bare names are resolved under heads
    #[command(after_long_help = outdoc::rest_delete())]
    Delete { name: String },
}

#[derive(Subcommand)]
enum BranchProtectionCmd {
    /// Get branch protection
    #[command(after_long_help = outdoc::rest_obj("raw GitHub branch protection object", ""))]
    Get {
        branch: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Replace branch protection using a native GitHub JSON object
    #[command(after_long_help = outdoc::rest_obj("raw GitHub branch protection object", ""))]
    Update {
        branch: String,
        #[arg(long)]
        rules_json: String,
    },
    /// Remove branch protection
    #[command(after_long_help = outdoc::rest_delete())]
    Delete { branch: String },
}

#[derive(Subcommand)]
enum CommitCmd {
    /// List commits
    #[command(after_long_help = outdoc::rest_list("raw GitHub commit objects", &["sha"], &outdoc::GITHUB_PAGES))]
    List {
        #[arg(long)]
        sha: Option<String>,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        author: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        until: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get a commit by ref or SHA
    #[command(after_long_help = outdoc::rest_obj("raw GitHub commit object", "sha"))]
    Get {
        r#ref: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
}

#[derive(Subcommand)]
enum CommitCommentCmd {
    /// List commit comments, optionally for one commit
    #[command(after_long_help = outdoc::rest_list("raw GitHub commit comment objects", &["id"], &outdoc::GITHUB_PAGES))]
    List {
        #[arg(long)]
        commit: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get a commit comment
    #[command(after_long_help = outdoc::rest_obj("raw GitHub commit comment object", "id"))]
    Get {
        id: Option<u64>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Create a commit comment
    #[command(after_long_help = outdoc::rest_obj("raw GitHub commit comment object", "id"))]
    Create {
        #[arg(long)]
        commit: String,
        #[command(flatten)]
        body: BodyInput,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        line: Option<u64>,
        #[arg(long)]
        position: Option<u64>,
    },
    /// Update a commit comment
    #[command(after_long_help = outdoc::rest_obj("raw GitHub commit comment object", "id"))]
    Update {
        id: u64,
        #[command(flatten)]
        body: BodyInput,
    },
    /// Delete a commit comment
    #[command(after_long_help = outdoc::rest_delete())]
    Delete { id: u64 },
}

#[derive(Subcommand)]
enum StatusCmd {
    /// List statuses for a commit
    #[command(after_long_help = outdoc::rest_list("raw GitHub commit status objects", &[], &outdoc::GITHUB_PAGES))]
    List {
        r#ref: String,
        #[command(flatten)]
        page: Page,
    },
    /// Create a commit status
    #[command(after_long_help = outdoc::rest_obj("raw GitHub commit status object", "id"))]
    Create {
        r#ref: String,
        #[arg(long)]
        state: String,
        #[arg(long)]
        context: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        target_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum CheckRunCmd {
    /// List check runs for a git ref
    #[command(after_long_help = outdoc::rest_list("raw GitHub check run objects", &["id"], &outdoc::GITHUB_PAGES))]
    List {
        r#ref: String,
        #[arg(long)]
        check_name: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        filter: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get a check run
    #[command(after_long_help = outdoc::rest_obj("raw GitHub check run object", "id"))]
    Get {
        id: Option<u64>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Request that a check run execute again
    #[command(after_long_help = outdoc::rest_delete())]
    Rerequest { id: u64 },
}

#[derive(Subcommand)]
enum CheckSuiteCmd {
    /// List check suites for a git ref
    #[command(after_long_help = outdoc::rest_list("raw GitHub check suite objects", &["id"], &outdoc::GITHUB_PAGES))]
    List {
        r#ref: String,
        #[command(flatten)]
        page: Page,
    },
    /// Get a check suite
    #[command(after_long_help = outdoc::rest_obj("raw GitHub check suite object", "id"))]
    Get {
        id: Option<u64>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Request that a check suite execute again
    #[command(after_long_help = outdoc::rest_delete())]
    Rerequest { id: u64 },
}

#[derive(Subcommand)]
enum ReleaseCmd {
    /// List releases
    #[command(after_long_help = outdoc::rest_list("raw GitHub release objects", &[], &outdoc::GITHUB_PAGES))]
    List {
        #[command(flatten)]
        page: Page,
    },
    /// Get a release by numeric ID or exact tag
    #[command(after_long_help = outdoc::rest_obj("raw GitHub release object", "id (database ID); tag_name also selects a release"))]
    Get {
        #[command(flatten)]
        selector: ReleaseSelector,
    },
    /// Create a release
    #[command(after_long_help = outdoc::rest_obj("raw GitHub release object", "id"))]
    Create {
        #[arg(long)]
        tag: String,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        body: BodyInput,
        #[arg(long)]
        draft: bool,
        #[arg(long)]
        prerelease: bool,
        #[arg(long)]
        generate_notes: bool,
    },
    /// Update a release; only supplied fields are changed
    #[command(after_long_help = outdoc::rest_obj("raw GitHub release object", "id"))]
    Update {
        id: u64,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        body: BodyInput,
        #[arg(long)]
        draft: Option<bool>,
        #[arg(long)]
        prerelease: Option<bool>,
    },
    /// Delete a release
    #[command(after_long_help = outdoc::rest_delete())]
    Delete { id: u64 },
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct ReleaseSelector {
    #[arg(long)]
    id: Option<u64>,
    #[arg(long)]
    tag: Option<String>,
}

#[derive(Subcommand)]
enum ReleaseAssetCmd {
    /// List release assets
    #[command(after_long_help = outdoc::rest_list("raw GitHub release asset objects", &["id"], &outdoc::GITHUB_PAGES))]
    List {
        #[arg(long)]
        release: u64,
        #[command(flatten)]
        page: Page,
    },
    /// Get release asset metadata
    #[command(after_long_help = outdoc::rest_obj("raw GitHub release asset object", "id"))]
    Get {
        id: Option<u64>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Delete a release asset
    #[command(after_long_help = outdoc::rest_delete())]
    Delete { id: u64 },
}

#[derive(Subcommand)]
enum ArtifactCmd {
    /// List repository artifacts
    #[command(after_long_help = outdoc::rest_list("raw GitHub artifact objects", &["id"], &outdoc::GITHUB_PAGES))]
    List {
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get artifact metadata
    #[command(after_long_help = outdoc::rest_obj("raw GitHub artifact object", "id"))]
    Get {
        id: Option<u64>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Delete an artifact
    #[command(after_long_help = outdoc::rest_delete())]
    Delete { id: u64 },
}

#[derive(Subcommand)]
enum LabelCmd {
    /// List labels
    #[command(after_long_help = outdoc::rest_list("raw GitHub label objects", &["name"], &outdoc::GITHUB_PAGES))]
    List {
        #[command(flatten)]
        page: Page,
    },
    /// Get a label
    #[command(after_long_help = outdoc::rest_obj("raw GitHub label object", "name"))]
    Get {
        name: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Create a label
    #[command(after_long_help = outdoc::rest_obj("raw GitHub label object", "name"))]
    Create {
        #[arg(long)]
        name: String,
        /// Six-character hex color without '#'
        #[arg(long)]
        color: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Update a label
    #[command(after_long_help = outdoc::rest_obj("raw GitHub label object", "name"))]
    Update {
        name: String,
        #[arg(long)]
        new_name: Option<String>,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a label
    #[command(after_long_help = outdoc::rest_delete())]
    Delete { name: String },
}

#[derive(Subcommand)]
enum CollaboratorCmd {
    /// List collaborators
    #[command(after_long_help = outdoc::rest_list("raw GitHub collaborator objects", &["login"], &outdoc::GITHUB_PAGES))]
    List {
        #[arg(long)]
        affiliation: Option<String>,
        #[arg(long)]
        permission: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get a collaborator's repository permissions
    #[command(after_long_help = outdoc::rest_obj("raw GitHub collaborator permission object", "user.login"))]
    Get {
        username: Option<String>,
        #[command(flatten)]
        from: FromFlag,
    },
    /// Add a collaborator or change their permission
    #[command(after_long_help = outdoc::lines(&[
        "A raw GitHub repository invitation object when an invitation is created; {} when the user is already a collaborator (GitHub returns no response body)",
    ]))]
    Add {
        username: String,
        #[arg(long)]
        permission: Option<String>,
    },
    /// Remove a collaborator
    #[command(after_long_help = outdoc::rest_delete())]
    Remove { username: String },
}

#[derive(Clone)]
struct Repo {
    owner: String,
    name: String,
}

#[derive(Clone, Copy)]
enum ListShape {
    Array,
    Key(&'static str),
    Issues,
}

macro_rules! path {
    ($repo:expr $(, $segment:expr)* $(,)?) => {{
        let mut segments = vec!["repos".to_owned(), $repo.owner.clone(), $repo.name.clone()];
        $(segments.push($segment.to_string());)*
        segments
    }};
}

pub fn run(
    cmd: Cmd,
    format: crate::output::Format,
    instance: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Cmd { repo, command } = cmd;
    let api = api(crate::auth::github_token(instance)?, format)?;
    match command {
        Resource::Repo(cmd) => run_repo(&api, repo, cmd),
        Resource::Issue(cmd) => run_issue(&api, selected_repo(repo)?, cmd),
        Resource::Comment(cmd) => run_comment(&api, selected_repo(repo)?, cmd),
        Resource::Pull(cmd) => run_pull(&api, selected_repo(repo)?, cmd),
        Resource::Review(cmd) => run_review(&api, selected_repo(repo)?, cmd),
        Resource::Workflow(cmd) => run_workflow(&api, selected_repo(repo)?, cmd),
        Resource::Run(cmd) => run_actions_run(&api, selected_repo(repo)?, cmd),
        Resource::Branch(cmd) => run_branch(&api, selected_repo(repo)?, cmd),
        Resource::Ref(cmd) => run_ref(&api, selected_repo(repo)?, cmd),
        Resource::BranchProtection(cmd) => run_branch_protection(&api, selected_repo(repo)?, cmd),
        Resource::Commit(cmd) => run_commit(&api, selected_repo(repo)?, cmd),
        Resource::CommitComment(cmd) => run_commit_comment(&api, selected_repo(repo)?, cmd),
        Resource::Status(cmd) => run_status(&api, selected_repo(repo)?, cmd),
        Resource::CheckRun(cmd) => run_check_run(&api, selected_repo(repo)?, cmd),
        Resource::CheckSuite(cmd) => run_check_suite(&api, selected_repo(repo)?, cmd),
        Resource::Release(cmd) => run_release(&api, selected_repo(repo)?, cmd),
        Resource::ReleaseAsset(cmd) => run_release_asset(&api, selected_repo(repo)?, cmd),
        Resource::Artifact(cmd) => run_artifact(&api, selected_repo(repo)?, cmd),
        Resource::Label(cmd) => run_label(&api, selected_repo(repo)?, cmd),
        Resource::Collaborator(cmd) => run_collaborator(&api, selected_repo(repo)?, cmd),
    }
}

pub fn authenticated() -> bool {
    crate::auth::github_token(crate::provider::DEFAULT_INSTANCE).is_ok()
        || crate::auth::vendor_has_stored_instances("github")
}

pub(crate) fn auth_identity(token: &str) -> Result<Value, crate::auth::ValidationError> {
    auth_identity_at(
        token,
        reqwest::Url::parse(&format!("{API_URL}user")).unwrap(),
    )
}

fn auth_identity_at(token: &str, url: reqwest::Url) -> Result<Value, crate::auth::ValidationError> {
    rest::identity(
        url,
        &Auth::Bearer(token.to_owned()),
        HEADERS,
        &[reqwest::StatusCode::UNAUTHORIZED],
    )
}

fn run_repo(
    api: &Api,
    repo: Option<String>,
    cmd: RepoCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        RepoCmd::List {
            visibility,
            affiliation,
            sort,
            direction,
            page,
        } => {
            let mut query = page_query(page);
            push_query(&mut query, "visibility", visibility);
            push_query(&mut query, "affiliation", affiliation);
            push_query(&mut query, "sort", sort);
            push_query(&mut query, "direction", direction);
            print_list(
                api,
                Method::GET,
                vec!["user".into(), "repos".into()],
                query,
                ListShape::Array,
            )
        }
        RepoCmd::Get => {
            let repo = selected_repo(repo)?;
            api.print(
                Method::GET,
                vec!["repos".into(), repo.owner, repo.name],
                Vec::new(),
                None,
            )
        }
    }
}

fn run_issue(api: &Api, repo: Repo, cmd: IssueCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        IssueCmd::List {
            state,
            assignee,
            creator,
            mentioned,
            milestone,
            labels,
            sort,
            direction,
            since,
            page,
        } => {
            let mut query = page_query(page);
            push_query(&mut query, "state", state);
            push_query(&mut query, "assignee", assignee);
            push_query(&mut query, "creator", creator);
            push_query(&mut query, "mentioned", mentioned);
            push_query(&mut query, "milestone", milestone);
            if !labels.is_empty() {
                query.push(("labels", labels.join(",")));
            }
            push_query(&mut query, "sort", sort);
            push_query(&mut query, "direction", direction);
            push_query(&mut query, "since", since);
            print_list(
                api,
                Method::GET,
                path!(&repo, "issues"),
                query,
                ListShape::Issues,
            )
        }
        IssueCmd::Get { number, from } => pipe::run_get(number, from, api.format, |number| {
            api.get_body(path!(&repo, "issues", number), Vec::new())
        }),
        IssueCmd::Create {
            title,
            body,
            assignees,
            labels,
            milestone,
        } => {
            let mut payload = Map::new();
            payload.insert("title".into(), title.into());
            insert_opt(&mut payload, "body", body.read()?);
            insert_vec(&mut payload, "assignees", assignees);
            insert_vec(&mut payload, "labels", labels);
            insert_opt(&mut payload, "milestone", milestone);
            api.print(
                Method::POST,
                path!(&repo, "issues"),
                Vec::new(),
                Some(payload.into()),
            )
        }
        IssueCmd::Update {
            number,
            title,
            body,
            state,
            state_reason,
            assignees,
            clear_assignees,
            labels,
            clear_labels,
            milestone,
            clear_milestone,
        } => {
            let mut payload = Map::new();
            insert_opt(&mut payload, "title", title);
            insert_opt(&mut payload, "body", body.read()?);
            insert_opt(&mut payload, "state", state);
            insert_opt(&mut payload, "state_reason", state_reason);
            insert_update_vec(&mut payload, "assignees", assignees, clear_assignees);
            insert_update_vec(&mut payload, "labels", labels, clear_labels);
            if clear_milestone {
                payload.insert("milestone".into(), Value::Null);
            } else {
                insert_opt(&mut payload, "milestone", milestone);
            }
            api.print(
                Method::PATCH,
                path!(&repo, "issues", number),
                Vec::new(),
                Some(payload.into()),
            )
        }
    }
}

fn run_comment(api: &Api, repo: Repo, cmd: CommentCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CommentCmd::List { issue, page } => print_list(
            api,
            Method::GET,
            path!(&repo, "issues", issue, "comments"),
            page_query(page),
            ListShape::Array,
        ),
        CommentCmd::Create { issue, body } => api.print(
            Method::POST,
            path!(&repo, "issues", issue, "comments"),
            Vec::new(),
            Some(json!({ "body": body.required()? })),
        ),
        CommentCmd::Update { id, body } => api.print(
            Method::PATCH,
            path!(&repo, "issues", "comments", id),
            Vec::new(),
            Some(json!({ "body": body.required()? })),
        ),
        CommentCmd::Delete { id } => api.print(
            Method::DELETE,
            path!(&repo, "issues", "comments", id),
            Vec::new(),
            None,
        ),
    }
}

fn run_pull(api: &Api, repo: Repo, cmd: PullCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PullCmd::List {
            state,
            head,
            base,
            sort,
            direction,
            page,
        } => {
            let mut query = page_query(page);
            push_query(&mut query, "state", state);
            push_query(&mut query, "head", head);
            push_query(&mut query, "base", base);
            push_query(&mut query, "sort", sort);
            push_query(&mut query, "direction", direction);
            print_list(
                api,
                Method::GET,
                path!(&repo, "pulls"),
                query,
                ListShape::Array,
            )
        }
        PullCmd::Get { number, from } => pipe::run_get(number, from, api.format, |number| {
            api.get_body(path!(&repo, "pulls", number), Vec::new())
        }),
        PullCmd::Create {
            title,
            head,
            base,
            body,
            draft,
            maintainer_can_modify,
        } => {
            let mut payload = Map::new();
            payload.insert("title".into(), title.into());
            payload.insert("head".into(), head.into());
            payload.insert("base".into(), base.into());
            insert_opt(&mut payload, "body", body.read()?);
            if draft {
                payload.insert("draft".into(), true.into());
            }
            if maintainer_can_modify {
                payload.insert("maintainer_can_modify".into(), true.into());
            }
            api.print(
                Method::POST,
                path!(&repo, "pulls"),
                Vec::new(),
                Some(payload.into()),
            )
        }
        PullCmd::Update {
            number,
            title,
            body,
            state,
            base,
            maintainer_can_modify,
        } => {
            let mut payload = Map::new();
            insert_opt(&mut payload, "title", title);
            insert_opt(&mut payload, "body", body.read()?);
            insert_opt(&mut payload, "state", state);
            insert_opt(&mut payload, "base", base);
            insert_opt(&mut payload, "maintainer_can_modify", maintainer_can_modify);
            api.print(
                Method::PATCH,
                path!(&repo, "pulls", number),
                Vec::new(),
                Some(payload.into()),
            )
        }
        PullCmd::Files { number, page } => print_list(
            api,
            Method::GET,
            path!(&repo, "pulls", number, "files"),
            page_query(page),
            ListShape::Array,
        ),
        PullCmd::Merge {
            number,
            method,
            commit_title,
            commit_message,
            sha,
        } => {
            let mut payload = Map::new();
            insert_opt(&mut payload, "merge_method", method);
            insert_opt(&mut payload, "commit_title", commit_title);
            insert_opt(&mut payload, "commit_message", commit_message);
            insert_opt(&mut payload, "sha", sha);
            api.print(
                Method::PUT,
                path!(&repo, "pulls", number, "merge"),
                Vec::new(),
                Some(payload.into()),
            )
        }
    }
}

fn run_review(api: &Api, repo: Repo, cmd: ReviewCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ReviewCmd::List { pull, page } => print_list(
            api,
            Method::GET,
            path!(&repo, "pulls", pull, "reviews"),
            page_query(page),
            ListShape::Array,
        ),
        ReviewCmd::Create {
            pull,
            event,
            body,
            commit_id,
            comments_json,
        } => {
            let mut payload = Map::new();
            insert_opt(
                &mut payload,
                "event",
                event.map(|event| event.api_value().to_owned()),
            );
            insert_opt(&mut payload, "body", body.read()?);
            insert_opt(&mut payload, "commit_id", commit_id);
            if let Some(comments) = comments_json {
                payload.insert(
                    "comments".into(),
                    parse_json_array("--comments-json", &comments)?,
                );
            }
            api.print(
                Method::POST,
                path!(&repo, "pulls", pull, "reviews"),
                Vec::new(),
                Some(payload.into()),
            )
        }
        ReviewCmd::Submit {
            pull,
            id,
            event,
            body,
        } => {
            let mut payload = Map::new();
            payload.insert("event".into(), event.api_value().into());
            insert_opt(&mut payload, "body", body.read()?);
            api.print(
                Method::POST,
                path!(&repo, "pulls", pull, "reviews", id, "events"),
                Vec::new(),
                Some(payload.into()),
            )
        }
        ReviewCmd::Delete { pull, id } => api.print(
            Method::DELETE,
            path!(&repo, "pulls", pull, "reviews", id),
            Vec::new(),
            None,
        ),
    }
}

fn run_workflow(api: &Api, repo: Repo, cmd: WorkflowCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        WorkflowCmd::List { page } => print_list(
            api,
            Method::GET,
            path!(&repo, "actions", "workflows"),
            page_query(page),
            ListShape::Key("workflows"),
        ),
        WorkflowCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!(&repo, "actions", "workflows", id), Vec::new())
        }),
        WorkflowCmd::Dispatch {
            id,
            r#ref,
            inputs_json,
        } => {
            let mut payload = Map::new();
            payload.insert("ref".into(), r#ref.into());
            if let Some(inputs) = inputs_json {
                payload.insert(
                    "inputs".into(),
                    parse_json_object("--inputs-json", &inputs)?,
                );
            }
            api.print(
                Method::POST,
                path!(&repo, "actions", "workflows", id, "dispatches"),
                Vec::new(),
                Some(payload.into()),
            )
        }
        WorkflowCmd::Enable { id } => api.print(
            Method::PUT,
            path!(&repo, "actions", "workflows", id, "enable"),
            Vec::new(),
            None,
        ),
        WorkflowCmd::Disable { id } => api.print(
            Method::PUT,
            path!(&repo, "actions", "workflows", id, "disable"),
            Vec::new(),
            None,
        ),
    }
}

fn run_actions_run(api: &Api, repo: Repo, cmd: RunCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        RunCmd::List {
            workflow,
            actor,
            branch,
            event,
            status,
            page,
        } => {
            let segments = if let Some(workflow) = workflow {
                path!(&repo, "actions", "workflows", workflow, "runs")
            } else {
                path!(&repo, "actions", "runs")
            };
            let mut query = page_query(page);
            push_query(&mut query, "actor", actor);
            push_query(&mut query, "branch", branch);
            push_query(&mut query, "event", event);
            push_query(&mut query, "status", status);
            print_list(
                api,
                Method::GET,
                segments,
                query,
                ListShape::Key("workflow_runs"),
            )
        }
        RunCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!(&repo, "actions", "runs", id), Vec::new())
        }),
        RunCmd::Jobs { id, filter, page } => {
            let mut query = page_query(page);
            push_query(&mut query, "filter", filter);
            print_list(
                api,
                Method::GET,
                path!(&repo, "actions", "runs", id, "jobs"),
                query,
                ListShape::Key("jobs"),
            )
        }
        RunCmd::Rerun { id, failed, debug } => api.print(
            Method::POST,
            if failed {
                path!(&repo, "actions", "runs", id, "rerun-failed-jobs")
            } else {
                path!(&repo, "actions", "runs", id, "rerun")
            },
            Vec::new(),
            Some(json!({ "enable_debug_logging": debug })),
        ),
        RunCmd::Cancel { id, force } => api.print(
            Method::POST,
            if force {
                path!(&repo, "actions", "runs", id, "force-cancel")
            } else {
                path!(&repo, "actions", "runs", id, "cancel")
            },
            Vec::new(),
            None,
        ),
    }
}

fn run_branch(api: &Api, repo: Repo, cmd: BranchCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        BranchCmd::List { protected, page } => {
            let mut query = page_query(page);
            push_query(&mut query, "protected", protected);
            print_list(
                api,
                Method::GET,
                path!(&repo, "branches"),
                query,
                ListShape::Array,
            )
        }
        BranchCmd::Get { name, from } => pipe::run_get(name, from, api.format, |name| {
            api.get_body(path!(&repo, "branches", name), Vec::new())
        }),
    }
}

fn run_ref(api: &Api, repo: Repo, cmd: RefCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        RefCmd::Create { name, sha } => api.print(
            Method::POST,
            path!(&repo, "git", "refs"),
            Vec::new(),
            Some(json!({ "ref": full_ref(&name), "sha": sha })),
        ),
        RefCmd::Delete { name } => api.print(
            Method::DELETE,
            path!(&repo, "git", "refs", short_ref(&name)),
            Vec::new(),
            None,
        ),
    }
}

fn run_branch_protection(
    api: &Api,
    repo: Repo,
    cmd: BranchProtectionCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        BranchProtectionCmd::Get { branch, from } => {
            pipe::run_get(branch, from, api.format, |branch| {
                api.get_body(path!(&repo, "branches", branch, "protection"), Vec::new())
            })
        }
        BranchProtectionCmd::Update { branch, rules_json } => api.print(
            Method::PUT,
            path!(&repo, "branches", branch, "protection"),
            Vec::new(),
            Some(parse_json_object("--rules-json", &rules_json)?),
        ),
        BranchProtectionCmd::Delete { branch } => api.print(
            Method::DELETE,
            path!(&repo, "branches", branch, "protection"),
            Vec::new(),
            None,
        ),
    }
}

fn run_commit(api: &Api, repo: Repo, cmd: CommitCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CommitCmd::List {
            sha,
            path: file_path,
            author,
            since,
            until,
            page,
        } => {
            let mut query = page_query(page);
            push_query(&mut query, "sha", sha);
            push_query(&mut query, "path", file_path);
            push_query(&mut query, "author", author);
            push_query(&mut query, "since", since);
            push_query(&mut query, "until", until);
            print_list(
                api,
                Method::GET,
                path!(&repo, "commits"),
                query,
                ListShape::Array,
            )
        }
        CommitCmd::Get { r#ref, from } => pipe::run_get(r#ref, from, api.format, |r#ref| {
            api.get_body(path!(&repo, "commits", r#ref), Vec::new())
        }),
    }
}

fn run_commit_comment(
    api: &Api,
    repo: Repo,
    cmd: CommitCommentCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CommitCommentCmd::List { commit, page } => print_list(
            api,
            Method::GET,
            if let Some(commit) = commit {
                path!(&repo, "commits", commit, "comments")
            } else {
                path!(&repo, "comments")
            },
            page_query(page),
            ListShape::Array,
        ),
        CommitCommentCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!(&repo, "comments", id), Vec::new())
        }),
        CommitCommentCmd::Create {
            commit,
            body,
            path: file_path,
            line,
            position,
        } => {
            let mut payload = Map::new();
            payload.insert("body".into(), body.required()?.into());
            insert_opt(&mut payload, "path", file_path);
            insert_opt(&mut payload, "line", line);
            insert_opt(&mut payload, "position", position);
            api.print(
                Method::POST,
                path!(&repo, "commits", commit, "comments"),
                Vec::new(),
                Some(payload.into()),
            )
        }
        CommitCommentCmd::Update { id, body } => api.print(
            Method::PATCH,
            path!(&repo, "comments", id),
            Vec::new(),
            Some(json!({ "body": body.required()? })),
        ),
        CommitCommentCmd::Delete { id } => api.print(
            Method::DELETE,
            path!(&repo, "comments", id),
            Vec::new(),
            None,
        ),
    }
}

fn run_status(api: &Api, repo: Repo, cmd: StatusCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        StatusCmd::List { r#ref, page } => print_list(
            api,
            Method::GET,
            path!(&repo, "commits", r#ref, "statuses"),
            page_query(page),
            ListShape::Array,
        ),
        StatusCmd::Create {
            r#ref,
            state,
            context,
            description,
            target_url,
        } => {
            let mut payload = Map::new();
            payload.insert("state".into(), state.into());
            insert_opt(&mut payload, "context", context);
            insert_opt(&mut payload, "description", description);
            insert_opt(&mut payload, "target_url", target_url);
            api.print(
                Method::POST,
                path!(&repo, "statuses", r#ref),
                Vec::new(),
                Some(payload.into()),
            )
        }
    }
}

fn run_check_run(
    api: &Api,
    repo: Repo,
    cmd: CheckRunCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CheckRunCmd::List {
            r#ref,
            check_name,
            status,
            filter,
            page,
        } => {
            let mut query = page_query(page);
            push_query(&mut query, "check_name", check_name);
            push_query(&mut query, "status", status);
            push_query(&mut query, "filter", filter);
            print_list(
                api,
                Method::GET,
                path!(&repo, "commits", r#ref, "check-runs"),
                query,
                ListShape::Key("check_runs"),
            )
        }
        CheckRunCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!(&repo, "check-runs", id), Vec::new())
        }),
        CheckRunCmd::Rerequest { id } => api.print(
            Method::POST,
            path!(&repo, "check-runs", id, "rerequest"),
            Vec::new(),
            None,
        ),
    }
}

fn run_check_suite(
    api: &Api,
    repo: Repo,
    cmd: CheckSuiteCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CheckSuiteCmd::List { r#ref, page } => print_list(
            api,
            Method::GET,
            path!(&repo, "commits", r#ref, "check-suites"),
            page_query(page),
            ListShape::Key("check_suites"),
        ),
        CheckSuiteCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!(&repo, "check-suites", id), Vec::new())
        }),
        CheckSuiteCmd::Rerequest { id } => api.print(
            Method::POST,
            path!(&repo, "check-suites", id, "rerequest"),
            Vec::new(),
            None,
        ),
    }
}

fn run_release(api: &Api, repo: Repo, cmd: ReleaseCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ReleaseCmd::List { page } => print_list(
            api,
            Method::GET,
            path!(&repo, "releases"),
            page_query(page),
            ListShape::Array,
        ),
        ReleaseCmd::Get {
            selector:
                ReleaseSelector {
                    id: Some(id),
                    tag: None,
                },
        } => api.print(Method::GET, path!(&repo, "releases", id), Vec::new(), None),
        ReleaseCmd::Get {
            selector:
                ReleaseSelector {
                    id: None,
                    tag: Some(tag),
                },
        } => api.print(
            Method::GET,
            path!(&repo, "releases", "tags", tag),
            Vec::new(),
            None,
        ),
        ReleaseCmd::Get { .. } => unreachable!("clap enforces --id xor --tag"),
        ReleaseCmd::Create {
            tag,
            target,
            name,
            body,
            draft,
            prerelease,
            generate_notes,
        } => {
            let mut payload = Map::new();
            payload.insert("tag_name".into(), tag.into());
            insert_opt(&mut payload, "target_commitish", target);
            insert_opt(&mut payload, "name", name);
            insert_opt(&mut payload, "body", body.read()?);
            payload.insert("draft".into(), draft.into());
            payload.insert("prerelease".into(), prerelease.into());
            payload.insert("generate_release_notes".into(), generate_notes.into());
            api.print(
                Method::POST,
                path!(&repo, "releases"),
                Vec::new(),
                Some(payload.into()),
            )
        }
        ReleaseCmd::Update {
            id,
            tag,
            target,
            name,
            body,
            draft,
            prerelease,
        } => {
            let mut payload = Map::new();
            insert_opt(&mut payload, "tag_name", tag);
            insert_opt(&mut payload, "target_commitish", target);
            insert_opt(&mut payload, "name", name);
            insert_opt(&mut payload, "body", body.read()?);
            insert_opt(&mut payload, "draft", draft);
            insert_opt(&mut payload, "prerelease", prerelease);
            api.print(
                Method::PATCH,
                path!(&repo, "releases", id),
                Vec::new(),
                Some(payload.into()),
            )
        }
        ReleaseCmd::Delete { id } => api.print(
            Method::DELETE,
            path!(&repo, "releases", id),
            Vec::new(),
            None,
        ),
    }
}

fn run_release_asset(
    api: &Api,
    repo: Repo,
    cmd: ReleaseAssetCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ReleaseAssetCmd::List { release, page } => print_list(
            api,
            Method::GET,
            path!(&repo, "releases", release, "assets"),
            page_query(page),
            ListShape::Array,
        ),
        ReleaseAssetCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!(&repo, "releases", "assets", id), Vec::new())
        }),
        ReleaseAssetCmd::Delete { id } => api.print(
            Method::DELETE,
            path!(&repo, "releases", "assets", id),
            Vec::new(),
            None,
        ),
    }
}

fn run_artifact(api: &Api, repo: Repo, cmd: ArtifactCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ArtifactCmd::List { name, page } => {
            let mut query = page_query(page);
            push_query(&mut query, "name", name);
            print_list(
                api,
                Method::GET,
                path!(&repo, "actions", "artifacts"),
                query,
                ListShape::Key("artifacts"),
            )
        }
        ArtifactCmd::Get { id, from } => pipe::run_get(id, from, api.format, |id| {
            api.get_body(path!(&repo, "actions", "artifacts", id), Vec::new())
        }),
        ArtifactCmd::Delete { id } => api.print(
            Method::DELETE,
            path!(&repo, "actions", "artifacts", id),
            Vec::new(),
            None,
        ),
    }
}

fn run_label(api: &Api, repo: Repo, cmd: LabelCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        LabelCmd::List { page } => print_list(
            api,
            Method::GET,
            path!(&repo, "labels"),
            page_query(page),
            ListShape::Array,
        ),
        LabelCmd::Get { name, from } => pipe::run_get(name, from, api.format, |name| {
            api.get_body(path!(&repo, "labels", name), Vec::new())
        }),
        LabelCmd::Create {
            name,
            color,
            description,
        } => {
            let mut payload = Map::new();
            payload.insert("name".into(), name.into());
            payload.insert("color".into(), color.into());
            insert_opt(&mut payload, "description", description);
            api.print(
                Method::POST,
                path!(&repo, "labels"),
                Vec::new(),
                Some(payload.into()),
            )
        }
        LabelCmd::Update {
            name,
            new_name,
            color,
            description,
        } => {
            let mut payload = Map::new();
            insert_opt(&mut payload, "new_name", new_name);
            insert_opt(&mut payload, "color", color);
            insert_opt(&mut payload, "description", description);
            api.print(
                Method::PATCH,
                path!(&repo, "labels", name),
                Vec::new(),
                Some(payload.into()),
            )
        }
        LabelCmd::Delete { name } => api.print(
            Method::DELETE,
            path!(&repo, "labels", name),
            Vec::new(),
            None,
        ),
    }
}

fn run_collaborator(
    api: &Api,
    repo: Repo,
    cmd: CollaboratorCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CollaboratorCmd::List {
            affiliation,
            permission,
            page,
        } => {
            let mut query = page_query(page);
            push_query(&mut query, "affiliation", affiliation);
            push_query(&mut query, "permission", permission);
            print_list(
                api,
                Method::GET,
                path!(&repo, "collaborators"),
                query,
                ListShape::Array,
            )
        }
        CollaboratorCmd::Get { username, from } => {
            pipe::run_get(username, from, api.format, |username| {
                api.get_body(
                    path!(&repo, "collaborators", username, "permission"),
                    Vec::new(),
                )
            })
        }
        CollaboratorCmd::Add {
            username,
            permission,
        } => {
            let mut payload = Map::new();
            insert_opt(&mut payload, "permission", permission);
            api.print(
                Method::PUT,
                path!(&repo, "collaborators", username),
                Vec::new(),
                Some(payload.into()),
            )
        }
        CollaboratorCmd::Remove { username } => api.print(
            Method::DELETE,
            path!(&repo, "collaborators", username),
            Vec::new(),
            None,
        ),
    }
}

fn api(token: String, format: crate::output::Format) -> Result<Api, Box<dyn std::error::Error>> {
    Ok(Api {
        client: reqwest::blocking::Client::new(),
        base_url: reqwest::Url::parse(API_URL)?,
        auth: Auth::Bearer(token),
        format,
        headers: HEADERS,
        trailing_slash: false,
    })
}

fn print_list(
    api: &Api,
    method: Method,
    segments: Vec<String>,
    query: Vec<(&'static str, String)>,
    shape: ListShape,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = api.send(method, &segments, &query, None)?;
    let items = list_items(response.body, shape)?;
    crate::output::print(
        &rest::wrap_list(items, page_info(response.link.as_deref())),
        api.format,
    );
    Ok(())
}

fn selected_repo(explicit: Option<String>) -> Result<Repo, Box<dyn std::error::Error>> {
    if let Some(repo) = explicit {
        return parse_repo(&repo).ok_or_else(|| "--repo must be OWNER/NAME".into());
    }
    let origin = ProcessCommand::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|url| parse_github_remote(url.trim()));
    if let Some(repo) = origin {
        return Ok(repo);
    }
    let remotes = ProcessCommand::new("git")
        .args(["remote", "-v"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok());
    if let Some(repo) = remotes.and_then(|remotes| {
        remotes
            .lines()
            .filter_map(|line| line.split_whitespace().nth(1))
            .find_map(parse_github_remote)
    }) {
        return Ok(repo);
    }
    Err("--repo is required outside a git checkout with a github.com remote".into())
}

fn parse_repo(value: &str) -> Option<Repo> {
    let value = value
        .trim_matches('/')
        .strip_suffix(".git")
        .unwrap_or(value.trim_matches('/'));
    let mut parts = value.split('/');
    let owner = parts.next()?;
    let name = parts.next()?;
    if owner.is_empty() || name.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(Repo {
        owner: owner.to_owned(),
        name: name.to_owned(),
    })
}

fn parse_github_remote(value: &str) -> Option<Repo> {
    let path = value
        .strip_prefix("https://github.com/")
        .or_else(|| value.strip_prefix("http://github.com/"))
        .or_else(|| value.strip_prefix("ssh://git@github.com/"))
        .or_else(|| value.strip_prefix("git@github.com:"))?;
    parse_repo(path)
}

fn page_query(page: Page) -> Vec<(&'static str, String)> {
    vec![
        ("per_page", page.limit.to_string()),
        ("page", page.page.to_string()),
    ]
}

fn insert_vec(object: &mut Map<String, Value>, name: &str, values: Vec<String>) {
    if !values.is_empty() {
        object.insert(name.to_owned(), values.into());
    }
}

fn insert_update_vec(
    object: &mut Map<String, Value>,
    name: &str,
    values: Vec<String>,
    clear: bool,
) {
    if clear || !values.is_empty() {
        object.insert(name.to_owned(), values.into());
    }
}

fn parse_json_array(flag: &str, input: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let value: Value = serde_json::from_str(input)?;
    if !value.is_array() {
        return Err(format!("{flag} must be a JSON array").into());
    }
    Ok(value)
}

fn parse_json_object(flag: &str, input: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let value: Value = serde_json::from_str(input)?;
    if !value.is_object() {
        return Err(format!("{flag} must be a JSON object").into());
    }
    Ok(value)
}

fn list_items(mut body: Value, shape: ListShape) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let items = match shape {
        ListShape::Array | ListShape::Issues => body
            .as_array_mut()
            .map(std::mem::take)
            .ok_or("GitHub list response was not an array")?,
        ListShape::Key(key) => body
            .as_object_mut()
            .and_then(|object| object.remove(key))
            .and_then(|value| value.as_array().cloned())
            .ok_or_else(|| format!("GitHub list response did not contain an array at {key}"))?,
    };
    if matches!(shape, ListShape::Issues) {
        Ok(items
            .into_iter()
            .filter(|item| item.get("pull_request").is_none())
            .collect())
    } else {
        Ok(items)
    }
}

fn page_info(link: Option<&str>) -> Value {
    let next = link_page(link, "next");
    let previous = link_page(link, "prev");
    json!({
        "hasNextPage": next.is_some(),
        "nextPage": next,
        "hasPreviousPage": previous.is_some(),
        "previousPage": previous,
    })
}

fn link_page(link: Option<&str>, relation: &str) -> Option<u32> {
    link?.split(',').find_map(|part| {
        let (url, metadata) = part.trim().split_once(';')?;
        if !metadata
            .split(';')
            .any(|item| item.trim() == format!("rel=\"{relation}\""))
        {
            return None;
        }
        reqwest::Url::parse(url.trim().trim_start_matches('<').trim_end_matches('>'))
            .ok()?
            .query_pairs()
            .find(|(key, _)| key == "page")?
            .1
            .parse()
            .ok()
    })
}

fn full_ref(name: &str) -> String {
    if name.starts_with("refs/") {
        name.to_owned()
    } else {
        format!("refs/heads/{name}")
    }
}

fn short_ref(name: &str) -> String {
    if let Some(name) = name.strip_prefix("refs/") {
        name.to_owned()
    } else if name.starts_with("heads/") || name.starts_with("tags/") {
        name.to_owned()
    } else {
        format!("heads/{name}")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn parses_supported_github_remotes() {
        for remote in [
            "https://github.com/owner/repo.git",
            "git@github.com:owner/repo.git",
            "ssh://git@github.com/owner/repo",
        ] {
            let repo = parse_github_remote(remote).unwrap();
            assert_eq!(repo.owner, "owner");
            assert_eq!(repo.name, "repo");
        }
        assert!(parse_github_remote("https://example.com/owner/repo").is_none());
        assert!(parse_repo("owner/repo/extra").is_none());
    }

    #[test]
    fn wraps_link_pagination_and_excludes_pull_requests_from_issues() {
        let link = concat!(
            "<https://api.github.com/repositories/1/issues?page=3>; rel=\"next\", ",
            "<https://api.github.com/repositories/1/issues?page=1>; rel=\"prev\""
        );
        assert_eq!(page_info(Some(link))["nextPage"], 3);
        assert_eq!(page_info(Some(link))["previousPage"], 1);

        let items = list_items(
            json!([
                { "number": 1, "title": "Issue" },
                { "number": 2, "title": "PR", "pull_request": { "url": "..." } }
            ]),
            ListShape::Issues,
        )
        .unwrap();
        assert_eq!(items, vec![json!({ "number": 1, "title": "Issue" })]);
    }

    #[test]
    fn reads_markdown_body_from_a_file() {
        let path = std::env::temp_dir().join(format!("foac-github-body-{}.md", std::process::id()));
        std::fs::write(&path, "A body from disk\n").unwrap();
        let body = BodyInput {
            body: None,
            body_file: Some(path.clone()),
        }
        .required()
        .unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(body, "A body from disk\n");
    }

    #[test]
    fn sends_github_headers_and_parses_json_response() {
        let (api, request_rx, server) = test_api("200 OK", "{\"ok\":true}", "");
        let response = api
            .send(
                Method::GET,
                &["repos".into(), "owner".into(), "repo".into()],
                &[("page", "2".into())],
                None,
            )
            .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /repos/owner/repo?page=2 http/1.1"));
        assert!(request.contains("authorization: bearer secret-token"));
        assert!(request.contains("x-github-api-version: 2026-03-10"));
        assert!(request.contains("accept: application/vnd.github+json"));
        assert_eq!(response.body, json!({ "ok": true }));
    }

    #[test]
    fn validates_github_identity_and_rejects_bad_credentials() {
        let (api, request_rx, server) = test_api(
            "200 OK",
            r#"{"id":1,"login":"octocat","name":"The Octocat"}"#,
            "",
        );
        let identity =
            auth_identity_at("status-token", api.base_url.join("user").unwrap()).unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /user http/1.1"));
        assert!(request.contains("authorization: bearer status-token"));
        assert_eq!(identity["login"], "octocat");

        let (api, _, server) = test_api("401 Unauthorized", r#"{"message":"bad credentials"}"#, "");
        let error = auth_identity_at("bad-token", api.base_url.join("user").unwrap()).unwrap_err();
        server.join().unwrap();
        assert!(matches!(error, crate::auth::ValidationError::Rejected(_)));
    }

    #[test]
    fn represents_empty_success_responses_as_json() {
        let (api, _, server) = test_api("204 No Content", "", "");
        let response = api
            .send(Method::DELETE, &["resource".into()], &[], None)
            .unwrap();
        server.join().unwrap();
        assert_eq!(response.body, json!({}));
    }

    #[test]
    fn propagates_github_error_json() {
        let (api, _, server) =
            test_api("422 Unprocessable Entity", "{\"message\":\"invalid\"}", "");
        let error = api
            .send(Method::POST, &["resource".into()], &[], Some(json!({})))
            .unwrap_err();
        server.join().unwrap();
        assert_eq!(error.to_string(), "{\"message\":\"invalid\"}");
    }

    #[test]
    fn serializes_json_mutation_payloads() {
        let (api, request_rx, server) = test_api("201 Created", "{}", "");
        api.send(
            Method::POST,
            &["issues".into()],
            &[],
            Some(json!({ "title": "Fix it", "draft": false })),
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({ "title": "Fix it", "draft": false })
        );
    }

    #[test]
    fn reads_pagination_from_http_link_header() {
        let (api, _, server) = test_api(
            "200 OK",
            "[{\"id\":1}]",
            "Link: <https://api.github.com/resources?page=3>; rel=\"next\"\r\n",
        );
        let response = api
            .send(Method::GET, &["resources".into()], &[], None)
            .unwrap();
        server.join().unwrap();

        assert_eq!(
            list_items(response.body, ListShape::Array).unwrap(),
            vec![json!({ "id": 1 })]
        );
        assert_eq!(page_info(response.link.as_deref())["nextPage"], 3);
    }

    #[test]
    fn issue_update_can_clear_assignees_and_labels() {
        let (api, request_rx, server) = test_api("200 OK", "{}", "");
        run_issue(
            &api,
            Repo {
                owner: "owner".into(),
                name: "repo".into(),
            },
            IssueCmd::Update {
                number: 42,
                title: None,
                body: BodyInput::default(),
                state: None,
                state_reason: None,
                assignees: Vec::new(),
                clear_assignees: true,
                labels: Vec::new(),
                clear_labels: true,
                milestone: None,
                clear_milestone: true,
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({ "assignees": [], "labels": [], "milestone": null })
        );
    }

    #[test]
    fn numeric_release_tags_use_the_tag_endpoint() {
        let (api, request_rx, server) = test_api("200 OK", "{}", "");
        run_release(
            &api,
            Repo {
                owner: "owner".into(),
                name: "repo".into(),
            },
            ReleaseCmd::Get {
                selector: ReleaseSelector {
                    id: None,
                    tag: Some("123".into()),
                },
            },
        )
        .unwrap();
        server.join().unwrap();

        let request = request_rx.recv().unwrap();
        assert!(request.starts_with("GET /repos/owner/repo/releases/tags/123 HTTP/1.1"));
    }

    fn test_api(
        status: &str,
        body: &str,
        extra_headers: &str,
    ) -> (Api, mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
        let (url, request_rx, server) = rest::testing::test_server(status, body, extra_headers);
        let api = Api {
            client: reqwest::blocking::Client::new(),
            format: crate::output::Format::Json,
            base_url: url,
            auth: Auth::Bearer("secret-token".into()),
            headers: HEADERS,
            trailing_slash: false,
        };
        (api, request_rx, server)
    }
}
