//! `cargo redox-mc build`: compile the mod and stage the cdylib.

use crate::cli::BuildArgs;
use crate::error::CliError;
use crate::project::{cargo_target_dir, ModProject};
use crate::repo::find_artifact;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

pub fn execute(args: BuildArgs) -> Result<(), CliError> {
    let project = match args.manifest_path {
        Some(manifest_path) => ModProject::from_manifest(&manifest_path)?,
        None => ModProject::find(&env::current_dir()?)?,
    };
    build_project(&project, args.release)?;
    Ok(())
}

/// Compiles a mod project and stages the artifact next to its `mod.toml`.
///
/// The staged copy lives in `<project>/.redox-mc/dist/<id>/`. If a test server
/// was launched before, the same copy is refreshed under
/// `<project>/.redox-mc/server/mods/<id>/` so `/redox reload` picks it up.
pub fn build_project(project: &ModProject, release: bool) -> Result<PathBuf, CliError> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut command = std::process::Command::new(&cargo);
    command.arg("build").current_dir(&project.root);
    if release {
        command.arg("--release");
    }

    let status = command.status()?;
    if !status.success() {
        return Err(CliError::message(format!(
            "cargo build failed for '{}' ({status})",
            project.package_name
        )));
    }

    let target_root = cargo_target_dir(&project.root);
    let file_name = project.library_file_name();
    let built =
        find_artifact(&target_root, &file_name).ok_or_else(|| CliError::ArtifactNotFound {
            root: target_root,
            file_name: file_name.clone(),
        })?;

    let staged = stage_into(
        &project.root.join(".redox-mc/dist").join(&project.mod_id),
        project,
        &built,
        &file_name,
    )?;

    let server_mod_dir = project
        .root
        .join(".redox-mc/server/mods")
        .join(&project.mod_id);
    if server_mod_dir.is_dir() {
        stage_into(&server_mod_dir, project, &built, &file_name)?;
    }

    println!(
        "[redox-mc] Built '{}' ({}) -> {}",
        project.mod_id,
        if release { "release" } else { "debug" },
        staged.display()
    );

    Ok(staged)
}

fn stage_into(
    directory: &std::path::Path,
    project: &ModProject,
    built: &std::path::Path,
    file_name: &str,
) -> Result<PathBuf, CliError> {
    fs::create_dir_all(directory)?;
    fs::copy(built, directory.join(file_name))?;
    fs::copy(project.root.join("mod.toml"), directory.join("mod.toml"))?;
    Ok(directory.to_path_buf())
}
