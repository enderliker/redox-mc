//! Bytecode and hook-table tests for Task 6.3 network channels.
//!
//! These tests verify that the version adapter exposes server and client inbound
//! payload hooks and that the wrapper transformer can rewrite the real Minecraft
//! classes without producing invalid bytecode.

use redox_mc_adapter_26_3::{CLIENT_HOOK_TARGETS, HOOK_TARGETS};
use redox_mc_loader::all_hook_targets;
use redox_mc_loader::transformer::transform_class;
use ristretto_classfile::attributes::{Attribute, Instruction};
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
fn client_hooks_forward_the_listener_to_the_bridge() {
    let expected_desc =
        "(Ljava/lang/Object;Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;)V";
    let mut hooks = 0;
    for hook in CLIENT_HOOK_TARGETS
        .iter()
        .filter(|hook| hook.bridge_method == "onClientCustomPayload")
    {
        hooks += 1;
        assert_eq!(
            hook.bridge_desc, expected_desc,
            "{}::{} must forward the listener as Object",
            hook.class_name, hook.method_name
        );
    }
    assert_eq!(
        hooks, 2,
        "both the play-phase and configuration-phase client hooks must be present"
    );
}

fn wrapper_code<'a>(
    class_file: &'a ClassFile<'_>,
    method_name: &str,
    descriptor: &str,
) -> &'a [Instruction] {
    let method = class_file
        .methods
        .iter()
        .find(|method| {
            let name = class_file
                .constant_pool
                .try_get_utf8(method.name_index)
                .map(|value| value.to_string())
                .unwrap_or_default();
            let desc = class_file
                .constant_pool
                .try_get_utf8(method.descriptor_index)
                .map(|value| value.to_string())
                .unwrap_or_default();
            name == method_name && desc == descriptor
        })
        .unwrap_or_else(|| panic!("wrapper {method_name}{descriptor} must exist"));

    method
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            Attribute::Code { code, .. } => Some(code.as_slice()),
            _ => None,
        })
        .expect("wrapper method must have a Code attribute")
}

#[test]
fn client_payload_wrapper_passes_this_and_payload_to_the_bridge() {
    let Some(jar_path) = client_jar() else {
        println!("SKIPPED: Minecraft client jar not found.");
        return;
    };

    let entry = "net/minecraft/client/multiplayer/ClientPacketListener.class";
    let class_name = "net/minecraft/client/multiplayer/ClientPacketListener";
    let descriptor = "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;)V";
    let original = extract_class(&jar_path, entry)
        .unwrap_or_else(|| panic!("failed to extract {entry} from {jar_path:?}"));
    let transformed = transform_class(&original, CLIENT_HOOK_TARGETS)
        .unwrap_or_else(|err| panic!("transformation of {class_name} failed: {err}"))
        .unwrap_or_else(|| panic!("{class_name} must match a client hook"));
    let class_file = ClassFile::from_bytes(&transformed).expect("valid transformed classfile");

    let code = wrapper_code(&class_file, "handleCustomPayload", descriptor);
    assert!(
        matches!(code.first(), Some(Instruction::Aload_0)),
        "wrapper must load `this` as the first argument, got {:?}",
        code.first()
    );
    assert!(
        matches!(code.get(1), Some(Instruction::Aload_1)),
        "wrapper must load the payload as the second argument, got {:?}",
        code.get(1)
    );

    let call_index = code
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::Invokestatic(index) => Some(*index),
            _ => None,
        })
        .expect("wrapper must invoke the bridge statically");

    let (class_index, name_and_type_index) = class_file
        .constant_pool
        .try_get_method_ref(call_index)
        .expect("bridge call must reference a method");
    let bridge_class = class_file
        .constant_pool
        .try_get_class(*class_index)
        .expect("bridge class reference must resolve")
        .to_string();
    assert_eq!(bridge_class, "net/redoxmc/bridge/network/ModNetworkBridge");

    let (name_index, desc_index) = class_file
        .constant_pool
        .try_get_name_and_type(*name_and_type_index)
        .expect("bridge method name and type must resolve");
    let bridge_method = class_file
        .constant_pool
        .try_get_utf8(*name_index)
        .expect("bridge method name must resolve")
        .to_string();
    let bridge_desc = class_file
        .constant_pool
        .try_get_utf8(*desc_index)
        .expect("bridge method descriptor must resolve")
        .to_string();
    assert_eq!(bridge_method, "onClientCustomPayload");
    assert_eq!(
        bridge_desc,
        "(Ljava/lang/Object;Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;)V",
        "the client wrapper must pass the listener so the bridge can answer the handshake"
    );
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
