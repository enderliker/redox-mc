//! `cargo redox-mc new`: scaffold a mod project.

use crate::cli::NewArgs;
use crate::error::CliError;
use crate::repo::RedoxRepo;
use crate::templates::{derive_mod_id, scaffold, ScaffoldOptions};
use std::env;

pub fn execute(args: NewArgs) -> Result<(), CliError> {
    let current_dir = env::current_dir()?;
    let id = derive_mod_id(&args.name)?;
    let repo = RedoxRepo::discover(&current_dir)?;

    let parent = match args.dir {
        Some(dir) => dir,
        None => current_dir,
    };
    let project_dir = parent.join(&id);

    let options = ScaffoldOptions {
        id: id.clone(),
        display_name: args.name.clone(),
        side: args.side.as_str(),
        author: args
            .author
            .unwrap_or_else(|| "Redox-MC developer".to_string()),
        api_path: repo.api_path(),
    };
    scaffold(&project_dir, &options)?;

    println!("[redox-mc] Created mod '{id}' at {}", project_dir.display());
    println!(
        "[redox-mc] Next: cd \"{}\" && cargo redox-mc run --server",
        project_dir.display()
    );

    Ok(())
}
