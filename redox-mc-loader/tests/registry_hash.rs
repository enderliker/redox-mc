use redox_mc_loader::registry::{
    compute_full_registry_hash, compute_registry_hash, RegisteredBlockEntry, RegisteredItemEntry,
    RegistryHasher, BLOCK_FLAG_HAS_USE_WITHOUT_ITEM, FLAG_HAS_USE, FLAG_HAS_USE_ON,
};

#[test]
fn test_registry_hash_determinism() {
    let items = vec![
        RegisteredItemEntry {
            id: "testmod:ruby".into(),
            max_stack_size: 64,
            durability: None,
            fire_resistant: true,
            behavior_flags: 0,
        },
        RegisteredItemEntry {
            id: "testmod:ruby_pickaxe".into(),
            max_stack_size: 1,
            durability: Some(1500),
            fire_resistant: false,
            behavior_flags: FLAG_HAS_USE | FLAG_HAS_USE_ON,
        },
    ];

    let hash1 = compute_registry_hash(&items);
    let hash2 = compute_registry_hash(&items);
    let hash3 = compute_registry_hash(&items);

    assert_eq!(hash1, hash2);
    assert_eq!(hash2, hash3);
    assert_eq!(hash1.len(), 64); // 64 hex characters = 256 bits
}

#[test]
fn test_registry_hash_order_sensitivity() {
    let item_a = RegisteredItemEntry {
        id: "mod_a:item".into(),
        max_stack_size: 64,
        durability: None,
        fire_resistant: false,
        behavior_flags: 0,
    };
    let item_b = RegisteredItemEntry {
        id: "mod_b:item".into(),
        max_stack_size: 16,
        durability: None,
        fire_resistant: false,
        behavior_flags: 0,
    };

    let hash_ab = compute_registry_hash(&[item_a.clone(), item_b.clone()]);
    let hash_ba = compute_registry_hash(&[item_b, item_a]);

    assert_ne!(
        hash_ab, hash_ba,
        "Hash must be sensitive to the deterministic order of mods"
    );
}

#[test]
fn test_registry_hash_property_sensitivity() {
    let base = RegisteredItemEntry {
        id: "testmod:item".into(),
        max_stack_size: 64,
        durability: None,
        fire_resistant: false,
        behavior_flags: 0,
    };

    let base_hash = compute_registry_hash(std::slice::from_ref(&base));

    let diff_id = RegisteredItemEntry {
        id: "testmod:item2".into(),
        ..base.clone()
    };
    assert_ne!(base_hash, compute_registry_hash(&[diff_id]));

    let diff_stack = RegisteredItemEntry {
        max_stack_size: 16,
        ..base.clone()
    };
    assert_ne!(base_hash, compute_registry_hash(&[diff_stack]));

    let diff_durability = RegisteredItemEntry {
        durability: Some(100),
        ..base.clone()
    };
    assert_ne!(base_hash, compute_registry_hash(&[diff_durability]));

    let diff_fire = RegisteredItemEntry {
        fire_resistant: true,
        ..base.clone()
    };
    assert_ne!(base_hash, compute_registry_hash(&[diff_fire]));

    let diff_flags = RegisteredItemEntry {
        behavior_flags: 1,
        ..base
    };
    assert_ne!(base_hash, compute_registry_hash(&[diff_flags]));
}

#[test]
fn test_incremental_hasher_matches_compute_function() {
    let items = vec![
        RegisteredItemEntry {
            id: "alpha:one".into(),
            max_stack_size: 64,
            durability: None,
            fire_resistant: false,
            behavior_flags: 0,
        },
        RegisteredItemEntry {
            id: "beta:two".into(),
            max_stack_size: 32,
            durability: Some(250),
            fire_resistant: true,
            behavior_flags: 3,
        },
    ];

    let mut hasher = RegistryHasher::new();
    for item in &items {
        hasher.update_item(item);
    }
    let incremental_hex = hasher.finalize_hex();

    let direct_hex = compute_registry_hash(&items);

    assert_eq!(incremental_hex, direct_hex);
}

#[test]
fn test_block_registry_hash_determinism_and_sensitivity() {
    let base_block = RegisteredBlockEntry {
        id: "testmod:ruby_block".into(),
        destroy_time: 3.0,
        explosion_resistance: 6.0,
        requires_tool: true,
        no_collision: false,
        behavior_flags: BLOCK_FLAG_HAS_USE_WITHOUT_ITEM,
        has_item: true,
    };

    let items = vec![RegisteredItemEntry {
        id: "testmod:ruby".into(),
        max_stack_size: 64,
        durability: None,
        fire_resistant: true,
        behavior_flags: 0,
    }];

    let hash1 = compute_full_registry_hash(&items, std::slice::from_ref(&base_block));
    let hash2 = compute_full_registry_hash(&items, std::slice::from_ref(&base_block));
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64);

    // Differentiate by hardness
    let diff_destroy = RegisteredBlockEntry {
        destroy_time: 5.0,
        ..base_block.clone()
    };
    assert_ne!(hash1, compute_full_registry_hash(&items, &[diff_destroy]));

    // Differentiate by resistance
    let diff_res = RegisteredBlockEntry {
        explosion_resistance: 12.0,
        ..base_block.clone()
    };
    assert_ne!(hash1, compute_full_registry_hash(&items, &[diff_res]));

    // Differentiate by requires_tool
    let diff_tool = RegisteredBlockEntry {
        requires_tool: false,
        ..base_block.clone()
    };
    assert_ne!(hash1, compute_full_registry_hash(&items, &[diff_tool]));

    // Differentiate by no_collision
    let diff_col = RegisteredBlockEntry {
        no_collision: true,
        ..base_block.clone()
    };
    assert_ne!(hash1, compute_full_registry_hash(&items, &[diff_col]));

    // Differentiate by behavior_flags
    let diff_flags = RegisteredBlockEntry {
        behavior_flags: 0,
        ..base_block.clone()
    };
    assert_ne!(hash1, compute_full_registry_hash(&items, &[diff_flags]));

    // Differentiate by has_item
    let diff_item = RegisteredBlockEntry {
        has_item: false,
        ..base_block
    };
    assert_ne!(hash1, compute_full_registry_hash(&items, &[diff_item]));
}
