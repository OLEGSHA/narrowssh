#![warn(clippy::suspicious)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::pedantic)]

use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};

use narrowssh::config::ControlManager;
use narrowssh::workspace::Workspace;

/// Manage allowlisted SSH commands for one or more users.
///
/// When run as root, affects all users unless overridden by --user; otherwise
/// affects only the executing user.
#[derive(Parser)]
#[command(author, version, about, long_about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Limit affected users to given username. Incompatible with --uid.
    #[arg(short, long)]
    user: Option<String>,

    /// Limit affected users to given user ID. Incompatible with --user.
    #[arg(long)]
    uid: Option<u32>,
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

    let control_manager = ControlManager::load(&ws, MAIN_CONTROL_FILE)?;
    let users = resolve_users(&cli.user, cli.uid, &control_manager, &ws)?;

    if users.is_empty() {
        bail!("All users are disabled in {}", MAIN_CONTROL_FILE);
    }

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
fn resolve_users<'a, W>(
    username: &Option<String>,
    uid: Option<u32>,
    control_manager: &ControlManager,
    ws: &'a W,
) -> Result<Vec<&'a uzers::User>>
where
    W: Workspace,
{
    if username.is_some() && uid.is_some() {
        bail!("--user and --uid are incompatible");
    }

    if let Some(username) = username {
        return ws
            .users()
            .user_by_username(username)?
            .map(|u| vec![u])
            .ok_or(anyhow!("No such user exists"));
    }

    if let Some(uid) = uid {
        return ws
            .users()
            .user_by_uid(uid)
            .map(|u| vec![u])
            .ok_or(anyhow!("No such user exists"));
    }

    let current_uid = ws.users().current_uid();
    Ok(if current_uid == 0 {
        ws.users()
            .all_users()
            .filter(|u| control_manager.get_user_control(u.uid()).enable)
            .collect()
    } else {
        vec![ws
            .users()
            .user_by_uid(current_uid)
            .expect("Current user does not exist")]
    })
}
