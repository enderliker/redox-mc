//! Bytecode and hook-table tests for Task 6.3 network channels.
//!
//! These tests verify that the version adapter exposes server and client inbound
//! payload hooks and that the wrapper transformer can rewrite the real Minecraft
//! classes without producing invalid bytecode.

use redox_mc_adapter_26_3::{CLIENT_HOOK_TARGETS, HOOK_TARGETS};
use redox_mc_loader::all_hook_targets;
use redox_mc_loader::transformer::transform_class;
use ristretto_classfile::ClassFile;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("loader crate has a workspace parent")
        .to_path_buf()
}

fn find_jar(env_vars: &[&str], candidates: &[PathBuf]) -> Option<PathBuf> {
    for env_var in env_vars {
        if let Ok(value) = std::env::var(env_var) {
            let path = PathBuf::from(value);
            if path.exists() {
                return Some(path);
            }
        }
    }

    candidates.iter().find(|path| path.exists()).cloned()
}

fn server_jar() -> Option<PathBuf> {
    let root = project_root();
    find_jar(
        &["MINECRAFT_SERVER_JAR", "REDOX_MC_SERVER_JAR"],
        &[
            root.join("versions/26.3/server-26.3.jar"),
            PathBuf::from("/home/ender/redox-mc/versions/26.3/server-26.3.jar"),
            PathBuf::from("/home/ender/rustmc/versions/26.3/server-26.3.jar"),
        ],
    )
}

fn client_jar() -> Option<PathBuf> {
    let root = project_root();
    find_jar(
        &["MINECRAFT_CLIENT_JAR", "REDOX_MC_CLIENT_JAR"],
        &[
            root.join("versions/26.3/client.jar"),
            PathBuf::from("/home/ender/redox-mc/versions/26.3/client.jar"),
            PathBuf::from("/home/ender/rustmc/versions/26.3/client.jar"),
            PathBuf::from("/home/ender/client.jar"),
        ],
    )
}

fn extract_class(jar_path: &Path, entry: &str) -> Option<Vec<u8>> {
    if let Ok(output) = Command::new("unzip")
        .args(["-p", jar_path.to_str()?, entry])
        .output()
    {
        if output.status.success() && !output.stdout.is_empty() {
            return Some(output.stdout);
        }
    }

    let temp_dir =
        std::env::temp_dir().join(format!("redox_extract_{}", entry.replace(['/', '.'], "_")));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).ok()?;
    let status = Command::new("jar")
        .args(["xf", jar_path.to_str()?, entry])
        .current_dir(&temp_dir)
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    fs::read(temp_dir.join(entry)).ok()
}

fn assert_wrapper_methods(class_bytes: &[u8], class_name: &str, methods: &[(&str, &str)]) {
    let class_file =
        ClassFile::from_bytes(class_bytes).expect("transformed classfile must remain valid");
    assert_eq!(class_file.class_name().unwrap(), class_name);

    for (method_name, descriptor) in methods {
        let original = format!("{method_name}$original");
        let mut found_original = false;
        let mut found_wrapper = false;

        for method in &class_file.methods {
            let name = class_file
                .constant_pool
                .try_get_utf8(method.name_index)
                .unwrap()
                .to_string();
            let desc = class_file
                .constant_pool
                .try_get_utf8(method.descriptor_index)
                .unwrap()
                .to_string();
            if desc != *descriptor {
                continue;
            }
            if name == original {
                found_original = true;
            } else if name == *method_name {
                found_wrapper = true;
            }
        }

        assert!(
            found_original,
            "{class_name} must rename {method_name} to {original}"
        );
        assert!(
            found_wrapper,
            "{class_name} must synthesize the {method_name} wrapper"
        );
    }
}

