use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../rustmc-bridge/bridge-core");
    println!("cargo:rerun-if-changed=../rustmc-bridge/bridge-game");
    println!("cargo:rerun-if-changed=../rustmc-bridge/bridge-client");
    println!("cargo:rerun-if-changed=../rustmc-bridge/build.sh");

    let script = Path::new("../rustmc-bridge/build.sh");
    if script.exists() {
        let status = Command::new("bash")
            .arg(script)
            .status()
            .expect("Failed to execute bridge build.sh");
        if !status.success() {
            panic!("rustmc-bridge build.sh failed with status: {:?}", status);
        }
    }
}
