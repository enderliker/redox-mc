//! Project scaffolding for `cargo redox-mc new`.
//!
//! The generated layout mirrors `examples/test-mod` so that any Redox-MC mod
//! works the same way whether it was hand-written or scaffolded.

use crate::error::CliError;
use std::fs;
use std::path::Path;

/// Options resolved by the `new` command before writing files.
#[derive(Debug, Clone)]
pub struct ScaffoldOptions {
    /// Validated mod id, also used as crate and library name.
    pub id: String,
    /// Human-readable name shown in `mod.toml`.
    pub display_name: String,
    /// One of `both`, `server`, `client`.
    pub side: &'static str,
    /// Author entry written to `mod.toml`.
    pub author: String,
    /// Absolute path to the `redox-mc-api` crate in the checkout.
    pub api_path: std::path::PathBuf,
}

/// Turns a free-form name into a valid mod id and Rust identifier.
///
/// Letters and digits are lowercased and kept; spaces, `-` and `_` become a
/// single underscore. Anything else (for example non-ASCII characters) is
/// rejected instead of silently mangled.
pub fn derive_mod_id(name: &str) -> Result<String, CliError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CliError::InvalidName {
            name: name.to_string(),
            reason: "the name must not be empty".to_string(),
        });
    }

    for character in trimmed.chars() {
        let allowed = character.is_ascii_alphanumeric()
            || character == '-'
            || character == '_'
            || character == ' ';
        if !allowed {
            return Err(CliError::InvalidName {
                name: name.to_string(),
                reason: "only ASCII letters, digits, spaces, '-' and '_' are allowed".to_string(),
            });
        }
    }

    let mut id: String = trimmed
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect();
    while id.contains("__") {
        id = id.replace("__", "_");
    }
    let id = id.trim_matches('_').to_string();

    if id.is_empty() {
        return Err(CliError::InvalidName {
            name: name.to_string(),
            reason: "the name must contain at least one ASCII letter or digit".to_string(),
        });
    }
    if id.len() > 64 {
        return Err(CliError::InvalidName {
            name: name.to_string(),
            reason: "the derived id exceeds the 64 character manifest limit".to_string(),
        });
    }
    if id.starts_with(|character: char| character.is_ascii_digit()) {
        return Err(CliError::InvalidName {
            name: name.to_string(),
            reason: "the derived id must not start with a digit".to_string(),
        });
    }

    Ok(id)
}

/// Writes the standard mod layout into `project_dir`.
///
/// Refuses to touch a directory that already contains files so an existing
/// project can never be silently overwritten.
pub fn scaffold(project_dir: &Path, options: &ScaffoldOptions) -> Result<(), CliError> {
    if project_dir.exists() {
        let mut entries = fs::read_dir(project_dir)?;
        if entries.next().is_some() {
            return Err(CliError::DestinationExists(project_dir.to_path_buf()));
        }
    }

    fs::create_dir_all(project_dir.join("src"))?;
    fs::write(project_dir.join("Cargo.toml"), render_cargo_toml(options))?;
    fs::write(project_dir.join("mod.toml"), render_mod_toml(options))?;
    fs::write(project_dir.join("src/lib.rs"), render_lib_rs(options))?;
    fs::write(project_dir.join(".gitignore"), GITIGNORE)?;
    fs::write(project_dir.join("README.md"), render_readme(options))?;

    Ok(())
}

const GITIGNORE: &str = "/target\n/.redox-mc\n";

fn render_cargo_toml(options: &ScaffoldOptions) -> String {
    let api_path = options.api_path.to_string_lossy().replace('\\', "/");
    format!(
        r#"[package]
name = "{id}"
version = "0.1.0"
edition = "2021"
description = "{display_name}, a native Redox-MC mod"

[lib]
name = "{id}"
crate-type = ["cdylib"]

[dependencies]
redox-mc-api = {{ path = "{api_path}" }}
"#,
        id = options.id,
        display_name = options.display_name,
        api_path = api_path,
    )
}

fn render_mod_toml(options: &ScaffoldOptions) -> String {
    let escaped_author = options
        .author
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "")
        .replace('\n', " ");
    format!(
        r#"id = "{id}"
name = "{display_name}"
version = "0.1.0"
side = "{side}"
min_loader_version = ">=0.1.0"
description = "{display_name}, a native Redox-MC mod"
authors = ["{author}"]
license = "MIT"
"#,
        id = options.id,
        display_name = options.display_name,
        side = options.side,
        author = escaped_author,
    )
}

fn render_lib_rs(options: &ScaffoldOptions) -> String {
    let id = &options.id;
    format!(
        r#"//! {display_name}
//!
//! Generated by `cargo redox-mc new`.

use redox_mc_api::redox_export;
use redox_mc_api::types::{{FfiResult, FfiStatus, FfiStr}};

/// Runs once when the loader loads this mod.
#[redox_export]
pub fn redox_mod_init() -> i32 {{
    println!("[{id}] initialized");
    0
}}

/// Runs when Minecraft bootstraps its vanilla registries.
#[redox_export]
pub fn redox_mod_on_bootstrap() -> FfiStatus {{
    println!("[{id}] bootstrap hook");
    FfiStatus::Success
}}

/// Runs for every player chat message. Return `false` to cancel delivery.
#[redox_export]
pub fn redox_mod_on_server_chat(_player: FfiStr, message: FfiStr) -> FfiResult<bool> {{
    let message = unsafe {{ message.as_str().unwrap_or("") }};
    println!("[{id}] chat: {{message}}");
    FfiResult::ok(true)
}}
"#,
        display_name = options.display_name,
        id = id,
    )
}

fn render_readme(options: &ScaffoldOptions) -> String {
    format!(
        r#"# {display_name}

A native Redox-MC mod scaffolded with `cargo redox-mc new`.

## Development loop

```bash
# Compile and stage the cdylib next to its mod.toml
cargo redox-mc build

# Launch a headless test server with the loader agent injected
cargo redox-mc run --server
```

While the server is running, rebuild the mod and reload it in place from the
server console:

```
/redox reload
```

Environment overrides:

- `REDOX_MC_HOME`: Redox-MC checkout root (auto-detected from parent directories).
- `REDOX_MC_AGENT_LIB`: explicit path to `libredox_mc_loader`.
- `REDOX_MC_SERVER_JAR`: explicit path to the Minecraft server bundler jar.
- `REDOX_MC_CLIENT_JAR`: explicit path to the Minecraft client bundler jar.
- `REDOX_MC_JAVA`: Java binary to launch (defaults to `java`).
"#,
        display_name = options.display_name,
    )
}
