//! Command-line surface for `cargo redox-mc`.
//!
//! Cargo invokes an external subcommand as `cargo-<name> <name> <args...>`
//! (see <https://doc.rust-lang.org/cargo/reference/external-tools.html#custom-subcommands>),
//! so `cargo redox-mc run --server` reaches this binary as
//! `cargo-redox-mc redox-mc run --server`. [`normalize_args`] strips that
//! duplicated subcommand token, while still allowing direct invocation.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Name Cargo forwards as the second argument of the process.
pub const SUBCOMMAND_NAME: &str = "redox-mc";

/// Removes the subcommand name Cargo inserts right after `argv[0]`.
pub fn normalize_args(mut args: Vec<String>) -> Vec<String> {
    if args.len() >= 2 && args[1] == SUBCOMMAND_NAME {
        args.remove(1);
    }
    args
}

#[derive(Parser, Debug)]
#[command(
    name = "cargo-redox-mc",
    bin_name = "cargo redox-mc",
    version,
    about = "Scaffold, build and run Redox-MC native mods"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a mod project with a `mod.toml` and a `cdylib` crate
    New(NewArgs),
    /// Compile the mod and stage the cdylib next to its `mod.toml`
    Build(BuildArgs),
    /// Launch a local test environment with the loader agent injected
    Run(RunArgs),
}

#[derive(Args, Debug)]
pub struct NewArgs {
    /// Mod name: used to derive the mod id, crate name and directory
    pub name: String,

    /// Parent directory where the project is created
    #[arg(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,

    /// Target side written to `mod.toml`
    #[arg(long, value_enum, default_value_t = SideArg::Both)]
    pub side: SideArg,

    /// Author written to `mod.toml`
    #[arg(long, value_name = "NAME")]
    pub author: Option<String>,
}

#[derive(Args, Debug)]
pub struct BuildArgs {
    /// Build with optimizations
    #[arg(long)]
    pub release: bool,

    /// Path to the mod's `Cargo.toml` (defaults to the nearest one)
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Run a headless server (default)
    #[arg(long, group = "target")]
    pub server: bool,

    /// Run the Minecraft client instead of the server
    #[arg(long, group = "target")]
    pub client: bool,

    /// Build with optimizations before launching
    #[arg(long)]
    pub release: bool,

    /// Skip the automatic build step
    #[arg(long)]
    pub no_build: bool,

    /// Server port written to `server.properties` on first launch
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Path to the mod's `Cargo.toml` (defaults to the nearest one)
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Override the server/client jar to launch
    #[arg(long, value_name = "PATH")]
    pub jar: Option<PathBuf>,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideArg {
    Both,
    Server,
    Client,
}

impl SideArg {
    /// `mod.toml` representation of this side.
    pub fn as_str(self) -> &'static str {
        match self {
            SideArg::Both => "both",
            SideArg::Server => "server",
            SideArg::Client => "client",
        }
    }
}
