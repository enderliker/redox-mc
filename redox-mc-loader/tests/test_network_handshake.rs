use redox_mc_adapter_26_3::HOOK_TARGETS;
use redox_mc_loader::network::handshake::{validate_client_handshake, HandshakeModInfo};
use redox_mc_loader::transformer::transform_class;
use ristretto_classfile::ClassFile;
use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_handshake_validation_success() {
    let server_hash = "abc123hash";
    let server_mods = vec![HandshakeModInfo {
        id: "testmod".to_string(),
        version: "0.1.0".to_string(),
    }];

    let client_hash = "abc123hash";
    let client_mods = vec![HandshakeModInfo {
        id: "testmod".to_string(),
        version: "0.1.0".to_string(),
    }];

    let result = validate_client_handshake(server_hash, &server_mods, client_hash, &client_mods);
    assert!(result.is_ok(), "Matching hash and mods must pass handshake");
}

#[test]
fn test_handshake_validation_hash_mismatch() {
    let server_hash = "server_hash_111";
    let server_mods = vec![HandshakeModInfo {
        id: "testmod".to_string(),
        version: "0.1.0".to_string(),
    }];

    let client_hash = "client_hash_222";
    let client_mods = vec![HandshakeModInfo {
        id: "testmod".to_string(),
        version: "0.1.0".to_string(),
    }];

    let result = validate_client_handshake(server_hash, &server_mods, client_hash, &client_mods);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("Mismatched registry content hash"),
        "Error message must specify registry hash mismatch, got: {}",
        err
    );
    assert!(err.contains("server_hash_111"));
    assert!(err.contains("client_hash_222"));
}

#[test]
fn test_handshake_validation_missing_mod() {
    let server_hash = "hash1";
    let server_mods = vec![
        HandshakeModInfo {
            id: "testmod".to_string(),
            version: "0.1.0".to_string(),
        },
        HandshakeModInfo {
            id: "extra_mod".to_string(),
            version: "1.0.0".to_string(),
        },
    ];

    let client_hash = "hash1";
    let client_mods = vec![HandshakeModInfo {
        id: "testmod".to_string(),
        version: "0.1.0".to_string(),
    }];

    let result = validate_client_handshake(server_hash, &server_mods, client_hash, &client_mods);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("Missing required mod: extra_mod"),
        "Error message must specify missing mod, got: {}",
        err
    );
}

#[test]
fn test_handshake_validation_version_mismatch() {
    let server_hash = "hash1";
    let server_mods = vec![HandshakeModInfo {
        id: "testmod".to_string(),
        version: "0.1.0".to_string(),
    }];

    let client_hash = "hash1";
    let client_mods = vec![HandshakeModInfo {
        id: "testmod".to_string(),
        version: "0.2.0".to_string(),
    }];

    let result = validate_client_handshake(server_hash, &server_mods, client_hash, &client_mods);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("Incompatible version for mod 'testmod'"),
        "Error message must specify version mismatch, got: {}",
        err
    );
    assert!(err.contains("0.1.0"));
    assert!(err.contains("0.2.0"));
}

#[test]
fn test_server_configuration_packet_listener_impl_wrapper_transformation() {
    let jar_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../versions/26.3/server-26.3.jar");
    if !jar_path.exists() {
        println!("SKIPPED: server-26.3.jar not found at {:?}", jar_path);
        return;
    }

    let class_entry = "net/minecraft/server/network/ServerConfigurationPacketListenerImpl.class";
    let listener_bytes = match Command::new("unzip")
        .args(["-p", jar_path.to_str().unwrap(), class_entry])
        .output()
    {
        Ok(out) if out.status.success() => out.stdout,
        _ => {
            let temp_dir = std::env::temp_dir().join("redox_extract_config_listener");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();
            let jar_status = Command::new("jar")
                .args(["xf", jar_path.to_str().unwrap(), class_entry])
                .current_dir(&temp_dir)
                .status();
            match jar_status {
                Ok(st) if st.success() => fs::read(temp_dir.join(class_entry)).unwrap_or_default(),
                _ => Vec::new(),
            }
        }
    };

    assert!(
        !listener_bytes.is_empty(),
        "Failed to extract ServerConfigurationPacketListenerImpl.class from server jar"
    );

    // Transform ServerConfigurationPacketListenerImpl using HOOK_TARGETS
    let transformed = transform_class(&listener_bytes, HOOK_TARGETS)
        .expect("Transformation must succeed")
        .expect("ServerConfigurationPacketListenerImpl must match a hook in HOOK_TARGETS");

    assert_ne!(
        listener_bytes, transformed,
        "Transformed bytecode must differ from original"
    );

    let class_file =
        ClassFile::from_bytes(&transformed).expect("Transformed classfile must be valid");

    assert_eq!(
        class_file.class_name().unwrap(),
        "net/minecraft/server/network/ServerConfigurationPacketListenerImpl"
    );

    let mut found_original = false;
    let mut found_wrapper = false;

    for method in &class_file.methods {
        let name = class_file
            .constant_pool
            .try_get_utf8(method.name_index)
            .unwrap();
        let desc = class_file
            .constant_pool
            .try_get_utf8(method.descriptor_index)
            .unwrap();

        if name == "startConfiguration$original" && desc == "()V" {
            found_original = true;
        }

        if name == "startConfiguration" && desc == "()V" {
            found_wrapper = true;
        }
    }

    assert!(
        found_original,
        "Must have renamed original method to startConfiguration$original"
    );
    assert!(
        found_wrapper,
        "Must have synthesized wrapper method startConfiguration"
    );
}
