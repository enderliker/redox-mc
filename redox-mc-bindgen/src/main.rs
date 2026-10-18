use clap::Parser;
use redox_mc_bindgen::{emit_class, read_class_from_jar};
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "redox-mc-bindgen",
    about = "Generate typed Rust JNI bindings directly from Minecraft JAR classfiles"
)]
pub struct Cli {
    /// Path to the Minecraft JAR file (e.g. versions/26.3/server-26.3.jar)
    #[arg(short, long)]
    pub jar: PathBuf,

    /// Class name in slash or dot format (e.g. net/minecraft/resources/Identifier)
    #[arg(short, long)]
    pub class: String,

    /// Optional output file path (defaults to printing to stdout)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let class_model = read_class_from_jar(&cli.jar, &cli.class)?;
    let generated_code = emit_class(&class_model);

    if let Some(out_path) = cli.out {
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out_path, &generated_code)?;
        println!(
            "[redox-mc-bindgen] Successfully generated bindings for '{}' -> '{}'",
            class_model.name,
            out_path.display()
        );
    } else {
        print!("{}", generated_code);
    }

    Ok(())
}
