use clap::{Parser, Subcommand};
use self_update::cargo_crate_version;

mod linear;

#[derive(Parser)]
#[command(about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

const SKILL_MD: &str = include_str!("../doc/SKILL.md");

#[derive(Subcommand)]
// One short-lived value on the stack; boxing the variant isn't worth it.
#[allow(clippy::large_enum_variant)]
enum Command {
    /// Interact with Linear (linear.app), requires LINEAR_API_KEY
    #[command(subcommand)]
    Linear(linear::Cmd),
    /// Print an agent skill (SKILL.md) describing how to use this CLI
    Skill {
        #[command(subcommand)]
        command: Option<SkillCmd>,
    },
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
    match Cli::parse().command {
        Some(Command::Linear(cmd)) => linear::run(cmd),
        Some(Command::Skill { command: None }) => {
            print!("{SKILL_MD}");
            Ok(())
        }
        Some(Command::Skill { command: Some(SkillCmd::Install) }) => {
            let home = std::env::home_dir().ok_or("could not determine the home directory")?;
            let installed = skill_install(&home)?;
            if installed.is_empty() {
                return Err("no supported agent found; install manually with: foac skill > <agent skills dir>/foac/SKILL.md".into());
            }
            for path in installed {
                println!("Installed {}", path.display());
            }
            Ok(())
        }
        Some(Command::Update) => update(),
        Some(Command::Version) => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        None => {
            println!("{}", env!("CARGO_PKG_DESCRIPTION"));
            Ok(())
        }
    }
}

#[derive(Subcommand)]
enum SkillCmd {
    /// Install the skill for every supported agent found on this machine
    Install,
}

/// Write the skill into the skill folders of the agents present under `home`.
/// Claude Code only reads its own folder; every other major agent (Cursor,
/// Codex, Gemini CLI, Copilot, OpenCode, Amp, Cline, ...) reads the
/// cross-agent standard ~/.agents/skills, so two writes cover them all.
fn skill_install(home: &std::path::Path) -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error>> {
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

fn update() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty());
    // Ask /releases/latest for the tag to install: that endpoint never returns
    // draft or prerelease releases, unlike the /releases listing self_update
    // walks by default, which includes in-progress drafts when authenticated.
    let mut request = reqwest::blocking::Client::new()
        .get("https://api.github.com/repos/lra/foac/releases/latest")
        .header("User-Agent", "foac");
    if let Some(token) = &token {
        request = request.bearer_auth(token);
    }
    let latest: serde_json::Value = request.send()?.error_for_status()?.json()?;
    let tag = latest["tag_name"]
        .as_str()
        .ok_or("no tag_name in the latest GitHub release")?;
    if tag == format!("v{}", cargo_crate_version!()) {
        println!("Already up to date ({})", cargo_crate_version!());
        return Ok(());
    }
    let mut builder = self_update::backends::github::Update::configure();
    builder
        .repo_owner("lra")
        .repo_name("foac")
        .bin_name("foac")
        .show_download_progress(true)
        .no_confirm(true)
        .release_tag(tag)
        // Assets pair an archive with a .sha256 file per target, and
        // self_update takes the first name containing the target — which is
        // the checksum, alphabetically. Require the archive extension.
        .asset_identifier(if cfg!(windows) { ".zip" } else { ".tar.gz" })
        .current_version(cargo_crate_version!());
    if let Some(token) = token {
        builder.auth_token(token);
    }
    let status = builder.build()?.update()?;
    if status.is_updated() {
        println!("Updated to {}", status.version());
    } else {
        println!("Already up to date ({})", status.version());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
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
