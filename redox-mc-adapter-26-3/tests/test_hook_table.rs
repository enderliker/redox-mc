use redox_mc_adapter_26_3::{CLIENT_HOOK_TARGETS, HOOK_TARGETS};
use redox_mc_adapter_api::HookTarget;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_minecraft_server_jar() -> Option<PathBuf> {
    find_jar(
        &["MINECRAFT_SERVER_JAR", "REDOX_MC_SERVER_JAR"],
        &[
            "../../versions/26.3/server-26.3.jar",
            "../versions/26.3/server-26.3.jar",
            "versions/26.3/server-26.3.jar",
            "/home/ender/redox-mc/versions/26.3/server-26.3.jar",
            "/home/ender/rustmc/versions/26.3/server-26.3.jar",
        ],
    )
}

fn find_minecraft_client_jar() -> Option<PathBuf> {
    find_jar(
        &["MINECRAFT_CLIENT_JAR", "REDOX_MC_CLIENT_JAR"],
        &[
            "../../versions/26.3/client.jar",
            "../versions/26.3/client.jar",
            "versions/26.3/client.jar",
            "/home/ender/redox-mc/versions/26.3/client.jar",
            "/home/ender/rustmc/versions/26.3/client.jar",
            "/home/ender/client.jar",
        ],
    )
}

fn find_jar(env_vars: &[&str], candidates: &[&str]) -> Option<PathBuf> {
    for env_var in env_vars {
        if let Ok(env_path) = std::env::var(env_var) {
            let p = PathBuf::from(env_path);
            if p.exists() {
                return Some(p);
            }
        }
    }

    for candidate in candidates {
        let p = Path::new(candidate);
        if p.exists() {
            return Some(
                p.canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(candidate)),
            );
        }
    }

    None
}

fn verify_hook_targets(jar_path: &Path, hooks: &[HookTarget]) {
    for hook in hooks {
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
}

#[test]
fn test_hook_table_matches_bytecode() {
    let Some(jar_path) = find_minecraft_server_jar() else {
        println!("SKIPPED: Minecraft server jar not found. Set MINECRAFT_SERVER_JAR to run bytecode verification.");
        return;
    };

    println!(
        "Testing hook table against real Minecraft jar: {:?}",
        jar_path
    );

    verify_hook_targets(&jar_path, HOOK_TARGETS);

    println!(
        "All {} hook targets verified successfully against Minecraft jar bytecode!",
        HOOK_TARGETS.len()
    );
}

#[test]
fn test_client_hook_table_matches_bytecode() {
    let Some(jar_path) = find_minecraft_client_jar() else {
        println!(
            "SKIPPED: Minecraft client jar not found. Set MINECRAFT_CLIENT_JAR to run client \
             bytecode verification."
        );
        return;
    };

    println!(
        "Testing client hook table against real Minecraft jar: {:?}",
        jar_path
    );

    verify_hook_targets(&jar_path, CLIENT_HOOK_TARGETS);

    println!(
        "All {} client hook targets verified successfully against Minecraft jar bytecode!",
        CLIENT_HOOK_TARGETS.len()
    );
}
