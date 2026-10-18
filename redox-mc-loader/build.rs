use std::path::{Path, PathBuf};
use std::process::Command;

fn find_bash() -> Option<PathBuf> {
    if !cfg!(windows) {
        return Some(PathBuf::from("bash"));
    }

    // On Windows, explicitly search for Git Bash to avoid C:\Windows\System32\bash.exe (WSL shim)
    let candidates = [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files\Git\usr\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\usr\bin\bash.exe",
    ];

    for candidate in &candidates {
        let p = Path::new(candidate);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }

    // Try finding git in PATH and derive bash from it
    if let Ok(output) = Command::new("where").arg("git").output() {
        if output.status.success() {
            if let Ok(paths_str) = String::from_utf8(output.stdout) {
                for line in paths_str.lines() {
                    let git_path = Path::new(line.trim());
                    if let Some(git_dir) = git_path.parent().and_then(|p| p.parent()) {
                        let bash_bin = git_dir.join("bin").join("bash.exe");
                        if bash_bin.exists() {
                            return Some(bash_bin);
                        }
                    }
                }
            }
        }
    }

    None
}

fn main() {
    println!("cargo:rerun-if-changed=../redox-mc-bridge/bridge-core");
    println!("cargo:rerun-if-changed=../redox-mc-bridge/bridge-game");
    println!("cargo:rerun-if-changed=../redox-mc-bridge/bridge-client");
    println!("cargo:rerun-if-changed=../redox-mc-bridge/build.sh");

    let script = Path::new("../redox-mc-bridge/build.sh");
    let bridge_jar = Path::new("../redox-mc-bridge/build/bridge-core.jar");

    if script.exists() {
        if let Some(bash) = find_bash() {
            let status = Command::new(&bash).arg(script).status();

            match status {
                Ok(s) if s.success() => {
                    // Successfully compiled bridge modules
                }
                Ok(s) => {
                    if bridge_jar.exists() {
                        println!(
                            "cargo:warning=redox-mc-bridge build.sh exited with status {:?}, but bridge-core.jar exists; continuing.",
                            s
                        );
                    } else {
                        panic!("redox-mc-bridge build.sh failed with status: {:?}", s);
                    }
                }
                Err(e) => {
                    if bridge_jar.exists() {
                        println!(
                            "cargo:warning=Failed to execute bridge build.sh ({:?}), but bridge-core.jar exists; continuing.",
                            e
                        );
                    } else {
                        panic!("Failed to execute bridge build.sh: {:?}", e);
                    }
                }
            }
        } else if bridge_jar.exists() {
            println!("cargo:warning=Bash not found; using existing bridge-core.jar.");
        } else {
            panic!("Bash was not found on this system and bridge-core.jar does not exist. Please run redox-mc-bridge/build.sh manually.");
        }
    }
}
