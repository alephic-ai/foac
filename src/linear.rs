use graphql_client::GraphQLQuery;

const API_URL: &str = "https://api.linear.app/graphql";

// Linear custom scalars, mapped to plain strings/JSON since we never compute on them.
type DateTime = String;
type TimelessDate = String;
type Duration = String;
type DateTimeOrDuration = String;
type TimelessDateOrDuration = String;
#[allow(clippy::upper_case_acronyms)]
type JSONObject = serde_json::Value;
#[allow(clippy::upper_case_acronyms)]
type JSON = serde_json::Value;

macro_rules! linear_query {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(GraphQLQuery)]
        #[graphql(
            schema_path = "graphql/linear/schema.graphql",
            query_path = "graphql/linear/queries.graphql",
            variables_derives = "Clone, Default",
            skip_serializing_none
        )]
        struct $name;
    )+};
}

linear_query!(
    WorkspaceGet,
    IssueList,
    IssueGet,
    IssueCreate,
    IssueUpdate,
    CommentList,
    CommentCreate,
    CommentUpdate,
    CommentDelete,
    ProjectList,
    ProjectGet,
    ProjectCreate,
    ProjectModify,
    TeamList,
    TeamGet,
    UserList,
    UserGet,
    CycleList,
    LabelList,
    LabelCreate,
    StatusList,
    StatusGet,
    DocumentList,
    DocumentGet,
    DocumentCreate,
    DocumentUpdate,
    InitiativeList,
    InitiativeGet,
    InitiativeCreate,
    InitiativeModify,
    MilestoneList,
    MilestoneGet,
    MilestoneCreate,
    MilestoneUpdate,
    ProjectLabelList,
    StatusUpdateGet,
    StatusUpdateCreate,
    StatusUpdateModify,
    StatusUpdateArchive,
    InitiativeStatusUpdateGet,
    InitiativeStatusUpdateCreate,
    InitiativeStatusUpdateModify,
    InitiativeStatusUpdateArchive,
    AttachmentGet,
    AttachmentCreate,
    AttachmentDelete,
);

/// Build `<Filter> { id: {eq} }` when the value looks like a UUID, else
/// `<Filter> { <alt field>: {eq} }` — lets flags accept an ID or a human name/key.
macro_rules! eq_filter {
    ($m:ident, $f:ident, $idc:ident, $alt:ident, $v:expr) => {{
        let v: String = $v;
        if is_uuid(&v) {
            $m::$f {
                id: Some($m::$idc {
                    eq: Some(v),
                    ..Default::default()
                }),
                ..Default::default()
            }
        } else {
            $m::$f {
                $alt: Some($m::StringComparator {
                    eq: Some(v),
                    ..Default::default()
                }),
                ..Default::default()
            }
        }
    }};
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.bytes().filter(|b| *b == b'-').count() == 4
}

#[derive(clap::Subcommand)]
pub enum Cmd {
    /// Issues
    #[command(subcommand)]
    Issue(IssueCmd),
    /// Comments on issues
    #[command(subcommand)]
    Comment(CommentCmd),
    /// Projects
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Teams
    #[command(subcommand)]
    Team(TeamCmd),
    /// Users
    #[command(subcommand)]
    User(UserCmd),
    /// Workspace (organization) info
    #[command(subcommand)]
    Workspace(WorkspaceCmd),
    /// Cycles (sprints)
    #[command(subcommand)]
    Cycle(CycleCmd),
    /// Issue labels
    #[command(subcommand)]
    Label(LabelCmd),
    /// Workflow states (issue statuses)
    #[command(subcommand)]
    Status(StatusCmd),
    /// Documents
    #[command(subcommand)]
    Document(DocumentCmd),
    /// Initiatives
    #[command(subcommand)]
    Initiative(InitiativeCmd),
    /// Project milestones
    #[command(subcommand)]
    Milestone(MilestoneCmd),
    /// Project labels
    #[command(subcommand)]
    ProjectLabel(ProjectLabelCmd),
    /// Status updates on projects or initiatives
    #[command(subcommand)]
    StatusUpdate(StatusUpdateCmd),
    /// Issue attachments (links)
    #[command(subcommand)]
    Attachment(AttachmentCmd),
}

