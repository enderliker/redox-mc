//! Discovering the Redox-MC checkout and the assets needed to launch a server.

use crate::error::CliError;
use crate::project::cargo_target_dir;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const AGENT_CRATE: &str = "redox_mc_loader";

/// A Redox-MC checkout, used to locate the agent library and the Minecraft jar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedoxRepo {
    pub root: PathBuf,
}

impl RedoxRepo {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Finds the checkout via `REDOX_MC_HOME`, then by walking up from `start`.
    pub fn discover(start: &Path) -> Result<Self, CliError> {
        if let Some(home) = env::var_os("REDOX_MC_HOME") {
            let root = PathBuf::from(home);
            if is_repo_root(&root) {
                return Ok(Self { root });
            }
        }

        let mut dir = if start.is_file() {
            start
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        } else {
            start.to_path_buf()
        };

        loop {
            if is_repo_root(&dir) {
                return Ok(Self { root: dir });
            }
            match dir.parent() {
                Some(parent) if parent != dir => dir = parent.to_path_buf(),
                _ => break,
            }
        }

        Err(CliError::RepoNotFound(start.to_path_buf()))
    }

    /// Path to the `redox-mc-api` crate, used as a path dependency.
    pub fn api_path(&self) -> PathBuf {
        self.root.join("redox-mc-api")
    }

    /// Directory where the agent is built.
    pub fn target_dir(&self) -> PathBuf {
        cargo_target_dir(&self.root)
    }

    /// Resolves the loader agent shared library.
    ///
    /// `REDOX_MC_AGENT_LIB` wins; otherwise the newest build under `target/` is
    /// used, checking `debug`, `release` and cross-compilation triples.
    pub fn agent_library(&self) -> Result<PathBuf, CliError> {
        if let Some(explicit) = env::var_os("REDOX_MC_AGENT_LIB") {
            let path = PathBuf::from(explicit);
            if path.is_file() {
                return Ok(path);
            }
            return Err(CliError::MissingFile {
                path,
                hint: "REDOX_MC_AGENT_LIB does not point to a file".to_string(),
            });
        }

        let file_name = format!(
            "{}{}{}",
            std::env::consts::DLL_PREFIX,
            AGENT_CRATE,
            std::env::consts::DLL_SUFFIX
        );
        let target = self.target_dir();
        find_artifact(&target, &file_name).ok_or(CliError::ArtifactNotFound {
            root: target,
            file_name,
        })
    }

    /// Resolves the Minecraft server bundler jar.
    pub fn server_jar(&self) -> Result<PathBuf, CliError> {
        resolve_jar(
            "REDOX_MC_SERVER_JAR",
            "versions/26.3/server.jar",
            &self.root,
            true,
        )
    }

    /// Resolves the Minecraft client bundler jar.
    pub fn client_jar(&self) -> Result<PathBuf, CliError> {
        resolve_jar(
            "REDOX_MC_CLIENT_JAR",
            "versions/26.3/client.jar",
            &self.root,
            false,
        )
    }
}

fn is_repo_root(dir: &Path) -> bool {
    dir.join("redox-mc-loader/Cargo.toml").is_file()
        && dir.join("redox-mc-api/Cargo.toml").is_file()
}

fn resolve_jar(
    env_var: &str,
    relative: &str,
    root: &Path,
    bundler_fallback: bool,
) -> Result<PathBuf, CliError> {
    if let Some(explicit) = env::var_os(env_var) {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Ok(path);
        }
        return Err(CliError::MissingFile {
            path,
            hint: format!("{env_var} does not point to a file"),
        });
    }

    if bundler_fallback {
        if let Some(bundler) = env::var_os("MINECRAFT_BUNDLER_JAR") {
            let path = PathBuf::from(bundler);
            if path.is_file() {
                return Ok(path);
            }
        }
    }

    let candidates = [
        root.join(relative),
        env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(basename(relative)),
    ];
    for candidate in candidates.iter() {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }

    Err(CliError::MissingFile {
        path: root.join(relative),
        hint: "run scripts/setup_mc_env.sh or set the matching REDOX_MC_*_JAR variable".to_string(),
    })
}

fn basename(relative: &str) -> &str {
    relative.rsplit('/').next().unwrap_or(relative)
}

/// Searches a Cargo target directory for an artifact.
///
/// Order: `debug`, `release`, then any cross-compilation triple directory. The
/// first match wins, so a native build is preferred over a cross build.
pub fn find_artifact(target_root: &Path, file_name: &str) -> Option<PathBuf> {
    find_artifact_for_profile(target_root, file_name, None)
}

/// Searches a Cargo target directory for an artifact, preferring the given profile.
pub fn find_artifact_for_profile(
    target_root: &Path,
    file_name: &str,
    preferred_profile: Option<&str>,
) -> Option<PathBuf> {
    let profiles: &[&str] = match preferred_profile {
        Some("release") => &["release", "debug"],
        Some("debug") => &["debug", "release"],
        _ => &["debug", "release"],
    };

    for profile in profiles {
        let candidate = target_root.join(profile).join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    let mut triples: Vec<PathBuf> = fs::read_dir(target_root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    triples.sort();

    for triple in triples {
        for profile in profiles {
            let candidate = triple.join(profile).join(file_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}