#[test]
fn combined_hook_table_contains_server_and_client_network_hooks() {
    let combined = all_hook_targets();

    for hook in HOOK_TARGETS
        .iter()
        .chain(CLIENT_HOOK_TARGETS.iter())
        .filter(|hook| {
            hook.bridge_class == "net/redoxmc/bridge/network/ModNetworkBridge"
                && hook.bridge_method == "onServerCustomPayload"
        })
    {
        assert!(
            combined.iter().any(|candidate| candidate == hook),
            "combined hook table is missing server hook {}::{}",
            hook.class_name,
            hook.method_name
        );
    }

    for expected in [
        (
            "net/minecraft/server/network/ServerGamePacketListenerImpl",
            "handleCustomPayload",
        ),
        (
            "net/minecraft/server/network/ServerCommonPacketListenerImpl",
            "handleCustomPayload",
        ),
        (
            "net/minecraft/client/multiplayer/ClientPacketListener",
            "handleCustomPayload",
        ),
        (
            "net/minecraft/client/multiplayer/ClientConfigurationPacketListenerImpl",
            "handleCustomPayload",
        ),
        ("net/minecraft/client/Minecraft", "tick"),
    ] {
        assert!(
            combined
                .iter()
                .any(|hook| (hook.class_name, hook.method_name) == expected),
            "combined hook table is missing {}::{}",
            expected.0,
            expected.1
        );
    }
}

#[test]
fn server_payload_hooks_transform_real_bytecode() {
    let Some(jar_path) = server_jar() else {
        println!("SKIPPED: Minecraft server jar not found.");
        return;
    };

    let cases = [
        (
            "net/minecraft/server/network/ServerGamePacketListenerImpl.class",
            "net/minecraft/server/network/ServerGamePacketListenerImpl",
            vec![
                (
                    "handleChat",
                    "(Lnet/minecraft/network/protocol/game/ServerboundChatPacket;)V",
                ),
                (
                    "handleCustomPayload",
                    "(Lnet/minecraft/network/protocol/common/ServerboundCustomPayloadPacket;)V",
                ),
            ],
        ),
        (
            "net/minecraft/server/network/ServerCommonPacketListenerImpl.class",
            "net/minecraft/server/network/ServerCommonPacketListenerImpl",
            vec![(
                "handleCustomPayload",
                "(Lnet/minecraft/network/protocol/common/ServerboundCustomPayloadPacket;)V",
            )],
        ),
    ];

    for (entry, class_name, methods) in cases {
        let original = extract_class(&jar_path, entry)
            .unwrap_or_else(|| panic!("failed to extract {entry} from {jar_path:?}"));
        let transformed = transform_class(&original, HOOK_TARGETS)
            .unwrap_or_else(|err| panic!("transformation of {class_name} failed: {err}"))
            .unwrap_or_else(|| panic!("{class_name} must match a server hook"));

        assert_ne!(original, transformed, "{class_name} bytecode must change");
        assert_wrapper_methods(&transformed, class_name, &methods);
        println!(
            "  [OK] Transformed {class_name} ({} bytes)",
            transformed.len()
        );
    }
}

#[test]
fn client_payload_hooks_transform_real_bytecode() {
    let Some(jar_path) = client_jar() else {
        println!("SKIPPED: Minecraft client jar not found.");
        return;
    };

    let cases = [
        (
            "net/minecraft/client/multiplayer/ClientPacketListener.class",
            "net/minecraft/client/multiplayer/ClientPacketListener",
            "handleCustomPayload",
            "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;)V",
        ),
        (
            "net/minecraft/client/multiplayer/ClientConfigurationPacketListenerImpl.class",
            "net/minecraft/client/multiplayer/ClientConfigurationPacketListenerImpl",
            "handleCustomPayload",
            "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;)V",
        ),
        (
            "net/minecraft/client/Minecraft.class",
            "net/minecraft/client/Minecraft",
            "tick",
            "()V",
        ),
    ];

    for (entry, class_name, method_name, descriptor) in cases {
        let original = extract_class(&jar_path, entry)
            .unwrap_or_else(|| panic!("failed to extract {entry} from {jar_path:?}"));
        let transformed = transform_class(&original, CLIENT_HOOK_TARGETS)
            .unwrap_or_else(|err| panic!("transformation of {class_name} failed: {err}"))
            .unwrap_or_else(|| panic!("{class_name} must match a client hook"));

        assert_ne!(original, transformed, "{class_name} bytecode must change");
        assert_wrapper_methods(&transformed, class_name, &[(method_name, descriptor)]);
        println!(
            "  [OK] Transformed {class_name} ({} bytes)",
            transformed.len()
        );
    }
}