#[derive(clap::Args)]
pub struct Page {
    /// Max results to return
    #[arg(long, default_value_t = 50)]
    limit: i64,
    /// Cursor from a previous page's pageInfo.endCursor
    #[arg(long)]
    after: Option<String>,
}

/// Optional issue fields shared by create and update. IDs must be UUIDs
/// (look them up with `user list`, `status list`, etc.).
#[derive(clap::Args)]
pub struct IssueOpts {
    /// Markdown description
    #[arg(long)]
    description: Option<String>,
    /// Assignee user UUID
    #[arg(long)]
    assignee: Option<String>,
    /// Workflow state UUID
    #[arg(long)]
    state: Option<String>,
    /// Priority: 0 none, 1 urgent, 2 high, 3 normal, 4 low
    #[arg(long)]
    priority: Option<i64>,
    /// Parent issue UUID
    #[arg(long)]
    parent: Option<String>,
    /// Project UUID
    #[arg(long)]
    project: Option<String>,
    /// Label UUID, repeatable; on update this replaces all labels
    #[arg(long = "label")]
    labels: Option<Vec<String>>,
    /// Due date, YYYY-MM-DD
    #[arg(long)]
    due_date: Option<String>,
    /// Point estimate
    #[arg(long)]
    estimate: Option<i64>,
    /// Cycle UUID
    #[arg(long)]
    cycle: Option<String>,
}

