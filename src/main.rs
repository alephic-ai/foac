use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

use foac::{auth, github, linear, output, provider, sentry, slack, update};

#[derive(Parser)]
#[command(about, arg_required_else_help = true)]
struct Cli {
    /// Output format for JSON command output (version, update, and skill ignore it)
    #[arg(long, global = true, value_enum, default_value = "auto")]
    format: output::FormatArg,
    #[command(subcommand)]
    command: Command,
}

const SKILL_MD: &str = include_str!("../doc/SKILL.md");

#[derive(Clone, Copy)]
struct Provider {
    name: &'static str,
    /// Authenticated and enabled; inactive providers are hidden from discovery.
    active: bool,
}

impl Provider {
    const fn new(name: &'static str, active: bool) -> Self {
        Self { name, active }
    }
}

#[derive(Subcommand)]
// One short-lived value on the stack; boxing the variant isn't worth it.
#[allow(clippy::large_enum_variant)]
enum Command {
    /// Check and configure provider authentication
    Auth(auth::Cmd),
    /// Interact with GitHub
    Github(github::Cmd),
    /// Interact with Linear (linear.app)
    #[command(subcommand)]
    Linear(linear::Cmd),
    /// Interact with Sentry
    Sentry(sentry::Cmd),
    /// Interact with Slack
    Slack(slack::Cmd),
    /// Enable or disable providers
    #[command(subcommand, arg_required_else_help = true)]
    Provider(provider::Cmd),
    /// Manage the per-provider agent skills describing how to use this CLI
    #[command(subcommand, arg_required_else_help = true)]
    Skill(SkillCmd),
    /// Download and replace this binary with the latest GitHub release
    Update,
    /// Print the version
    Version,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Probing auth (keychain reads, possible `gh` subprocess) is only needed to
    // hide providers in help/error output and to render the skill, so parse
    // with the plain command first and probe only on those cold paths.
    let cli = match Cli::try_parse_from(std::env::args_os()) {
        Ok(cli) => cli,
        Err(_) => {
            let providers = providers()?;
            match try_parse_from(&providers, std::env::args_os()) {
                Ok(cli) => cli,
                Err(error) => error.exit(),
            }
        }
    };
    let format = output::resolve(
        cli.format,
        std::env::var("FOAC_FORMAT").ok().as_deref(),
        std::env::var_os("CI").is_some(),
        std::io::IsTerminal::is_terminal(&std::io::stdout()),
    );
    let command = cli.command;
    let skip_check = matches!(command, Command::Update);
    let result = match command {
        Command::Auth(cmd) => auth::run(cmd, format),
        Command::Github(cmd) => {
            provider::ensure_enabled(&provider::load()?, "github")?;
            github::run(cmd, format)
        }
        Command::Linear(cmd) => {
            provider::ensure_enabled(&provider::load()?, "linear")?;
            linear::run(cmd, format)
        }
        Command::Sentry(cmd) => {
            provider::ensure_enabled(&provider::load()?, "sentry")?;
            sentry::run(cmd, format)
        }
        Command::Slack(cmd) => {
            provider::ensure_enabled(&provider::load()?, "slack")?;
            slack::run(cmd, format)
        }
        Command::Provider(cmd) => provider::run(cmd, format),
        Command::Skill(cmd) => match cmd {
            SkillCmd::Print { provider } => {
                print!("{}", render_provider_skill(&provider));
                Ok(())
            }
            SkillCmd::Install => skill_install_cmd(),
        },
        Command::Update => update::run(),
        Command::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    };
    if !skip_check {
        update::notify_if_outdated();
    }
    result
}

#[derive(Subcommand)]
enum SkillCmd {
    /// Print one provider's skill to stdout
    #[command(arg_required_else_help = true)]
    Print {
        #[arg(value_parser = clap::builder::PossibleValuesParser::new(provider::PROVIDERS))]
        provider: String,
    },
    /// Install one skill per active provider for every supported agent found
    /// on this machine, removing the skills of inactive providers
    Install,
}

