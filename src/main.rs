use clap::{Parser, Subcommand};
use self_update::cargo_crate_version;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Download and replace this binary with the latest GitHub release
    Update,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Some(Command::Update) => update(),
        None => {
            println!("{}", env!("CARGO_PKG_DESCRIPTION"));
            Ok(())
        }
    }
}

fn update() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = self_update::backends::github::Update::configure();
    builder
        .repo_owner("lra")
        .repo_name("foac")
        .bin_name("foac")
        .show_download_progress(true)
        .no_confirm(true)
        .current_version(cargo_crate_version!());
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
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
