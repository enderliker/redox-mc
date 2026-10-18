use semver::{Version, VersionReq};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("Failed to parse TOML manifest: {0}")]
    TomlParse(String),
    #[error("Invalid mod ID '{0}': must be non-empty, 1-64 chars, containing only lowercase letters, digits, '_' or '-'")]
    InvalidModId(String),
    #[error("Invalid semver version: {0}")]
    InvalidVersion(#[from] semver::Error),
    #[error("Invalid dependency specification for '{0}': {1}")]
    InvalidDependency(String, String),
}

/// Target side / environment where the mod is intended to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModSide {
    #[default]
    Both,
    Client,
    Server,
}

impl FromStr for ModSide {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "both" => Ok(ModSide::Both),
            "client" => Ok(ModSide::Client),
            "server" => Ok(ModSide::Server),
            other => Err(format!("Unknown mod side: '{}'", other)),
        }
    }
}

/// Detailed dependency requirement specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DependencySpec {
    pub version: VersionReq,
    pub optional: bool,
}

impl fmt::Display for DependencySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            self.version,
            if self.optional { " (optional)" } else { "" }
        )
    }
}

// Custom deserializer to allow either a string `"^1.0"` or a table `{ version = "^1.0", optional = true }`
impl<'de> Deserialize<'de> for DependencySpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper {
            Simple(String),
            Detailed {
                version: String,
                #[serde(default)]
                optional: bool,
            },
        }

        match Helper::deserialize(deserializer)? {
            Helper::Simple(v_str) => {
                let version = VersionReq::parse(&v_str).map_err(serde::de::Error::custom)?;
                Ok(DependencySpec {
                    version,
                    optional: false,
                })
            }
            Helper::Detailed { version, optional } => {
                let req = VersionReq::parse(&version).map_err(serde::de::Error::custom)?;
                Ok(DependencySpec {
                    version: req,
                    optional,
                })
            }
        }
    }
}

/// Mod metadata manifest (`mod.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModManifest {
    /// Unique mod ID (lowercase alphanumeric, `_`, `-`)
    pub id: String,

    /// Human-readable mod display name
    #[serde(default)]
    pub name: Option<String>,

    /// SemVer version of the mod
    pub version: Version,

    /// Target side (client, server, both)
    #[serde(default)]
    pub side: ModSide,

    /// Minimum Redox-MC loader version required
    #[serde(default)]
    pub min_loader_version: Option<VersionReq>,

    /// Mod description
    #[serde(default)]
    pub description: Option<String>,

    /// Mod authors
    #[serde(default)]
    pub authors: Vec<String>,

    /// Mod license identifier (e.g. "MIT", "Apache-2.0")
    #[serde(default)]
    pub license: Option<String>,

    /// Dependencies mapped by mod ID
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySpec>,
}

impl ModManifest {
    /// Parses a `mod.toml` string and validates all fields.
    pub fn from_toml_str(toml_str: &str) -> Result<Self, ManifestError> {
        let manifest: ModManifest =
            toml::from_str(toml_str).map_err(|e| ManifestError::TomlParse(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validates the mod manifest fields.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.id.is_empty() || self.id.len() > 64 {
            return Err(ManifestError::InvalidModId(self.id.clone()));
        }

        let is_valid_char =
            |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-';
        if !self.id.chars().all(is_valid_char) {
            return Err(ManifestError::InvalidModId(self.id.clone()));
        }

        Ok(())
    }

    /// Returns display name, or `id` if not set.
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }

    /// Checks if this mod is compatible with the given environment side.
    pub fn is_compatible_with(&self, running_side: ModSide) -> bool {
        matches!(
            (self.side, running_side),
            (ModSide::Both, _)
                | (_, ModSide::Both)
                | (ModSide::Client, ModSide::Client)
                | (ModSide::Server, ModSide::Server)
        )
    }
}
