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
    /// Manage the agent skill (SKILL.md) describing how to use this CLI
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
            let providers = providers();
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
            provider::ensure_enabled(&provider::load(), "github")?;
            github::run(cmd, format)
        }
        Command::Linear(cmd) => {
            provider::ensure_enabled(&provider::load(), "linear")?;
            linear::run(cmd, format)
        }
        Command::Sentry(cmd) => {
            provider::ensure_enabled(&provider::load(), "sentry")?;
            sentry::run(cmd, format)
        }
        Command::Slack(cmd) => {
            provider::ensure_enabled(&provider::load(), "slack")?;
            slack::run(cmd, format)
        }
        Command::Provider(cmd) => provider::run(cmd, format),
        Command::Skill(cmd) => {
            let skill = render_skill(&providers());
            match cmd {
                SkillCmd::Print => {
                    print!("{skill}");
                    Ok(())
                }
                SkillCmd::Install => skill_install_cmd(&skill),
            }
        }
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
    /// Print the skill to stdout
    Print,
    /// Install the skill for every supported agent found on this machine
    Install,
}

fn providers() -> [Provider; 4] {
    let config = provider::load();
    [
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
    ]
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

fn skill_install_cmd(skill: &str) -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::home_dir().ok_or("could not determine the home directory")?;
    let installed = skill_install(&home, skill)?;
    if installed.is_empty() {
        return Err("no supported agent found; install manually with: foac skill print > <agent skills dir>/foac/SKILL.md".into());
    }
    for path in installed {
        println!("Installed {}", path.display());
    }
    Ok(())
}

/// Write the skill into the skill folders of the agents present under `home`.
/// Claude Code only reads its own folder; every other major agent (Cursor,
/// Codex, Gemini CLI, Copilot, OpenCode, Amp, Cline, ...) reads the
/// cross-agent standard ~/.agents/skills, so two writes cover them all.
fn skill_install(
    home: &std::path::Path,
    skill: &str,
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
        let dir = skills_dir.join("foac");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("SKILL.md");
        std::fs::write(&path, skill)?;
        installed.push(path);
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
    fn skill_only_documents_authenticated_providers() {
        for (linear, github, sentry, slack) in [
            (false, false, false, false),
            (true, false, false, false),
            (false, true, false, false),
            (false, false, true, false),
            (false, false, false, true),
            (true, true, true, true),
        ] {
            let skill = render_skill(&test_providers(linear, github, sentry, slack));
            assert_eq!(skill.contains("foac linear issue list"), linear);
            assert_eq!(skill.contains("foac github issue list"), github);
            assert_eq!(skill.contains("foac sentry issue list"), sentry);
            assert_eq!(skill.contains("foac slack search"), slack);
            assert!(!skill.contains("<!-- foac-provider:"));
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
        assert!(matches!(
            Cli::try_parse_from(["foac", "skill", "print"])
                .unwrap()
                .command,
            Command::Skill(SkillCmd::Print)
        ));
    }

    #[test]
    fn skill_install_targets_detected_agents() {
        let home = std::env::temp_dir().join(format!("foac-test-{}", std::process::id()));
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        std::fs::create_dir_all(home.join(".cursor")).unwrap();
        let skill = render_skill(&test_providers(true, false, false, false));
        let installed = skill_install(&home, &skill).unwrap();
        assert_eq!(
            installed,
            vec![
                home.join(".claude/skills/foac/SKILL.md"),
                home.join(".agents/skills/foac/SKILL.md"),
            ]
        );
        assert_eq!(std::fs::read_to_string(&installed[0]).unwrap(), skill);
        assert_eq!(std::fs::read_to_string(&installed[1]).unwrap(), skill);
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
