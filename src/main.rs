use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

use foac::{
    auth, confluence, github, jira, linear, neon, output, provider, sentry, slack, update, vercel,
};

#[derive(Parser)]
#[command(about, arg_required_else_help = true)]
struct Cli {
    /// Output format for JSON command output (version, update, and skill ignore it)
    #[arg(long, global = true, value_enum, default_value = "auto")]
    format: output::FormatArg,
    /// Provider instance to use (from `foac auth <provider> login --instance <name>`);
    /// defaults to the nearest [defaults] setting
    #[arg(short = 'i', long, global = true)]
    instance: Option<String>,
    #[command(subcommand)]
    command: Command,
}

const SKILL_MD: &str = include_str!("../assets/SKILL.md");

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
    /// Interact with Confluence
    Confluence(confluence::Cmd),
    /// Interact with GitHub
    Github(github::Cmd),
    /// Interact with Jira
    Jira(jira::Cmd),
    /// Interact with Linear
    #[command(subcommand)]
    Linear(linear::Cmd),
    /// Interact with Neon
    Neon(neon::Cmd),
    /// Interact with Sentry
    Sentry(sentry::Cmd),
    /// Interact with Slack
    Slack(slack::Cmd),
    /// Interact with Vercel
    Vercel(vercel::Cmd),
    /// Enable or disable providers
    #[command(subcommand, arg_required_else_help = true)]
    Provider(provider::Cmd),
    /// Manage the per-provider agent skills describing how to use this CLI
    #[command(subcommand, arg_required_else_help = true)]
    Skill(SkillCmd),
    /// Download the latest release and refresh any installed foac skills
    Update,
    /// Print the version
    Version,
    /// Show the foac banner, version, and repository
    About,
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
    let instance_flag = cli.instance;
    // The instance a provider command targets: --instance flag, then the
    // nearest [defaults] setting; the resolved provider@instance must not
    // be disabled.
    let provider_instance = |name: &str| -> Result<String, Box<dyn std::error::Error>> {
        let settings = provider::SettingsStore.load()?;
        let instance = provider::resolve_instance(name, instance_flag.as_deref(), &settings)?;
        provider::ensure_enabled(&settings, name, &instance)?;
        Ok(instance)
    };
    let skip_check = matches!(command, Command::Update);
    let result = match command {
        Command::Auth(cmd) => auth::run(cmd, format, instance_flag.clone()),
        Command::Confluence(cmd) => confluence::run(cmd, format, &provider_instance("confluence")?),
        Command::Github(cmd) => github::run(cmd, format, &provider_instance("github")?),
        Command::Jira(cmd) => jira::run(cmd, format, &provider_instance("jira")?),
        Command::Linear(cmd) => linear::run(cmd, format, &provider_instance("linear")?),
        Command::Neon(cmd) => neon::run(cmd, format, &provider_instance("neon")?),
        Command::Sentry(cmd) => sentry::run(cmd, format, &provider_instance("sentry")?),
        Command::Slack(cmd) => slack::run(cmd, format, &provider_instance("slack")?),
        Command::Vercel(cmd) => vercel::run(cmd, format, &provider_instance("vercel")?),
        Command::Provider(cmd) => provider::run(cmd, format, instance_flag.clone()),
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
        Command::About => {
            about();
            Ok(())
        }
    };
    if !skip_check {
        update::notify_if_outdated();
    }
    result
}