fn providers() -> Result<[Provider; 4], Box<dyn std::error::Error>> {
    let config = provider::load()?;
    Ok([
        // Config first: short-circuit skips the keychain/`gh` probe when disabled.
        Provider::new(
            "github",
            config.enabled("github") && github::authenticated(),
        ),
        Provider::new(
            "linear",
            config.enabled("linear") && linear::authenticated(),
        ),
        Provider::new(
            "sentry",
            config.enabled("sentry") && sentry::authenticated(),
        ),
        Provider::new("slack", config.enabled("slack") && slack::authenticated()),
    ])
}

fn cli_command(providers: &[Provider]) -> clap::Command {
    providers.iter().fold(Cli::command(), |command, provider| {
        command.mut_subcommand(provider.name, |subcommand| {
            subcommand.hide(!provider.active)
        })
    })
}

fn try_parse_from<I, T>(providers: &[Provider], args: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let matches = cli_command(providers)
        .try_get_matches_from(args)
        .map_err(|mut error| {
            remove_hidden_provider_suggestions(&mut error, providers);
            error
        })?;
    Cli::from_arg_matches(&matches)
}

fn remove_hidden_provider_suggestions(error: &mut clap::Error, providers: &[Provider]) {
    use clap::error::{ContextKind, ContextValue};

    let Some(ContextValue::Strings(mut suggestions)) =
        error.remove(ContextKind::SuggestedSubcommand)
    else {
        return;
    };
    suggestions.retain(|suggestion| {
        providers
            .iter()
            .find(|provider| provider.name == suggestion)
            .is_none_or(|provider| provider.active)
    });
    if !suggestions.is_empty() {
        error.insert(
            ContextKind::SuggestedSubcommand,
            ContextValue::Strings(suggestions),
        );
    }
}

fn render_skill(providers: &[Provider]) -> String {
    let mut rendered = String::with_capacity(SKILL_MD.len());
    let mut provider_block = None;

    for line in SKILL_MD.split_inclusive('\n') {
        let marker = line.trim();
        // Lint pragmas are for the source file, not the rendered skill.
        if marker.starts_with("<!-- rumdl-") {
            continue;
        }
        if let Some(name) = marker
            .strip_prefix("<!-- foac-provider:")
            .and_then(|marker| marker.strip_suffix(" -->"))
        {
            assert!(
                provider_block.is_none(),
                "provider skill blocks cannot nest"
            );
            let active = providers
                .iter()
                .find(|provider| provider.name == name)
                .unwrap_or_else(|| panic!("unknown provider skill block: {name}"))
                .active;
            provider_block = Some((name, active));
            continue;
        }
        if let Some(name) = marker
            .strip_prefix("<!-- /foac-provider:")
            .and_then(|marker| marker.strip_suffix(" -->"))
        {
            assert_eq!(provider_block.map(|(name, _)| name), Some(name));
            provider_block = None;
            continue;
        }
        if provider_block.is_none_or(|(_, active)| active) {
            rendered.push_str(line);
        }
    }

    assert!(
        provider_block.is_none(),
        "provider skill block is not closed"
    );
    rendered
}

fn render_provider_skill(name: &str) -> String {
    render_skill(&provider::PROVIDERS.map(|p| Provider::new(p, p == name)))
}

fn skill_install_cmd() -> Result<(), Box<dyn std::error::Error>> {
    let active: Vec<&str> = providers()?
        .iter()
        .filter(|provider| provider.active)
        .map(|provider| provider.name)
        .collect();
    if active.is_empty() {
        return Err(
            "no authenticated, enabled providers; authenticate with `foac auth <provider> login` and retry".into(),
        );
    }
    let home = std::env::home_dir().ok_or("could not determine the home directory")?;
    let installed = skill_install(&home, &active)?;
    if installed.is_empty() {
        return Err("no supported agent found; install manually with: foac skill print <provider> > <agent skills dir>/foac-<provider>/SKILL.md".into());
    }
    for path in installed {
        println!("Installed {}", path.display());
    }
    Ok(())
}

