use rustmc_adapter_26_3::HOOK_TARGETS;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_minecraft_server_jar() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("MINECRAFT_SERVER_JAR") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(env_path) = std::env::var("RUSTMC_SERVER_JAR") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
    }

    let candidates = [
        "../../versions/26.3/server-26.3.jar",
        "../versions/26.3/server-26.3.jar",
        "versions/26.3/server-26.3.jar",
        "/home/ender/rustmc/versions/26.3/server-26.3.jar",
    ];

    for candidate in &candidates {
        let p = Path::new(candidate);
        if p.exists() {
            return Some(p.canonicalize().unwrap_or_else(|_| PathBuf::from(candidate)));
        }
    }

    None
}

#[test]
fn test_hook_table_matches_bytecode() {
    let Some(jar_path) = find_minecraft_server_jar() else {
        println!("SKIPPED: Minecraft server jar not found. Set MINECRAFT_SERVER_JAR to run bytecode verification.");
        return;
    };

    println!("Testing hook table against real Minecraft jar: {:?}", jar_path);

    for hook in HOOK_TARGETS {
        let binary_name = hook.class_name.replace('/', ".");
        let output = Command::new("javap")
            .args(["-s", "-p", "-cp", jar_path.to_str().unwrap(), &binary_name])
            .output()
            .unwrap_or_else(|e| panic!("Failed to run javap: {}", e));

        assert!(
            output.status.success(),
            "javap failed for class '{}' in {:?}: {}",
            binary_name,
            jar_path,
            String::from_utf8_lossy(&output.stderr)
        );

        let javap_out = String::from_utf8_lossy(&output.stdout);

        // Verify that the method name exists
        assert!(
            javap_out.contains(hook.method_name),
            "Method '{}' not found in class '{}'!\njavap output:\n{}",
            hook.method_name,
            binary_name,
            javap_out
        );

        // Verify that the descriptor exists
        let expected_desc_line = format!("descriptor: {}", hook.method_desc);
        assert!(
            javap_out.contains(&expected_desc_line),
            "Expected descriptor '{}' for method '{}' in '{}' not found!\njavap output:\n{}",
            expected_desc_line,
            hook.method_name,
            binary_name,
            javap_out
        );

        println!(
            "  [OK] Verified {}::{} with descriptor {}",
            binary_name, hook.method_name, hook.method_desc
        );
    }

    println!("All {} hook targets verified successfully against Minecraft jar bytecode!", HOOK_TARGETS.len());
}