/// The brand banner (assets/brand/): the Merge mark in amber, the wordmark,
/// and the tagline, then version and repository. Bypasses the printer and
/// `--format`, like `version`.
fn about() {
    let (amber, grey, bold, reset) = if output::color_enabled() {
        (
            "\x1b[38;2;240;136;62m", // Amber on dark, from assets/brand/README.md
            "\x1b[38;2;140;149;159m",
            "\x1b[1m",
            "\x1b[0m",
        )
    } else {
        ("", "", "", "")
    };
    // The two arms merge into the bar and run out to the right, like the icon.
    let mark = ["━━╲     ", "   ╲    ", "━━━━━━━━", "   ╱    ", "━━╱     "];
    // Lowercase geometric "foac": a hooked f, a bowl, a stemmed a, an open c.
    let wordmark = [
        r" ╭───",
        r" │",
        r"─┼──  ╭──╮  ──╮ ╭──",
        r" │    │  │ ╭──┤ │",
        r" │    ╰──╯ ╰──┘ ╰──",
    ];
    println!();
    for (mark_row, wordmark_row) in mark.iter().zip(wordmark) {
        println!("  {amber}{mark_row}{reset}  {bold}{wordmark_row}{reset}");
    }
    println!();
    println!("            {grey}many services · many agents · one door{reset}");
    println!();
    println!(
        "{} — v{}",
        env!("CARGO_PKG_DESCRIPTION"),
        env!("CARGO_PKG_VERSION")
    );
    println!("{}", env!("CARGO_PKG_REPOSITORY"));
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
    /// on this machine, removing the skills of inactive providers; uses the
    /// global toggles only, ignoring .foac.toml overrides
    Install,
}

fn providers() -> Result<[Provider; 8], Box<dyn std::error::Error>> {
    let settings = provider::SettingsStore.load()?;
    Ok(providers_where(|name| settings.enabled(name)))
}

fn providers_where(enabled: impl Fn(&str) -> bool) -> [Provider; 8] {
    // Settings first: short-circuit skips the keychain/`gh` probe when disabled.
    [
        Provider::new(
            "confluence",
            enabled("confluence") && confluence::authenticated(),
        ),
        Provider::new("github", enabled("github") && github::authenticated()),
        Provider::new("jira", enabled("jira") && jira::authenticated()),
        Provider::new("linear", enabled("linear") && linear::authenticated()),
        Provider::new("neon", enabled("neon") && neon::authenticated()),
        Provider::new("sentry", enabled("sentry") && sentry::authenticated()),
        Provider::new("slack", enabled("slack") && slack::authenticated()),
        Provider::new("vercel", enabled("vercel") && vercel::authenticated()),
    ]
}

fn cli_command(providers: &[Provider]) -> clap::Command {
    let command = Cli::command().help_template(format!(
        // clap's default template with the providers section inserted; clap
        // cannot split subcommands under two headings, so providers are hidden
        // from {all-args} and rendered by hand.
        "{{before-help}}{{about-with-newline}}\n{{usage-heading}} {{usage}}\n\n{}{{all-args}}{{after-help}}",
        providers_help_section(providers)
    ));
    // Hide every provider from the Commands list: active ones are listed in
    // the providers section instead, and hiding doesn't affect parsing or typo
    // suggestions (remove_hidden_provider_suggestions keys off `active`).
    providers.iter().fold(command, |command, provider| {
        command.mut_subcommand(provider.name, |subcommand| subcommand.hide(true))
    })
}

fn providers_help_section(providers: &[Provider]) -> String {
    let command = Cli::command();
    let styles = command.get_styles();
    let (header, literal) = (styles.get_header(), styles.get_literal());
    let active: Vec<&Provider> = providers
        .iter()
        .filter(|provider| provider.active)
        .collect();
    let width = active.iter().map(|provider| provider.name.len()).max();
    let Some(width) = width else {
        return String::new();
    };
    let mut section = format!("{header}Providers:{header:#}\n");
    for provider in active {
        let about = command
            .find_subcommand(provider.name)
            .and_then(clap::Command::get_about)
            .map(ToString::to_string)
            .unwrap_or_default();
        // Pad the plain name so the ANSI codes don't skew the column.
        section.push_str(&format!(
            "  {literal}{name:width$}{literal:#}  {about}\n",
            name = provider.name
        ));
    }
    section.push('\n');
    section
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
    // Skills are installed machine-wide, so load the global settings only:
    // a per-folder .foac.toml override must neither toggle providers here
    // nor, when malformed, block the install.
    let settings = provider::SettingsStore.load_global()?;
    let active: Vec<&str> = providers_where(|name| settings.enabled(name))
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
    if agent_skill_dirs(&home).is_empty() {
        return Err("no supported agent found; install manually with: foac skill print <provider> > <agent skills dir>/foac-<provider>/SKILL.md".into());
    }
    let events = skill_install(&home, &active)?;
    for (action, path) in events {
        println!("{action} {}", path.display());
    }
    Ok(())
}

