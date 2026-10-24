//! Error type shared by every `cargo redox-mc` command.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    /// A user-facing message that does not fit any other variant.
    #[error("{0}")]
    Message(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse '{path}': {source}")]
    Toml {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error(
        "could not find the Redox-MC checkout from '{0}'; set REDOX_MC_HOME to the repository root"
    )]
    RepoNotFound(PathBuf),

    #[error("could not find a mod project from '{0}' (no mod.toml next to a Cargo.toml in any parent directory)")]
    ProjectNotFound(PathBuf),

    #[error("'{0}' already exists and is not empty")]
    DestinationExists(PathBuf),

    #[error("invalid mod name '{name}': {reason}")]
    InvalidName { name: String, reason: String },

    #[error("expected to find '{file_name}' under '{root}' but no build artifact matched")]
    ArtifactNotFound { root: PathBuf, file_name: String },

    #[error("'{path}' does not exist; {hint}")]
    MissingFile { path: PathBuf, hint: String },
}

impl CliError {
    /// Convenience constructor for [`CliError::Message`].
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}
