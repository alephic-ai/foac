use clap::{Parser, Subcommand};
use self_update::cargo_crate_version;

mod linear;

#[derive(Parser)]
#[command(about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
// One short-lived value on the stack; boxing the variant isn't worth it.
#[allow(clippy::large_enum_variant)]
enum Command {
    /// Interact with Linear (linear.app), requires LINEAR_API_KEY
    #[command(subcommand)]
    Linear(linear::Cmd),
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
}