/// Write one skill per active provider into the skill folders of the agents
/// present under `home`, and remove the skills of inactive providers.
/// Claude Code only reads its own folder; every other major agent (Cursor,
/// Codex, Gemini CLI, Copilot, OpenCode, Amp, Cline, ...) reads the
/// cross-agent standard ~/.agents/skills, so two targets cover them all.
fn agent_skill_dirs(home: &std::path::Path) -> Vec<std::path::PathBuf> {
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
    targets
}

fn skill_install(
    home: &std::path::Path,
    active: &[&str],
) -> Result<Vec<(&'static str, std::path::PathBuf)>, Box<dyn std::error::Error>> {
    let mut events = Vec::new();
    for skills_dir in agent_skill_dirs(home) {
        for name in provider::PROVIDERS {
            let dir = skills_dir.join(format!("foac-{name}"));
            if active.contains(&name) {
                std::fs::create_dir_all(&dir)?;
                let path = dir.join("SKILL.md");
                let contents = render_provider_skill(name);
                let action = match std::fs::read(&path) {
                    Ok(existing) if existing == contents.as_bytes() => {
                        events.push(("Unchanged", path));
                        continue;
                    }
                    Ok(_) => "Updated",
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => "Installed",
                    Err(err) => return Err(err.into()),
                };
                std::fs::write(&path, contents)?;
                events.push((action, path));
            } else {
                match std::fs::remove_dir_all(&dir) {
                    Ok(()) => events.push(("Removed", dir)),
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => return Err(err.into()),
                }
            }
        }
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, error::ErrorKind};

    use super::*;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    /// Every JSON-emitting provider leaf command must document its output
    /// contract (`#[command(after_long_help = outdoc::...)]`), and only leaf
    /// commands may carry one.
    #[test]
    fn every_provider_command_documents_its_output() {
        fn walk(command: &clap::Command, path: &str) {
            let subcommands: Vec<&clap::Command> = command
                .get_subcommands()
                .filter(|sub| sub.get_name() != "help")
                .collect();
            let help = command.get_after_long_help().map(ToString::to_string);
            if subcommands.is_empty() {
                let help = help.unwrap_or_default();
                assert!(
                    help.starts_with("Output:"),
                    "`foac {path}` emits JSON but its --help has no Output section; \
                     attach #[command(after_long_help = outdoc::...)] to the verb"
                );
            } else {
                assert!(
                    help.is_none(),
                    "`foac {path}`: Output sections belong on leaf verbs only"
                );
                for sub in subcommands {
                    walk(sub, &format!("{path} {}", sub.get_name()));
                }
            }
        }
        let mut command = Cli::command();
        command.build();
        for provider in provider::PROVIDERS {
            walk(command.find_subcommand(provider).unwrap(), provider);
        }
    }

    fn long_help(args: &[&str]) -> String {
        let error = match Cli::try_parse_from(args) {
            Ok(_) => panic!("--help should stop parsing"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
        error.to_string()
    }

    #[test]
    fn list_help_documents_records_and_pagination() {
        // The motivating case: linear lists are GraphQL connections, not the
        // REST items/pageInfo wrapper.
        let help = long_help(&["foac", "linear", "user", "list", "--help"]);
        assert!(help.contains("users.nodes"));
        assert!(help.contains("users.pageInfo.endCursor"));
        assert!(help.contains("--after while hasNextPage is true"));
        let help = long_help(&["foac", "github", "issue", "list", "--help"]);
        assert!(help.contains("\"items\": [<record>, ...]"));
        assert!(help.contains("pageInfo.nextPage to --page"));
        assert!(help.contains("pull requests are filtered out"));
    }

    #[test]
    fn get_and_mutation_help_document_the_envelope() {
        let help = long_help(&["foac", "linear", "issue", "get", "--help"]);
        assert!(help.contains("{\"issue\": {...}}"));
        assert!(help.contains("issue.identifier"));
        let help = long_help(&["foac", "linear", "issue", "create", "--help"]);
        assert!(help.contains("{\"issueCreate\": {\"success\": true, \"issue\": {...}}}"));
        let help = long_help(&["foac", "github", "comment", "delete", "--help"]);
        assert!(help.contains("{} on success"));
    }

    #[test]
    fn short_help_stays_compact() {
        let error = match Cli::try_parse_from(["foac", "linear", "user", "list", "-h"]) {
            Ok(_) => panic!("-h should stop parsing"),
            Err(error) => error,
        };
        assert!(!error.to_string().contains("Output:"));
    }

    #[test]
    fn help_only_lists_authenticated_providers() {
        for expected in [
            vec![],
            vec!["linear"],
            vec!["github"],
            vec!["sentry"],
            vec!["slack"],
            vec!["jira"],
            vec!["confluence"],
            vec!["neon"],
            vec!["vercel"],
            provider::PROVIDERS.to_vec(),
        ] {
            let providers = test_providers(&expected);
            for args in [vec!["foac"], vec!["foac", "--help"]] {
                let help = parse_error(&providers, args).to_string();
                for name in [
                    "confluence",
                    "github",
                    "jira",
                    "linear",
                    "neon",
                    "sentry",
                    "slack",
                    "vercel",
                ] {
                    assert_eq!(help_lists(&help, name), expected.contains(&name));
                }
                for name in [
                    "about", "auth", "provider", "skill", "update", "version", "help",
                ] {
                    assert!(help_lists(&help, name));
                }
                if expected.is_empty() {
                    assert!(!help.contains("Providers:"));
                } else {
                    let providers_at = help.find("Providers:").unwrap();
                    assert!(providers_at < help.find("Commands:").unwrap());
                }
            }
        }
    }

    #[test]
    fn hidden_providers_still_parse() {
        let providers = test_providers(&[]);
        for args in [
            vec!["foac", "confluence", "page", "list"],
            vec!["foac", "github", "issue", "list", "--repo", "owner/repo"],
            vec!["foac", "jira", "issue", "list", "--jql", "project = ENG"],
            vec!["foac", "linear", "team", "list"],
            vec!["foac", "neon", "project", "list"],
            vec!["foac", "sentry", "issue", "list", "--org", "acme"],
            vec!["foac", "slack", "conversation", "list"],
            vec!["foac", "vercel", "project", "list"],
        ] {
            try_parse_from(&providers, args).unwrap();
        }
    }

    #[test]
    fn hidden_provider_help_remains_available() {
        let providers = test_providers(&[]);
        for (args, usage) in [
            (
                vec!["foac", "confluence", "--help"],
                "Usage: foac confluence",
            ),
            (vec!["foac", "github", "--help"], "Usage: foac github"),
            (vec!["foac", "jira", "--help"], "Usage: foac jira"),
            (vec!["foac", "linear", "--help"], "Usage: foac linear"),
            (vec!["foac", "neon", "--help"], "Usage: foac neon"),
            (vec!["foac", "sentry", "--help"], "Usage: foac sentry"),
            (vec!["foac", "slack", "--help"], "Usage: foac slack"),
            (vec!["foac", "vercel", "--help"], "Usage: foac vercel"),
        ] {
            let error = parse_error(&providers, args);
            assert_eq!(error.kind(), ErrorKind::DisplayHelp);
            assert!(error.to_string().contains(usage));
        }
    }

    #[test]
    fn hidden_providers_are_not_suggested() {
        let error = parse_error(&test_providers(&[]), ["foac", "githu"]).to_string();
        assert!(!error.contains("github"));

        let error = parse_error(&test_providers(&["github"]), ["foac", "githu"]).to_string();
        assert!(error.contains("github"));
    }

    #[test]
    fn skill_documents_one_provider() {
        let examples = [
            ("confluence", "foac confluence page list"),
            ("github", "foac github issue list"),
            ("jira", "foac jira issue list"),
            ("linear", "foac linear issue list"),
            ("neon", "foac neon branch list"),
            ("sentry", "foac sentry issue list"),
            ("slack", "foac slack search"),
            ("vercel", "foac vercel project list"),
        ];
        for (name, _) in examples {
            let skill = render_provider_skill(name);
            assert!(skill.starts_with(&format!("---\nname: foac-{name}\ndescription:")));
            assert_eq!(skill.matches("name: foac-").count(), 1);
            assert!(skill.contains(&format!("# foac-{name}")));
            for (other, other_example) in examples {
                assert_eq!(skill.contains(other_example), other == name);
            }
            // The cross-provider join example is shared: every skill gets it.
            assert!(skill.contains("foac linear user list | foac slack user get --from email"));
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
            vec!["foac", "auth", "neon"],
            vec!["foac", "auth", "jira"],
            vec!["foac", "auth", "confluence"],
            vec!["foac", "auth", "sentry"],
            vec!["foac", "auth", "slack"],
            vec!["foac", "auth", "vercel"],
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
            vec!["foac", "auth", "neon", "status"],
            vec!["foac", "auth", "neon", "login"],
            vec!["foac", "auth", "neon", "logout"],
            vec!["foac", "auth", "jira", "status"],
            vec!["foac", "auth", "jira", "login"],
            vec![
                "foac",
                "auth",
                "jira",
                "login",
                "--host",
                "acme.atlassian.net",
                "--email",
                "user@example.com",
            ],
            vec!["foac", "auth", "jira", "logout"],
            vec!["foac", "auth", "confluence", "status"],
            vec!["foac", "auth", "confluence", "login"],
            vec![
                "foac",
                "auth",
                "confluence",
                "login",
                "--host",
                "acme.atlassian.net",
                "--email",
                "user@example.com",
            ],
            vec!["foac", "auth", "confluence", "logout"],
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
            vec!["foac", "auth", "vercel", "status"],
            vec!["foac", "auth", "vercel", "login"],
            vec!["foac", "auth", "vercel", "logout"],
            vec!["foac", "auth", "slack", "login", "--instance", "workb"],
            vec!["foac", "auth", "slack", "logout", "-i", "workb"],
        ] {
            Cli::try_parse_from(args).unwrap();
        }
        // --host is a Sentry and Jira login flag; clap rejects it elsewhere.
        for provider in ["linear", "github", "neon", "slack", "vercel"] {
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
            vec!["foac", "provider", "enable", "jira"],
            vec!["foac", "provider", "disable", "jira"],
            vec!["foac", "provider", "enable", "confluence"],
            vec!["foac", "provider", "disable", "confluence"],
            vec!["foac", "provider", "enable", "neon"],
            vec!["foac", "provider", "disable", "neon"],
            vec!["foac", "provider", "enable", "sentry"],
            vec!["foac", "provider", "disable", "sentry"],
            vec!["foac", "provider", "enable", "slack"],
            vec!["foac", "provider", "disable", "slack"],
            vec!["foac", "provider", "enable", "github", "--local"],
            vec!["foac", "provider", "disable", "slack", "--local"],
            vec![
                "foac",
                "provider",
                "disable",
                "slack",
                "--instance",
                "workb",
                "--local",
            ],
            vec!["foac", "provider", "enable", "slack", "-i", "workb"],
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
    fn instance_flag_parses_at_any_position() {
        for args in [
            vec!["foac", "-i", "workb", "slack", "conversation", "list"],
            vec![
                "foac",
                "slack",
                "conversation",
                "list",
                "--instance",
                "workb",
            ],
            vec!["foac", "slack", "-i", "workb", "conversation", "list"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert_eq!(cli.instance.as_deref(), Some("workb"));
        }
        assert!(
            Cli::try_parse_from(["foac", "slack", "conversation", "list"])
                .unwrap()
                .instance
                .is_none()
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
    fn get_verbs_parse_the_piped_join_form() {
        // Positional omitted, with or without --from: pipe mode.
        for args in [
            vec!["foac", "linear", "issue", "get", "--from", "identifier"],
            vec!["foac", "linear", "user", "get"],
            vec!["foac", "github", "issue", "get", "--from", "number"],
            vec!["foac", "jira", "issue", "get", "--from", "key"],
            vec!["foac", "confluence", "page", "get", "--from", "id"],
            vec!["foac", "neon", "branch", "get", "--from", "id"],
            vec![
                "foac", "sentry", "issue", "get", "--org", "acme", "--from", "shortId",
            ],
            vec!["foac", "slack", "user", "get", "--from", "email"],
            vec!["foac", "slack", "message", "get", "#eng", "--from", "ts"],
            vec!["foac", "vercel", "project", "get", "--from", "name"],
            vec![
                "foac",
                "vercel",
                "project-domain",
                "get",
                "--project",
                "web",
                "--from",
                "name",
            ],
        ] {
            Cli::try_parse_from(args).unwrap();
        }
        // The single-value form is unchanged.
        Cli::try_parse_from(["foac", "linear", "issue", "get", "ENG-123"]).unwrap();
        Cli::try_parse_from(["foac", "github", "pull", "get", "42"]).unwrap();
    }

    #[test]
    fn skill_requires_subcommand() {
        assert!(Cli::try_parse_from(["foac", "skill"]).is_err());
        let missing_provider = Cli::try_parse_from(["foac", "skill", "print"])
            .err()
            .unwrap()
            .to_string();
        assert!(missing_provider.contains("<PROVIDER>"));
        assert!(
            missing_provider
                .contains("possible values: confluence, github, jira, linear, neon, sentry, slack")
        );
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
        let existing = home.join(".claude/skills/foac-github");
        std::fs::create_dir_all(&existing).unwrap();
        std::fs::write(existing.join("SKILL.md"), "stale").unwrap();
        let events = skill_install(&home, &["github", "linear"]).unwrap();
        assert_eq!(
            events,
            vec![
                ("Updated", home.join(".claude/skills/foac-github/SKILL.md")),
                (
                    "Installed",
                    home.join(".claude/skills/foac-linear/SKILL.md")
                ),
                (
                    "Installed",
                    home.join(".agents/skills/foac-github/SKILL.md")
                ),
                (
                    "Installed",
                    home.join(".agents/skills/foac-linear/SKILL.md")
                ),
                ("Removed", stale.clone()),
            ]
        );
        for (_, path) in events.iter().filter(|(action, _)| *action != "Removed") {
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

    #[test]
    fn skill_install_skips_identical_skills() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".claude")).unwrap();
        let skill = home.path().join(".claude/skills/foac-github/SKILL.md");
        std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
        std::fs::write(&skill, render_provider_skill("github")).unwrap();

        let events = skill_install(home.path(), &["github"]).unwrap();

        assert_eq!(events, vec![("Unchanged", skill.clone())]);
        assert_eq!(
            std::fs::read_to_string(skill).unwrap(),
            render_provider_skill("github")
        );
    }

    fn test_providers(active: &[&str]) -> [Provider; 8] {
        [
            Provider::new("confluence", active.contains(&"confluence")),
            Provider::new("github", active.contains(&"github")),
            Provider::new("jira", active.contains(&"jira")),
            Provider::new("linear", active.contains(&"linear")),
            Provider::new("neon", active.contains(&"neon")),
            Provider::new("sentry", active.contains(&"sentry")),
            Provider::new("slack", active.contains(&"slack")),
            Provider::new("vercel", active.contains(&"vercel")),
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
