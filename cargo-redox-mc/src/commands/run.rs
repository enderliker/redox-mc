//! `cargo redox-mc run`: launch a local test environment with the agent injected.

use crate::cli::RunArgs;
use crate::commands::build::build_project;
use crate::error::CliError;
use crate::project::ModProject;
use crate::repo::RedoxRepo;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

/// Which game process to launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchTarget {
    Server,
    Client,
}

/// A fully described JVM invocation, kept separate from spawning so it can be
/// asserted in tests without running Java.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
}

impl LaunchCommand {
    /// Runs the command with inherited stdin/stdout/stderr and waits for it.
    pub fn spawn(&self) -> Result<ExitStatus, CliError> {
        std::process::Command::new(&self.program)
            .args(&self.args)
            .current_dir(&self.cwd)
            .status()
            .map_err(|error| {
                CliError::message(format!(
                    "failed to launch '{}': {error}",
                    self.program.to_string_lossy()
                ))
            })
    }
}

/// Builds the `java -agentpath:... -jar ...` command for a target.
pub fn build_launch_command(
    program: &OsStr,
    agent: &Path,
    jar: &Path,
    target: LaunchTarget,
    cwd: &Path,
) -> LaunchCommand {
    let mut args = vec![OsString::from(format!("-agentpath:{}", agent.display()))];
    args.push(OsString::from("-jar"));
    args.push(jar.as_os_str().to_os_string());
    if target == LaunchTarget::Server {
        args.push(OsString::from("nogui"));
    }

    LaunchCommand {
        program: program.to_os_string(),
        args,
        cwd: cwd.to_path_buf(),
    }
}

pub fn execute(args: RunArgs) -> Result<(), CliError> {
    let target = match (args.server, args.client) {
        (_, true) => LaunchTarget::Client,
        _ => LaunchTarget::Server,
    };

    let project = match args.manifest_path {
        Some(manifest_path) => ModProject::from_manifest(&manifest_path)?,
        None => ModProject::find(&env::current_dir()?)?,
    };
    let repo = RedoxRepo::discover(&project.root)?;

    if !args.no_build {
        build_project(&project, args.release)?;
    }

    let run_dir = project.root.join(".redox-mc").join(match target {
        LaunchTarget::Server => "server",
        LaunchTarget::Client => "client",
    });
    fs::create_dir_all(&run_dir)?;

    if target == LaunchTarget::Server {
        write_server_config(&run_dir, args.port)?;
    }

    let staged = project.root.join(".redox-mc/dist").join(&project.mod_id);
    if staged.is_dir() {
        copy_dir(&staged, &run_dir.join("mods").join(&project.mod_id))?;
    }

    let agent = repo.agent_library()?;
    let jar = match args.jar {
        Some(jar) => jar,
        None => match target {
            LaunchTarget::Server => repo.server_jar()?,
            LaunchTarget::Client => repo.client_jar()?,
        },
    };

    println!(
        "[redox-mc] Launching Minecraft {} from '{}' with agent '{}'",
        match target {
            LaunchTarget::Server => "server",
            LaunchTarget::Client => "client",
        },
        jar.display(),
        agent.display()
    );

    let java = env::var_os("REDOX_MC_JAVA").unwrap_or_else(|| OsString::from("java"));
    let command = build_launch_command(&java, &agent, &jar, target, &run_dir);
    let status = command.spawn()?;
    if !status.success() {
        return Err(CliError::message(format!(
            "Minecraft {} exited with {status}",
            match target {
                LaunchTarget::Server => "server",
                LaunchTarget::Client => "client",
            }
        )));
    }

    Ok(())
}

fn write_server_config(run_dir: &Path, port: Option<u16>) -> Result<(), CliError> {
    let eula = run_dir.join("eula.txt");
    if !eula.exists() {
        fs::write(&eula, "eula=true\n")?;
    }

    let properties = run_dir.join("server.properties");
    if !properties.exists() {
        fs::write(
            &properties,
            default_server_properties(port.unwrap_or(25565)),
        )?;
        return Ok(());
    }

    if let Some(port) = port {
        let text = fs::read_to_string(&properties)?;
        fs::write(
            &properties,
            upsert_property(&text, "server-port", &port.to_string()),
        )?;
    }

    Ok(())
}

fn default_server_properties(port: u16) -> String {
    format!(
        "online-mode=false\n\
         server-port={port}\n\
         sync-chunk-writes=false\n\
         level-type=minecraft:flat\n\
         gamemode=survival\n\
         difficulty=peaceful\n\
         spawn-protection=0\n"
    )
}

/// Replaces `key=...` in a properties file, appending it when absent.
fn upsert_property(text: &str, key: &str, value: &str) -> String {
    let prefix = format!("{key}=");
    let mut replaced = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| {
            if !replaced && line.starts_with(&prefix) {
                replaced = true;
                format!("{key}={value}")
            } else {
                line.to_string()
            }
        })
        .collect();

    if !replaced {
        lines.push(format!("{key}={value}"));
    }

    let mut output = lines.join("\n");
    output.push('\n');
    output
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), CliError> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    Ok(())
}
