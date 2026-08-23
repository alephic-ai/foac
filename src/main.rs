use clap::{Parser, Subcommand};

mod auth;
mod github;
mod linear;
mod update;

#[derive(Parser)]
#[command(about, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

const SKILL_MD: &str = include_str!("../doc/SKILL.md");

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
    let command = Cli::parse().command;
    let skip_check = matches!(command, Command::Update);
    let result = match command {
        Command::Auth(cmd) => auth::run(cmd),
        Command::Github(cmd) => github::run(cmd),
        Command::Linear(cmd) => linear::run(cmd),
        Command::Skill(SkillCmd::Print) => {
            print!("{SKILL_MD}");
            Ok(())
        }
        Command::Skill(SkillCmd::Install) => skill_install_cmd(),
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

fn skill_install_cmd() -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::home_dir().ok_or("could not determine the home directory")?;
    let installed = skill_install(&home)?;
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
        std::fs::write(&path, SKILL_MD)?;
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
    fn bare_auth_commands_display_help() {
        for args in [
            vec!["foac", "auth"],
            vec!["foac", "auth", "linear"],
            vec!["foac", "auth", "github"],
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
        let installed = skill_install(&home).unwrap();
        assert_eq!(
            installed,
            vec![
                home.join(".claude/skills/foac/SKILL.md"),
                home.join(".agents/skills/foac/SKILL.md"),
            ]
        );
        assert_eq!(std::fs::read_to_string(&installed[0]).unwrap(), SKILL_MD);
        std::fs::remove_dir_all(&home).unwrap();
    }
}