/// Write one skill per active provider into the skill folders of the agents
/// present under `home`, and remove the skills of inactive providers.
/// Claude Code only reads its own folder; every other major agent (Cursor,
/// Codex, Gemini CLI, Copilot, OpenCode, Amp, Cline, ...) reads the
/// cross-agent standard ~/.agents/skills, so two targets cover them all.
fn skill_install(
    home: &std::path::Path,
    active: &[&str],
) -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error>> {
    let shared_agent_roots = [
        ".agents",
        ".cursor",
        ".codex",
        ".gemini",
        ".copilot",
        ".config/opencode",
        ".config/amp",
    ];
    let mut targets = Vec::new();
    if home.join(".claude").is_dir() {
        targets.push(home.join(".claude/skills"));
    }
    if shared_agent_roots.iter().any(|r| home.join(r).is_dir()) {
        targets.push(home.join(".agents/skills"));
    }
    let mut installed = Vec::new();
    for skills_dir in targets {
        for name in provider::PROVIDERS {
            let dir = skills_dir.join(format!("foac-{name}"));
            if active.contains(&name) {
                std::fs::create_dir_all(&dir)?;
                let path = dir.join("SKILL.md");
                std::fs::write(&path, render_provider_skill(name))?;
                installed.push(path);
            } else if let Err(err) = std::fs::remove_dir_all(&dir)
                && err.kind() != std::io::ErrorKind::NotFound
            {
                return Err(err.into());
            }
        }
    }
    Ok(installed)
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, error::ErrorKind};

    use super::*;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn help_only_lists_authenticated_providers() {
        for (linear, github, sentry, slack, expected) in [
            (false, false, false, false, vec![]),
            (true, false, false, false, vec!["linear"]),
            (false, true, false, false, vec!["github"]),
            (false, false, true, false, vec!["sentry"]),
            (false, false, false, true, vec!["slack"]),
            (
                true,
                true,
                true,
                true,
                vec!["github", "linear", "sentry", "slack"],
            ),
        ] {
            let providers = test_providers(linear, github, sentry, slack);
            for args in [vec!["foac"], vec!["foac", "--help"]] {
                let help = parse_error(&providers, args).to_string();
                for name in ["github", "linear", "sentry", "slack"] {
                    assert_eq!(help_lists(&help, name), expected.contains(&name));
                }
                for name in ["auth", "provider", "skill", "update", "version", "help"] {
                    assert!(help_lists(&help, name));
                }
            }
        }
    }

    #[test]
    fn hidden_providers_still_parse() {
        let providers = test_providers(false, false, false, false);
        for args in [
            vec!["foac", "github", "issue", "list", "--repo", "owner/repo"],
            vec!["foac", "linear", "team", "list"],
            vec!["foac", "sentry", "issue", "list", "--org", "acme"],
            vec!["foac", "slack", "conversation", "list"],
        ] {
            try_parse_from(&providers, args).unwrap();
        }
    }

    #[test]
    fn hidden_provider_help_remains_available() {
        let providers = test_providers(false, false, false, false);
        for (args, usage) in [
            (vec!["foac", "github", "--help"], "Usage: foac github"),
            (vec!["foac", "linear", "--help"], "Usage: foac linear"),
            (vec!["foac", "sentry", "--help"], "Usage: foac sentry"),
            (vec!["foac", "slack", "--help"], "Usage: foac slack"),
        ] {
            let error = parse_error(&providers, args);
            assert_eq!(error.kind(), ErrorKind::DisplayHelp);
            assert!(error.to_string().contains(usage));
        }
    }

    #[test]
    fn hidden_providers_are_not_suggested() {
        let error = parse_error(
            &test_providers(false, false, false, false),
            ["foac", "githu"],
        )
        .to_string();
        assert!(!error.contains("github"));

        let error = parse_error(
            &test_providers(false, true, false, false),
            ["foac", "githu"],
        )
        .to_string();
        assert!(error.contains("github"));
    }

    #[test]
    fn skill_documents_one_provider() {
        let examples = [
            ("github", "foac github issue list"),
            ("linear", "foac linear issue list"),
            ("sentry", "foac sentry issue list"),
            ("slack", "foac slack search"),
        ];
        for (name, _) in examples {
            let skill = render_provider_skill(name);
            assert!(skill.starts_with(&format!("---\nname: foac-{name}\ndescription:")));
            assert_eq!(skill.matches("name: foac-").count(), 1);
            assert!(skill.contains(&format!("# foac-{name}")));
            for (other, other_example) in examples {
                assert_eq!(skill.contains(other_example), other == name);
            }
            assert!(!skill.contains("<!-- foac-provider:"));
            assert!(!skill.contains("rumdl"));
            assert!(
                skill.contains("top-level `--help` lists only authenticated, enabled providers")
            );
        }
    }

    #[test]
    fn bare_auth_commands_display_help() {
        for args in [
            vec!["foac", "auth"],
            vec!["foac", "auth", "linear"],
            vec!["foac", "auth", "github"],
            vec!["foac", "auth", "sentry"],
            vec!["foac", "auth", "slack"],
        ] {
            let error = match Cli::try_parse_from(args) {
                Ok(_) => panic!("bare auth command should display help"),
                Err(error) => error,
            };
            assert_eq!(
                error.kind(),
                ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            );
        }
    }

    #[test]
    fn parses_auth_commands() {
        for args in [
            vec!["foac", "auth", "status"],
            vec!["foac", "auth", "linear", "status"],
            vec!["foac", "auth", "linear", "login"],
            vec!["foac", "auth", "linear", "logout"],
            vec!["foac", "auth", "github", "status"],
            vec!["foac", "auth", "github", "login"],
            vec!["foac", "auth", "github", "logout"],
            vec!["foac", "auth", "sentry", "status"],
            vec!["foac", "auth", "sentry", "login"],
            vec![
                "foac",
                "auth",
                "sentry",
                "login",
                "--host",
                "sentry.example.com",
            ],
            vec!["foac", "auth", "sentry", "logout"],
            vec!["foac", "auth", "slack", "status"],
            vec!["foac", "auth", "slack", "login"],
            vec!["foac", "auth", "slack", "logout"],
        ] {
            Cli::try_parse_from(args).unwrap();
        }
        // --host is a Sentry-only login flag; clap rejects it elsewhere.
        for provider in ["linear", "github", "slack"] {
            let parsed =
                Cli::try_parse_from(["foac", "auth", provider, "login", "--host", "example.com"]);
            assert!(parsed.is_err());
        }
    }

    #[test]
    fn slack_login_help_describes_both_token_inputs() {
        let error = match Cli::try_parse_from(["foac", "auth", "slack", "login", "--help"]) {
            Ok(_) => panic!("--help should stop parsing"),
            Err(error) => error,
        };
        let help = error.to_string();
        assert!(help.contains("bot token, then the user token"));
        assert!(help.contains("two lines in the same order"));
        assert!(help.contains("either token may be blank"));
    }

    #[test]
    fn parses_provider_commands() {
        for args in [
            vec!["foac", "provider", "list"],
            vec!["foac", "provider", "enable", "github"],
            vec!["foac", "provider", "enable", "linear"],
            vec!["foac", "provider", "disable", "github"],
            vec!["foac", "provider", "disable", "linear"],
            vec!["foac", "provider", "enable", "sentry"],
            vec!["foac", "provider", "disable", "sentry"],
            vec!["foac", "provider", "enable", "slack"],
            vec!["foac", "provider", "disable", "slack"],
        ] {
            Cli::try_parse_from(args).unwrap();
        }
        assert!(Cli::try_parse_from(["foac", "provider", "enable", "nope"]).is_err());
        let error = match Cli::try_parse_from(["foac", "provider"]) {
            Ok(_) => panic!("bare provider command should display help"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn parses_slack_commands() {
        for args in [
            vec!["foac", "slack", "conversation", "list"],
            vec!["foac", "slack", "conversation", "get", "#eng"],
            vec!["foac", "slack", "message", "list", "#eng"],
            vec![
                "foac",
                "slack",
                "message",
                "get",
                "#eng",
                "1724432400.123456",
                "--thread-ts",
                "1724432300.123456",
            ],
            vec![
                "foac", "slack", "message", "create", "#eng", "--body", "hello",
            ],
            vec![
                "foac",
                "slack",
                "message",
                "update",
                "C123",
                "1724432400.123456",
                "--body-file",
                "/tmp/message.md",
            ],
            vec![
                "foac",
                "slack",
                "message",
                "delete",
                "C123",
                "1724432400.123456",
            ],
            vec!["foac", "slack", "user", "list"],
            vec!["foac", "slack", "user", "get", "person@example.com"],
            vec![
                "foac",
                "slack",
                "search",
                "deployment in:eng",
                "--after",
                "next-cursor",
            ],
            vec![
                "foac",
                "slack",
                "reaction",
                "add",
                "#eng",
                "1724432400.123456",
                "eyes",
            ],
            vec![
                "foac",
                "slack",
                "reaction",
                "remove",
                "#eng",
                "1724432400.123456",
                "eyes",
            ],
        ] {
            Cli::try_parse_from(args).unwrap();
        }
    }

    #[test]
    fn skill_requires_subcommand() {
        assert!(Cli::try_parse_from(["foac", "skill"]).is_err());
        let missing_provider = Cli::try_parse_from(["foac", "skill", "print"])
            .err()
            .unwrap()
            .to_string();
        assert!(missing_provider.contains("<PROVIDER>"));
        assert!(missing_provider.contains("possible values: github, linear, sentry, slack"));
        assert!(Cli::try_parse_from(["foac", "skill", "print", "nope"]).is_err());
        assert!(matches!(
            Cli::try_parse_from(["foac", "skill", "print", "linear"])
                .unwrap()
                .command,
            Command::Skill(SkillCmd::Print { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["foac", "skill", "install"])
                .unwrap()
                .command,
            Command::Skill(SkillCmd::Install)
        ));
    }

    #[test]
    fn skill_install_targets_detected_agents() {
        let home = std::env::temp_dir().join(format!("foac-test-{}", std::process::id()));
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        std::fs::create_dir_all(home.join(".cursor")).unwrap();
        let stale = home.join(".agents/skills/foac-sentry");
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("SKILL.md"), "stale").unwrap();
        let installed = skill_install(&home, &["github", "linear"]).unwrap();
        assert_eq!(
            installed,
            vec![
                home.join(".claude/skills/foac-github/SKILL.md"),
                home.join(".claude/skills/foac-linear/SKILL.md"),
                home.join(".agents/skills/foac-github/SKILL.md"),
                home.join(".agents/skills/foac-linear/SKILL.md"),
            ]
        );
        for path in &installed {
            let name = path
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            let provider = name.strip_prefix("foac-").unwrap();
            assert_eq!(
                std::fs::read_to_string(path).unwrap(),
                render_provider_skill(provider)
            );
        }
        assert!(!stale.exists());
        std::fs::remove_dir_all(&home).unwrap();
    }

    fn test_providers(linear: bool, github: bool, sentry: bool, slack: bool) -> [Provider; 4] {
        [
            Provider::new("github", github),
            Provider::new("linear", linear),
            Provider::new("sentry", sentry),
            Provider::new("slack", slack),
        ]
    }

    fn help_lists(help: &str, command: &str) -> bool {
        help.lines()
            .map(str::trim_start)
            .any(|line| line == command || line.starts_with(&format!("{command} ")))
    }

    fn parse_error<I, T>(providers: &[Provider], args: I) -> clap::Error
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        match try_parse_from(providers, args) {
            Ok(_) => panic!("arguments should produce a clap error"),
            Err(error) => error,
        }
    }
}
