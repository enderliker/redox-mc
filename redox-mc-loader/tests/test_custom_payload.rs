use redox_mc_adapter_26_3::HOOK_TARGETS;
use redox_mc_loader::transformer::transform_class;
use ristretto_classfile::ClassFile;
use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_custom_packet_payload_bytecode_transformation() {
    let jar_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../versions/26.3/server-26.3.jar");
    if !jar_path.exists() {
        println!("SKIPPED: server-26.3.jar not found at {:?}", jar_path);
        return;
    }

    // Extract net/minecraft/network/protocol/common/custom/CustomPacketPayload.class
    let class_entry = "net/minecraft/network/protocol/common/custom/CustomPacketPayload.class";
    let payload_bytes = match Command::new("unzip")
        .args(["-p", jar_path.to_str().unwrap(), class_entry])
        .output()
    {
        Ok(out) if out.status.success() => out.stdout,
        _ => {
            let temp_dir = std::env::temp_dir().join("redox_extract_custom_payload");
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
        !payload_bytes.is_empty(),
        "Failed to extract CustomPacketPayload.class from server jar"
    );

    // Transform CustomPacketPayload using HOOK_TARGETS
    let transformed = transform_class(&payload_bytes, HOOK_TARGETS)
        .expect("Transformation of CustomPacketPayload must succeed")
        .expect(
            "CustomPacketPayload must match a hook in HOOK_TARGETS and produce transformed bytes",
        );

    assert_ne!(
        payload_bytes, transformed,
        "Transformed bytecode must differ from original"
    );

    // Parse transformed classfile
    let class_file = ClassFile::from_bytes(&transformed)
        .expect("Transformed CustomPacketPayload classfile must be valid");

    assert_eq!(
        class_file.class_name().unwrap(),
        "net/minecraft/network/protocol/common/custom/CustomPacketPayload"
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

        if name == "codec$original"
            && desc == "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload$FallbackProvider;Ljava/util/List;)Lnet/minecraft/network/codec/StreamCodec;"
        {
            found_original = true;
        }

        if name == "codec"
            && desc == "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload$FallbackProvider;Ljava/util/List;)Lnet/minecraft/network/codec/StreamCodec;"
        {
            found_wrapper = true;
        }
    }

    assert!(
        found_original,
        "Must have renamed original method to codec$original"
    );
    assert!(found_wrapper, "Must have synthesized wrapper method codec");
}

#[test]
fn test_mod_network_payload_rust_dispatch() {
    use redox_mc_loader::network::{dispatch_mod_payload, register_channel_handler};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let received = Arc::new(AtomicBool::new(false));
    let received_clone = Arc::clone(&received);

    register_channel_handler("testmod:data", move |data: &[u8]| {
        if data == b"hello mod payload" {
            received_clone.store(true, Ordering::SeqCst);
        }
    });

    // Dispatch payload for testmod:data
    dispatch_mod_payload("testmod", "data", b"hello mod payload");

    assert!(
        received.load(Ordering::SeqCst),
        "Registered channel handler must receive the exact binary payload as safe &[u8]"
    );
}
