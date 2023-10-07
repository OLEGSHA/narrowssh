#![warn(clippy::suspicious)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::pedantic)]

use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};

use narrowssh::config::ControlManager;
use narrowssh::workspace::Workspace;

/// Manage allowlisted SSH commands for one or more users.
#[derive(Parser)]
#[command(author, version, about, long_about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Affect user with given username instead of running user.
    ///
    /// Incompatible with --uid and --all-users.
    #[arg(short, long)]
    user: Option<String>,

    /// Affect user with given user ID instead of running user.
    ///
    /// Incompatible with --user and --all-users.
    #[arg(long)]
    uid: Option<u32>,

    /// Affect all users according to the control file.
    ///
    /// Incompatible with --user and --uid.
    #[arg(short, long)]
    all_users: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Install or update allowlisted SSH commands for one or all users.
    Refresh,

    /// Purge SSH allowlist setup from one or all users.
    Uninstall,
}

/// Absolute path to main control file.
pub const MAIN_CONTROL_FILE: &str = "/etc/narrowssh/control.toml";

fn main() {
    if let Err(err) = try_main() {
        eprintln!("narrowssh: {}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| eprintln!("  - {}", cause));
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let cli = Cli::parse();
    let ws = unsafe { narrowssh::workspace::RealWorkspace::new() };

    let users = resolve_users(&cli, &ws)?;

    println!("Affecting users {users:?}");

    match &cli.command {
        Commands::Refresh => {
            println!("Refreshing");
        }
        Commands::Uninstall => {
            println!("Uninstalling");
        }
    }

    Ok(())
}

/// Returns all users that should be affected.
///
/// If `--all-users` is set, control file is read, parsed and discarded.
fn resolve_users<'a, W>(cli: &Cli, ws: &'a W) -> Result<Vec<&'a uzers::User>>
where
    W: Workspace,
{
    // Count enabled user selection flags
    if i32::from(cli.user.is_some())
        + i32::from(cli.uid.is_some())
        + i32::from(cli.all_users)
        > 1
    {
        bail!("Only one of --user, --uid and --all-users is allowed");
    }

    if let Some(username) = &cli.user {
        return ws
            .users()
            .user_by_username(username)?
            .map(|u| vec![u])
            .ok_or(anyhow!("No such user exists"));
    }

    if let Some(uid) = cli.uid {
        return ws
            .users()
            .user_by_uid(uid)
            .map(|u| vec![u])
            .ok_or(anyhow!("No such user exists"));
    }

    if cli.all_users {
        let control_manager = ControlManager::load(ws, MAIN_CONTROL_FILE)?;

        let result: Vec<_> = ws
            .users()
            .all_users()
            .filter(|u| control_manager.get_user_control(u.uid()).enable)
            .collect();

        if result.is_empty() {
            bail!("All users are disabled in {}", MAIN_CONTROL_FILE);
        }

        return Ok(result);
    }

    Ok(vec![ws
        .users()
        .user_by_uid(ws.users().current_uid())
        .expect("Current user does not exist")])
}
