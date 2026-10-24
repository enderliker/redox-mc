//! Locating and describing a mod project on disk.

use crate::error::CliError;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Minimal view of a `Cargo.toml` needed to find the built cdylib.
#[derive(Debug, Deserialize)]
struct CargoManifest {
    package: CargoPackage,
    #[serde(default)]
    lib: Option<CargoLib>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CargoLib {
    #[serde(default)]
    name: Option<String>,
}

/// Minimal view of a `mod.toml` manifest.
#[derive(Debug, Deserialize)]
struct ModManifestLite {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    side: Option<String>,
}

/// A mod project rooted at a directory containing both `Cargo.toml` and `mod.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModProject {
    pub root: PathBuf,
    pub cargo_manifest_path: PathBuf,
    pub package_name: String,
    pub lib_name: String,
    pub mod_id: String,
    pub mod_name: Option<String>,
    pub side: Option<String>,
}

impl ModProject {
    /// Returns true if the mod can run on dedicated servers (side is "both" or "server").
    pub fn targets_server(&self) -> bool {
        !self
            .side
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("client"))
    }

    /// Returns true if the mod can run on game clients (side is "both" or "client").
    pub fn targets_client(&self) -> bool {
        !self
            .side
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("server"))
    }
    /// Searches `start` and its ancestors for a mod project.
    pub fn find(start: &Path) -> Result<Self, CliError> {
        let mut dir = if start.is_file() {
            start
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        } else {
            start.to_path_buf()
        };

        loop {
            let cargo_manifest = dir.join("Cargo.toml");
            let mod_manifest = dir.join("mod.toml");
            if cargo_manifest.is_file() && mod_manifest.is_file() {
                return Self::from_manifest(&cargo_manifest);
            }

            match dir.parent() {
                Some(parent) if parent != dir => dir = parent.to_path_buf(),
                _ => return Err(CliError::ProjectNotFound(start.to_path_buf())),
            }
        }
    }

    /// Loads a project from an explicit path to its `Cargo.toml`.
    pub fn from_manifest(manifest_path: &Path) -> Result<Self, CliError> {
        let root = manifest_path
            .parent()
            .ok_or_else(|| CliError::ProjectNotFound(manifest_path.to_path_buf()))?
            .to_path_buf();

        let cargo_text = fs::read_to_string(manifest_path)?;
        let cargo: CargoManifest =
            toml::from_str(&cargo_text).map_err(|source| CliError::Toml {
                path: manifest_path.to_path_buf(),
                source,
            })?;

        let mod_manifest_path = root.join("mod.toml");
        let mod_text = fs::read_to_string(&mod_manifest_path)?;
        let manifest: ModManifestLite =
            toml::from_str(&mod_text).map_err(|source| CliError::Toml {
                path: mod_manifest_path,
                source,
            })?;

        let lib_name = cargo
            .lib
            .and_then(|lib| lib.name)
            .unwrap_or_else(|| cargo.package.name.replace('-', "_"));

        Ok(Self {
            root,
            cargo_manifest_path: manifest_path.to_path_buf(),
            package_name: cargo.package.name,
            lib_name,
            mod_id: manifest.id,
            mod_name: manifest.name,
            side: manifest.side,
        })
    }

    /// File name Cargo gives the compiled cdylib for the current platform.
    pub fn library_file_name(&self) -> String {
        format!(
            "{}{}{}",
            std::env::consts::DLL_PREFIX,
            self.lib_name,
            std::env::consts::DLL_SUFFIX
        )
    }
}

/// Resolves Cargo's target directory, honouring `CARGO_TARGET_DIR`.
///
/// A relative `CARGO_TARGET_DIR` is resolved against `base`, matching Cargo.
pub fn cargo_target_dir(base: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) => {
            let path = PathBuf::from(dir);
            if path.is_absolute() {
                path
            } else {
                base.join(path)
            }
        }
        None => base.join("target"),
    }
}
