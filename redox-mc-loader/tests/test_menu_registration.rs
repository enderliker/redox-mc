use redox_mc_loader::registry::hash::{
    compute_full_registry_hash_all, compute_registry_content_hash_all, RegistryHashBuilder,
};
use redox_mc_loader::registry::menu::RegisteredMenuEntry;

#[test]
fn test_menu_entry_creation() {
    let entry = RegisteredMenuEntry::new("testmod:counter_menu", 9, "Counter Menu");
    assert_eq!(entry.id, "testmod:counter_menu");
    assert_eq!(entry.slots, 9);
    assert_eq!(entry.title, "Counter Menu");
}

#[test]
fn test_menu_hash_determinism_and_sorting() {
    let m1 = RegisteredMenuEntry::new("testmod:b_menu", 9, "B Menu");
    let m2 = RegisteredMenuEntry::new("testmod:a_menu", 18, "A Menu");

    // Regardless of insertion order, the computed hash must be identical because menus are sorted by ID
    let hash1 = compute_full_registry_hash_all(&[], &[], &[], &[m1.clone(), m2.clone()]);
    let hash2 = compute_full_registry_hash_all(&[], &[], &[], &[m2.clone(), m1.clone()]);

    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64);

    // Hash must change if slots change
    let m2_modified = RegisteredMenuEntry::new("testmod:a_menu", 27, "A Menu");
    let hash_diff = compute_full_registry_hash_all(&[], &[], &[], &[m1, m2_modified]);
    assert_ne!(hash1, hash_diff);
}

#[test]
fn test_registry_hash_builder_with_menus() {
    let mut builder1 = RegistryHashBuilder::new();
    builder1
        .add_item("testmod:ruby", 64)
        .add_menu("testmod:counter_menu", 9, "Counter");

    let mut builder2 = RegistryHashBuilder::new();
    builder2
        .add_menu("testmod:counter_menu", 9, "Counter")
        .add_item("testmod:ruby", 64);

    assert_eq!(builder1.compute_hex(), builder2.compute_hex());
}

#[test]
fn test_empty_content_hash_returns_none() {
    let hash = compute_registry_content_hash_all(&[], &[], &[], &[]);
    assert!(hash.is_none());

    let menu = RegisteredMenuEntry::new("testmod:counter_menu", 9, "Counter");
    let hash_with_menu = compute_registry_content_hash_all(&[], &[], &[], &[menu]);
    assert!(hash_with_menu.is_some());
}