#[derive(clap::Subcommand)]
pub enum IssueCmd {
    /// List issues, optionally filtered; values may be a UUID or a human name/key
    List {
        /// Team (UUID or key like ENG)
        #[arg(long)]
        team: Option<String>,
        /// Assignee (UUID, email, or exact display name)
        #[arg(long)]
        assignee: Option<String>,
        /// Workflow state (UUID or exact name like "In Progress")
        #[arg(long)]
        state: Option<String>,
        /// Project (UUID or exact name)
        #[arg(long)]
        project: Option<String>,
        /// Label (UUID or exact name)
        #[arg(long)]
        label: Option<String>,
        /// Full-text match on title/description
        #[arg(long)]
        query: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get one issue by UUID or identifier like ENG-123
    Get { id: String },
    /// Create an issue
    Create {
        /// Team UUID
        #[arg(long)]
        team: String,
        #[arg(long)]
        title: String,
        #[command(flatten)]
        opts: IssueOpts,
    },
    /// Update an issue; only the flags you pass are changed
    Update {
        /// Issue UUID or identifier like ENG-123
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[command(flatten)]
        opts: IssueOpts,
    },
}

#[derive(clap::Subcommand)]
pub enum CommentCmd {
    /// List comments on an issue
    List {
        /// Issue UUID or identifier like ENG-123
        #[arg(long)]
        issue: String,
        #[command(flatten)]
        page: Page,
    },
    /// Add a comment to an issue
    Create {
        /// Issue UUID or identifier like ENG-123
        #[arg(long)]
        issue: String,
        /// Markdown body
        #[arg(long)]
        body: String,
        /// Parent comment UUID, to reply in a thread
        #[arg(long)]
        parent: Option<String>,
    },
    /// Edit a comment
    Update {
        /// Comment UUID
        id: String,
        /// Markdown body
        #[arg(long)]
        body: String,
    },
    /// Delete a comment
    Delete { id: String },
}

#[derive(clap::Subcommand)]
pub enum ProjectCmd {
    /// List projects
    List {
        /// Team (UUID or key)
        #[arg(long)]
        team: Option<String>,
        /// Full-text match on name/content
        #[arg(long)]
        query: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get one project by UUID
    Get { id: String },
    /// Create a project
    Create {
        #[arg(long)]
        name: String,
        /// Team UUID, repeatable
        #[arg(long = "team", required = true)]
        teams: Vec<String>,
        #[arg(long)]
        description: Option<String>,
        /// Lead user UUID
        #[arg(long)]
        lead: Option<String>,
        /// Project status UUID
        #[arg(long)]
        status: Option<String>,
        /// YYYY-MM-DD
        #[arg(long)]
        start_date: Option<String>,
        /// YYYY-MM-DD
        #[arg(long)]
        target_date: Option<String>,
    },
    /// Update a project; only the flags you pass are changed
    Update {
        /// Project UUID
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Lead user UUID
        #[arg(long)]
        lead: Option<String>,
        /// Project status UUID
        #[arg(long)]
        status: Option<String>,
        /// YYYY-MM-DD
        #[arg(long)]
        start_date: Option<String>,
        /// YYYY-MM-DD
        #[arg(long)]
        target_date: Option<String>,
    },
}

#[derive(clap::Subcommand)]
pub enum TeamCmd {
    /// List teams
    List {
        #[command(flatten)]
        page: Page,
    },
    /// Get one team by UUID
    Get { id: String },
}

#[derive(clap::Subcommand)]
pub enum UserCmd {
    /// List users
    List {
        #[command(flatten)]
        page: Page,
    },
    /// Get one user by UUID
    Get { id: String },
}

#[derive(clap::Subcommand)]
pub enum WorkspaceCmd {
    /// Get the workspace and the authenticated user
    Get,
}

#[derive(clap::Subcommand)]
pub enum CycleCmd {
    /// List cycles of a team
    List {
        /// Team UUID
        #[arg(long)]
        team: String,
        #[command(flatten)]
        page: Page,
    },
}

#[derive(clap::Subcommand)]
pub enum LabelCmd {
    /// List issue labels
    List {
        /// Team (UUID or key)
        #[arg(long)]
        team: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Create an issue label
    Create {
        #[arg(long)]
        name: String,
        /// Team UUID; omit for a workspace label
        #[arg(long)]
        team: Option<String>,
        /// Hex color like #ff0000
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
}

#[derive(clap::Subcommand)]
pub enum StatusCmd {
    /// List workflow states
    List {
        /// Team (UUID or key)
        #[arg(long)]
        team: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get one workflow state by UUID
    Get { id: String },
}

#[derive(clap::Subcommand)]
pub enum DocumentCmd {
    /// List documents
    List {
        /// Project (UUID or exact name)
        #[arg(long)]
        project: Option<String>,
        /// Initiative (UUID or exact name)
        #[arg(long)]
        initiative: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get one document by UUID or slug, including its content
    Get { id: String },
    /// Create a document
    Create {
        #[arg(long)]
        title: String,
        /// Markdown content
        #[arg(long)]
        content: Option<String>,
        /// Project UUID
        #[arg(long)]
        project: Option<String>,
        /// Initiative UUID
        #[arg(long)]
        initiative: Option<String>,
    },
    /// Update a document; only the flags you pass are changed
    Update {
        /// Document UUID
        id: String,
        #[arg(long)]
        title: Option<String>,
        /// Markdown content, replaces the whole document
        #[arg(long)]
        content: Option<String>,
    },
}

#[derive(clap::Subcommand)]
pub enum InitiativeCmd {
    /// List initiatives
    List {
        #[command(flatten)]
        page: Page,
    },
    /// Get one initiative by UUID
    Get { id: String },
    /// Create an initiative
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
        /// YYYY-MM-DD
        #[arg(long)]
        target_date: Option<String>,
    },
    /// Update an initiative; only the flags you pass are changed
    Update {
        /// Initiative UUID
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// YYYY-MM-DD
        #[arg(long)]
        target_date: Option<String>,
    },
}

#[derive(clap::Subcommand)]
pub enum MilestoneCmd {
    /// List project milestones
    List {
        /// Project (UUID or exact name)
        #[arg(long)]
        project: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// Get one milestone by UUID
    Get { id: String },
    /// Create a milestone
    Create {
        /// Project UUID
        #[arg(long)]
        project: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
        /// YYYY-MM-DD
        #[arg(long)]
        target_date: Option<String>,
    },
    /// Update a milestone; only the flags you pass are changed
    Update {
        /// Milestone UUID
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// YYYY-MM-DD
        #[arg(long)]
        target_date: Option<String>,
    },
}

#[derive(clap::Subcommand)]
pub enum ProjectLabelCmd {
    /// List project labels
    List {
        #[command(flatten)]
        page: Page,
    },
}

#[derive(clap::Subcommand)]
pub enum StatusUpdateCmd {
    /// Get a status update by UUID
    Get {
        id: String,
        /// The id is an initiative status update, not a project one
        #[arg(long)]
        initiative: bool,
    },
    /// Post a status update on a project or an initiative
    Create {
        /// Project UUID
        #[arg(
            long,
            conflicts_with = "initiative",
            required_unless_present = "initiative"
        )]
        project: Option<String>,
        /// Initiative UUID
        #[arg(long)]
        initiative: Option<String>,
        /// Markdown body
        #[arg(long)]
        body: String,
        /// onTrack, atRisk, or offTrack
        #[arg(long)]
        health: Option<String>,
    },
    /// Edit a status update
    Update {
        /// Status update UUID
        id: String,
        /// The id is an initiative status update, not a project one
        #[arg(long)]
        initiative: bool,
        /// Markdown body
        #[arg(long)]
        body: Option<String>,
        /// onTrack, atRisk, or offTrack
        #[arg(long)]
        health: Option<String>,
    },
    /// Archive (delete) a status update
    Delete {
        /// Status update UUID
        id: String,
        /// The id is an initiative status update, not a project one
        #[arg(long)]
        initiative: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum AttachmentCmd {
    /// Get one attachment by UUID
    Get { id: String },
    /// Attach a URL to an issue
    Create {
        /// Issue UUID or identifier like ENG-123
        #[arg(long)]
        issue: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        subtitle: Option<String>,
    },
    /// Delete an attachment
    Delete { id: String },
}

fn issue_filter(
    team: Option<String>,
    assignee: Option<String>,
    state: Option<String>,
    project: Option<String>,
    label: Option<String>,
    query: Option<String>,
) -> issue_list::IssueFilter {
    use issue_list::*;
    IssueFilter {
        team: Box::new(team.map(|v| eq_filter!(issue_list, TeamFilter, IDComparator, key, v))),
        assignee: Box::new(assignee.map(|v| {
            if v.contains('@') {
                NullableUserFilter {
                    email: Some(StringComparator {
                        eq: Some(v),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            } else {
                eq_filter!(
                    issue_list,
                    NullableUserFilter,
                    IDComparator,
                    display_name,
                    v
                )
            }
        })),
        state: Box::new(
            state.map(|v| eq_filter!(issue_list, WorkflowStateFilter, IDComparator, name, v)),
        ),
        project: Box::new(project.map(|v| {
            eq_filter!(
                issue_list,
                NullableProjectFilter,
                EntityIdentifierIDComparator,
                name,
                v
            )
        })),
        labels: Box::new(label.map(|v| IssueLabelCollectionFilter {
            some: Box::new(Some(eq_filter!(
                issue_list,
                IssueLabelFilter,
                IDComparator,
                name,
                v
            ))),
            ..Default::default()
        })),
        searchable_content: query.map(|v| ContentComparator {
            contains: Some(v),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn run(cmd: Cmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Cmd::Issue(cmd) => match cmd {
            IssueCmd::List {
                team,
                assignee,
                state,
                project,
                label,
                query,
                page,
            } => exec::<IssueList>(issue_list::Variables {
                first: Some(page.limit),
                after: page.after,
                filter: Some(issue_filter(team, assignee, state, project, label, query)),
            }),
            IssueCmd::Get { id } => exec::<IssueGet>(issue_get::Variables { id }),
            IssueCmd::Create { team, title, opts } => {
                use issue_create::*;
                exec::<IssueCreate>(Variables {
                    input: IssueCreateInput {
                        team_id: team,
                        title: Some(title),
                        description: opts.description,
                        assignee_id: opts.assignee,
                        state_id: opts.state,
                        priority: opts.priority,
                        parent_id: opts.parent,
                        project_id: opts.project,
                        label_ids: opts.labels,
                        due_date: opts.due_date,
                        estimate: opts.estimate,
                        cycle_id: opts.cycle,
                        ..Default::default()
                    },
                })
            }
            IssueCmd::Update { id, title, opts } => {
                use issue_update::*;
                exec::<IssueUpdate>(Variables {
                    id,
                    input: IssueUpdateInput {
                        title,
                        description: opts.description,
                        assignee_id: opts.assignee,
                        state_id: opts.state,
                        priority: opts.priority,
                        parent_id: opts.parent,
                        project_id: opts.project,
                        label_ids: opts.labels,
                        due_date: opts.due_date,
                        estimate: opts.estimate,
                        cycle_id: opts.cycle,
                        ..Default::default()
                    },
                })
            }
        },
        Cmd::Comment(cmd) => match cmd {
            CommentCmd::List { issue, page } => exec::<CommentList>(comment_list::Variables {
                issue_id: issue,
                first: Some(page.limit),
                after: page.after,
            }),
            CommentCmd::Create {
                issue,
                body,
                parent,
            } => {
                use comment_create::*;
                exec::<CommentCreate>(Variables {
                    input: CommentCreateInput {
                        issue_id: Some(issue),
                        body: Some(body),
                        parent_id: parent,
                        ..Default::default()
                    },
                })
            }
            CommentCmd::Update { id, body } => {
                use comment_update::*;
                exec::<CommentUpdate>(Variables {
                    id,
                    input: CommentUpdateInput {
                        body: Some(body),
                        ..Default::default()
                    },
                })
            }
            CommentCmd::Delete { id } => exec::<CommentDelete>(comment_delete::Variables { id }),
        },
        Cmd::Project(cmd) => match cmd {
            ProjectCmd::List { team, query, page } => {
                use project_list::*;
                let filter = ProjectFilter {
                    accessible_teams: Box::new(team.map(|v| TeamCollectionFilter {
                        some: Box::new(Some(eq_filter!(
                            project_list,
                            TeamFilter,
                            IDComparator,
                            key,
                            v
                        ))),
                        ..Default::default()
                    })),
                    searchable_content: query.map(|v| ContentComparator {
                        contains: Some(v),
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                exec::<ProjectList>(Variables {
                    first: Some(page.limit),
                    after: page.after,
                    filter: Some(filter),
                })
            }
            ProjectCmd::Get { id } => exec::<ProjectGet>(project_get::Variables { id }),
            ProjectCmd::Create {
                name,
                teams,
                description,
                lead,
                status,
                start_date,
                target_date,
            } => {
                use project_create::*;
                exec::<ProjectCreate>(Variables {
                    input: ProjectCreateInput {
                        name,
                        team_ids: teams,
                        description,
                        lead_id: lead,
                        status_id: status,
                        start_date,
                        target_date,
                        ..Default::default()
                    },
                })
            }
            ProjectCmd::Update {
                id,
                name,
                description,
                lead,
                status,
                start_date,
                target_date,
            } => {
                use project_modify::*;
                exec::<ProjectModify>(Variables {
                    id,
                    input: ProjectUpdateInput {
                        name,
                        description,
                        lead_id: lead,
                        status_id: status,
                        start_date,
                        target_date,
                        ..Default::default()
                    },
                })
            }
        },
        Cmd::Team(cmd) => match cmd {
            TeamCmd::List { page } => exec::<TeamList>(team_list::Variables {
                first: Some(page.limit),
                after: page.after,
            }),
            TeamCmd::Get { id } => exec::<TeamGet>(team_get::Variables { id }),
        },
        Cmd::User(cmd) => match cmd {
            UserCmd::List { page } => exec::<UserList>(user_list::Variables {
                first: Some(page.limit),
                after: page.after,
            }),
            UserCmd::Get { id } => exec::<UserGet>(user_get::Variables { id }),
        },
        Cmd::Workspace(WorkspaceCmd::Get) => exec::<WorkspaceGet>(workspace_get::Variables {}),
        Cmd::Cycle(CycleCmd::List { team, page }) => exec::<CycleList>(cycle_list::Variables {
            team_id: team,
            first: Some(page.limit),
            after: page.after,
        }),
        Cmd::Label(cmd) => match cmd {
            LabelCmd::List { team, page } => {
                use label_list::*;
                let filter = team.map(|v| IssueLabelFilter {
                    team: Box::new(Some(eq_filter!(
                        label_list,
                        NullableTeamFilter,
                        IDComparator,
                        key,
                        v
                    ))),
                    ..Default::default()
                });
                exec::<LabelList>(Variables {
                    first: Some(page.limit),
                    after: page.after,
                    filter,
                })
            }
            LabelCmd::Create {
                name,
                team,
                color,
                description,
            } => {
                use label_create::*;
                exec::<LabelCreate>(Variables {
                    input: IssueLabelCreateInput {
                        name,
                        team_id: team,
                        color,
                        description,
                        ..Default::default()
                    },
                })
            }
        },
        Cmd::Status(cmd) => match cmd {
            StatusCmd::List { team, page } => {
                use status_list::*;
                let filter = team.map(|v| WorkflowStateFilter {
                    team: Box::new(Some(eq_filter!(
                        status_list,
                        TeamFilter,
                        IDComparator,
                        key,
                        v
                    ))),
                    ..Default::default()
                });
                exec::<StatusList>(Variables {
                    first: Some(page.limit),
                    after: page.after,
                    filter,
                })
            }
            StatusCmd::Get { id } => exec::<StatusGet>(status_get::Variables { id }),
        },
        Cmd::Document(cmd) => match cmd {
            DocumentCmd::List {
                project,
                initiative,
                page,
            } => {
                use document_list::*;
                let filter = DocumentFilter {
                    project: Box::new(project.map(|v| {
                        eq_filter!(
                            document_list,
                            ProjectFilter,
                            EntityIdentifierIDComparator,
                            name,
                            v
                        )
                    })),
                    initiative: Box::new(initiative.map(|v| {
                        eq_filter!(
                            document_list,
                            InitiativeFilter,
                            EntityIdentifierIDComparator,
                            name,
                            v
                        )
                    })),
                    ..Default::default()
                };
                exec::<DocumentList>(Variables {
                    first: Some(page.limit),
                    after: page.after,
                    filter: Some(filter),
                })
            }
            DocumentCmd::Get { id } => exec::<DocumentGet>(document_get::Variables { id }),
            DocumentCmd::Create {
                title,
                content,
                project,
                initiative,
            } => {
                use document_create::*;
                exec::<DocumentCreate>(Variables {
                    input: DocumentCreateInput {
                        title,
                        content,
                        project_id: project,
                        initiative_id: initiative,
                        ..Default::default()
                    },
                })
            }
            DocumentCmd::Update { id, title, content } => {
                use document_update::*;
                exec::<DocumentUpdate>(Variables {
                    id,
                    input: DocumentUpdateInput {
                        title,
                        content,
                        ..Default::default()
                    },
                })
            }
        },
        Cmd::Initiative(cmd) => match cmd {
            InitiativeCmd::List { page } => exec::<InitiativeList>(initiative_list::Variables {
                first: Some(page.limit),
                after: page.after,
            }),
            InitiativeCmd::Get { id } => exec::<InitiativeGet>(initiative_get::Variables { id }),
            InitiativeCmd::Create {
                name,
                description,
                target_date,
            } => {
                use initiative_create::*;
                exec::<InitiativeCreate>(Variables {
                    input: InitiativeCreateInput {
                        name,
                        description,
                        target_date,
                        ..Default::default()
                    },
                })
            }
            InitiativeCmd::Update {
                id,
                name,
                description,
                target_date,
            } => {
                use initiative_modify::*;
                exec::<InitiativeModify>(Variables {
                    id,
                    input: InitiativeUpdateInput {
                        name,
                        description,
                        target_date,
                        ..Default::default()
                    },
                })
            }
        },
        Cmd::Milestone(cmd) => match cmd {
            MilestoneCmd::List { project, page } => {
                use milestone_list::*;
                let filter = project.map(|v| ProjectMilestoneFilter {
                    project: Box::new(Some(eq_filter!(
                        milestone_list,
                        NullableProjectFilter,
                        EntityIdentifierIDComparator,
                        name,
                        v
                    ))),
                    ..Default::default()
                });
                exec::<MilestoneList>(Variables {
                    first: Some(page.limit),
                    after: page.after,
                    filter,
                })
            }
            MilestoneCmd::Get { id } => exec::<MilestoneGet>(milestone_get::Variables { id }),
            MilestoneCmd::Create {
                project,
                name,
                description,
                target_date,
            } => {
                use milestone_create::*;
                exec::<MilestoneCreate>(Variables {
                    input: ProjectMilestoneCreateInput {
                        project_id: project,
                        name,
                        description,
                        target_date,
                        ..Default::default()
                    },
                })
            }
            MilestoneCmd::Update {
                id,
                name,
                description,
                target_date,
            } => {
                use milestone_update::*;
                exec::<MilestoneUpdate>(Variables {
                    id,
                    input: ProjectMilestoneUpdateInput {
                        name,
                        description,
                        target_date,
                        ..Default::default()
                    },
                })
            }
        },
        Cmd::ProjectLabel(ProjectLabelCmd::List { page }) => {
            exec::<ProjectLabelList>(project_label_list::Variables {
                first: Some(page.limit),
                after: page.after,
            })
        }
        Cmd::StatusUpdate(cmd) => match cmd {
            StatusUpdateCmd::Get {
                id,
                initiative: false,
            } => exec::<StatusUpdateGet>(status_update_get::Variables { id }),
            StatusUpdateCmd::Get {
                id,
                initiative: true,
            } => exec::<InitiativeStatusUpdateGet>(initiative_status_update_get::Variables { id }),
            StatusUpdateCmd::Create {
                project: Some(project),
                initiative: None,
                body,
                health,
            } => {
                use status_update_create::*;
                exec::<StatusUpdateCreate>(Variables {
                    input: ProjectUpdateCreateInput {
                        project_id: project,
                        body: Some(body),
                        health: health.map(parse_enum).transpose()?,
                        ..Default::default()
                    },
                })
            }
            StatusUpdateCmd::Create {
                initiative: Some(initiative),
                body,
                health,
                ..
            } => {
                use initiative_status_update_create::*;
                exec::<InitiativeStatusUpdateCreate>(Variables {
                    input: InitiativeUpdateCreateInput {
                        initiative_id: initiative,
                        body: Some(body),
                        health: health.map(parse_enum).transpose()?,
                        ..Default::default()
                    },
                })
            }
            StatusUpdateCmd::Create { .. } => {
                unreachable!("clap enforces --project xor --initiative")
            }
            StatusUpdateCmd::Update {
                id,
                initiative: false,
                body,
                health,
            } => {
                use status_update_modify::*;
                exec::<StatusUpdateModify>(Variables {
                    id,
                    input: ProjectUpdateUpdateInput {
                        body,
                        health: health.map(parse_enum).transpose()?,
                        ..Default::default()
                    },
                })
            }
            StatusUpdateCmd::Update {
                id,
                initiative: true,
                body,
                health,
            } => {
                use initiative_status_update_modify::*;
                exec::<InitiativeStatusUpdateModify>(Variables {
                    id,
                    input: InitiativeUpdateUpdateInput {
                        body,
                        health: health.map(parse_enum).transpose()?,
                        ..Default::default()
                    },
                })
            }
            StatusUpdateCmd::Delete {
                id,
                initiative: false,
            } => exec::<StatusUpdateArchive>(status_update_archive::Variables { id }),
            StatusUpdateCmd::Delete {
                id,
                initiative: true,
            } => {
                exec::<InitiativeStatusUpdateArchive>(initiative_status_update_archive::Variables {
                    id,
                })
            }
        },
        Cmd::Attachment(cmd) => match cmd {
            AttachmentCmd::Get { id } => exec::<AttachmentGet>(attachment_get::Variables { id }),
            AttachmentCmd::Create {
                issue,
                url,
                title,
                subtitle,
            } => {
                use attachment_create::*;
                exec::<AttachmentCreate>(Variables {
                    input: AttachmentCreateInput {
                        issue_id: issue,
                        url,
                        title,
                        subtitle,
                        ..Default::default()
                    },
                })
            }
            AttachmentCmd::Delete { id } => {
                exec::<AttachmentDelete>(attachment_delete::Variables { id })
            }
        },
    }
}

pub(crate) fn auth_identity(
    token: &str,
) -> Result<serde_json::Value, crate::auth::ValidationError> {
    auth_identity_at(token, API_URL)
}

fn auth_identity_at(
    token: &str,
    api_url: &str,
) -> Result<serde_json::Value, crate::auth::ValidationError> {
    let response = reqwest::blocking::Client::new()
        .post(api_url)
        .header("Authorization", token)
        .json(&WorkspaceGet::build_query(workspace_get::Variables {}))
        .send()
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .map_err(|error| crate::auth::ValidationError::Failed(error.to_string()))?;
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(crate::auth::ValidationError::Rejected(body.to_string()));
    }
    if !status.is_success() {
        return Err(crate::auth::ValidationError::Failed(body.to_string()));
    }
    if body.get("errors").is_some_and(|errors| !errors.is_null()) {
        return Err(crate::auth::ValidationError::Failed(body.to_string()));
    }
    Ok(body["data"].clone())
}

/// Parse a CLI string into a generated GraphQL enum (e.g. "onTrack") via serde.
fn parse_enum<T: serde::de::DeserializeOwned>(s: String) -> Result<T, Box<dyn std::error::Error>> {
    Ok(serde_json::from_value(serde_json::Value::String(s))?)
}

/// POST a compile-time-checked query to Linear, print the `data` JSON on stdout.
fn exec<Q: GraphQLQuery>(variables: Q::Variables) -> Result<(), Box<dyn std::error::Error>> {
    let key = crate::auth::linear_token()?;
    let response = reqwest::blocking::Client::new()
        .post(API_URL)
        // Personal API keys are sent bare, no "Bearer" prefix.
        .header("Authorization", key)
        .json(&Q::build_query(variables))
        .send()?;
    let status = response.status();
    let body: serde_json::Value = response.json()?;
    if !status.is_success() || body.get("errors").is_some_and(|e| !e.is_null()) {
        return Err(body.to_string().into());
    }
    println!("{}", body["data"]);
    Ok(())
}

pub(crate) fn authenticated() -> bool {
    crate::auth::linear_token().is_ok()
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn issue_filter_maps_flags_to_variables() {
        let q = IssueList::build_query(issue_list::Variables {
            first: Some(5),
            after: None,
            filter: Some(issue_filter(
                Some("ENG".into()),
                Some("someone@example.com".into()),
                None,
                None,
                Some("Bug".into()),
                None,
            )),
        });
        let v = serde_json::to_value(&q).unwrap();
        let filter = &v["variables"]["filter"];
        assert_eq!(filter["team"]["key"]["eq"], "ENG");
        assert_eq!(filter["assignee"]["email"]["eq"], "someone@example.com");
        assert_eq!(filter["labels"]["some"]["name"]["eq"], "Bug");
        // Unset optional fields must be omitted entirely, not sent as null:
        // null values in update inputs would wipe data server-side.
        assert_eq!(filter.as_object().unwrap().len(), 3);
        assert!(v["variables"].get("after").is_none());
    }

    #[test]
    fn validates_linear_identity_and_rejects_bad_credentials() {
        let body = r#"{"data":{"organization":{"id":"workspace-id","name":"Workspace","urlKey":"workspace","createdAt":"2026-01-01T00:00:00Z"},"viewer":{"id":"user-id","name":"User","displayName":"Display","email":"user@example.com"}}}"#;
        let (url, request_rx, server) = test_api("200 OK", body);
        let identity = auth_identity_at("linear-token", &url).unwrap();
        server.join().unwrap();
        let request = request_rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.contains("authorization: linear-token"));
        assert_eq!(identity["viewer"]["id"], "user-id");

        let (url, _, server) = test_api("401 Unauthorized", r#"{"error":"invalid"}"#);
        let error = auth_identity_at("bad-token", &url).unwrap_err();
        server.join().unwrap();
        assert!(matches!(error, crate::auth::ValidationError::Rejected(_)));

        let (url, _, server) = test_api(
            "200 OK",
            r#"{"errors":[{"message":"provider failure"}],"data":null}"#,
        );
        let error = auth_identity_at("token", &url).unwrap_err();
        server.join().unwrap();
        assert!(matches!(error, crate::auth::ValidationError::Failed(_)));
    }

    fn test_api(
        status: &str,
        body: &str,
    ) -> (String, mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (request_tx, request_rx) = mpsc::channel();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..read]);
                let complete = request
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .is_some_and(|header_end| {
                        let headers = String::from_utf8_lossy(&request[..header_end]);
                        let content_length = headers.lines().find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        });
                        request.len() >= header_end + 4 + content_length.unwrap_or(0)
                    });
                if read == 0 || complete {
                    break;
                }
            }
            let _ = request_tx.send(String::from_utf8(request).unwrap());
            stream.write_all(response.as_bytes()).unwrap();
        });
        (format!("http://{address}/"), request_rx, server)
    }
}
