use redox_mc_loader::registry::block_entity::register_block_entity_in_game;
use redox_mc_loader::registry::hash::{
    compute_registry_content_hash_full, RegisteredBlockEntityEntry, RegisteredBlockEntry,
    RegistryHashBuilder,
};

#[test]
fn test_block_entity_affects_registry_hash() {
    let mut b1 = RegistryHashBuilder::new();
    b1.add_item("testmod:ruby", 64);
    let hash1 = b1.compute_hex();

    let mut b2 = RegistryHashBuilder::new();
    b2.add_item("testmod:ruby", 64);
    b2.add_block_entity("testmod:counter_be", &["testmod:counter_block"], 3);
    let hash2 = b2.compute_hex();

    assert_ne!(hash1, hash2);
}

#[test]
fn test_block_entity_deterministic_sorting() {
    let mut b1 = RegistryHashBuilder::new();
    b1.add_block_entity("testmod:alpha_be", &["testmod:block_a"], 1);
    b1.add_block_entity("testmod:beta_be", &["testmod:block_b"], 2);
    let hash1 = b1.compute_hex();

    let mut b2 = RegistryHashBuilder::new();
    b2.add_block_entity("testmod:beta_be", &["testmod:block_b"], 2);
    b2.add_block_entity("testmod:alpha_be", &["testmod:block_a"], 1);
    let hash2 = b2.compute_hex();

    assert_eq!(
        hash1, hash2,
        "Block entities must hash deterministically regardless of insertion order"
    );
}

#[test]
fn test_block_entity_valid_blocks_sorting() {
    let mut b1 = RegistryHashBuilder::new();
    b1.add_block_entity(
        "testmod:counter_be",
        &["testmod:block_z", "testmod:block_a"],
        1,
    );
    let hash1 = b1.compute_hex();

    let mut b2 = RegistryHashBuilder::new();
    b2.add_block_entity(
        "testmod:counter_be",
        &["testmod:block_a", "testmod:block_z"],
        1,
    );
    let hash2 = b2.compute_hex();

    assert_eq!(
        hash1, hash2,
        "Valid blocks list must hash deterministically regardless of slice ordering"
    );
}

#[test]
fn test_block_entity_flag_variation() {
    let mut b1 = RegistryHashBuilder::new();
    b1.add_block_entity("testmod:counter_be", &["testmod:block_a"], 1);
    let hash1 = b1.compute_hex();

    let mut b2 = RegistryHashBuilder::new();
    b2.add_block_entity("testmod:counter_be", &["testmod:block_a"], 2);
    let hash2 = b2.compute_hex();

    assert_ne!(hash1, hash2, "Different flags must produce different hash");
}

#[test]
fn test_attached_block_entity_affects_block_hash() {
    let b_plain = RegisteredBlockEntry::new("testmod:ruby_block");
    let mut b_with_be = RegisteredBlockEntry::new("testmod:ruby_block");
    b_with_be.block_entity = Some(RegisteredBlockEntityEntry {
        id: "testmod:ruby_be".into(),
        valid_blocks: vec!["testmod:ruby_block".into()],
        behavior_flags: 3,
    });

    let mut h1 = RegistryHashBuilder::new();
    h1.add_block(b_plain);
    let hash1 = h1.compute_hex();

    let mut h2 = RegistryHashBuilder::new();
    h2.add_block(b_with_be);
    let hash2 = h2.compute_hex();

    assert_ne!(hash1, hash2, "Attached block entity must alter block hash");
}

#[test]
fn test_empty_registry_content_hash() {
    let empty_hash = compute_registry_content_hash_full(&[], &[], &[]);
    assert_eq!(empty_hash, None);
}

#[test]
fn test_register_block_entity_null_env() {
    let entry = RegisteredBlockEntityEntry {
        id: "testmod:demo_be".into(),
        valid_blocks: vec!["testmod:demo_block".into()],
        behavior_flags: 1,
    };
    let res = unsafe { register_block_entity_in_game(std::ptr::null_mut(), &entry) };
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "JNIEnv pointer is null");
}
