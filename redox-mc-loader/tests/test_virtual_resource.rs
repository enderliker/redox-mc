use redox_mc_adapter_26_3::HOOK_TARGETS;
use redox_mc_loader::registry::resource::{
    clear_virtual_resources, get_virtual_namespaces, get_virtual_resource,
    get_virtual_root_resource, list_virtual_resources, register_virtual_resource,
    register_virtual_root_resource,
};
use redox_mc_loader::transformer::transform_class;
use std::fs;
use std::process::Command;

#[test]
fn test_virtual_resource_registry_and_metadata() {
    clear_virtual_resources();

    // Test default pack.mcmeta format
    let default_meta = get_virtual_root_resource("pack.mcmeta").expect("default pack.mcmeta");
    let meta_str = std::str::from_utf8(&default_meta).expect("utf8 pack.mcmeta");
    assert!(meta_str.contains("Redox Virtual Mod Resources"));
    assert!(meta_str.contains("\"min_format\":97"));
    assert!(meta_str.contains("\"max_format\":121"));

    // Register custom root resource
    register_virtual_root_resource("pack.png", vec![1, 2, 3, 4]);
    assert_eq!(
        get_virtual_root_resource("pack.png"),
        Some(vec![1, 2, 3, 4])
    );

    // Register data pack resources
    register_virtual_resource(
        "data",
        "testmod",
        "recipe/ruby_from_crafting.json",
        b"{\"type\":\"minecraft:crafting_shapeless\"}".to_vec(),
    );
    register_virtual_resource(
        "data",
        "testmod",
        "loot_table/blocks/ruby_block.json",
        b"{\"type\":\"minecraft:block\"}".to_vec(),
    );
    register_virtual_resource(
        "assets",
        "testmod",
        "models/item/ruby.json",
        b"{\"parent\":\"item/generated\"}".to_vec(),
    );

    // Verify get_virtual_resource
    assert_eq!(
        get_virtual_resource("data", "testmod", "recipe/ruby_from_crafting.json"),
        Some(b"{\"type\":\"minecraft:crafting_shapeless\"}".to_vec())
    );
    assert_eq!(
        get_virtual_resource("data", "testmod", "loot_table/blocks/ruby_block.json"),
        Some(b"{\"type\":\"minecraft:block\"}".to_vec())
    );
    assert_eq!(
        get_virtual_resource("assets", "testmod", "models/item/ruby.json"),
        Some(b"{\"parent\":\"item/generated\"}".to_vec())
    );
    assert_eq!(
        get_virtual_resource("data", "testmod", "nonexistent.json"),
        None
    );

    // Verify list_virtual_resources
    let recipe_list = list_virtual_resources("data", "testmod", "recipe");
    assert_eq!(recipe_list, vec!["recipe/ruby_from_crafting.json"]);

    let all_data_list = list_virtual_resources("data", "testmod", "");
    assert_eq!(
        all_data_list,
        vec![
            "loot_table/blocks/ruby_block.json",
            "recipe/ruby_from_crafting.json"
        ]
    );

    // Verify get_virtual_namespaces
    let data_namespaces = get_virtual_namespaces("data");
    assert_eq!(data_namespaces, vec!["testmod"]);

    let asset_namespaces = get_virtual_namespaces("assets");
    assert_eq!(asset_namespaces, vec!["testmod"]);

    clear_virtual_resources();
}

#[test]
fn test_transform_pack_repository_class() {
    let jar_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../versions/26.3/server-26.3.jar");
    if !jar_path.exists() {
        println!("SKIPPED: server-26.3.jar not found at {:?}", jar_path);
        return;
    }

    let class_name = "net/minecraft/server/packs/repository/PackRepository.class";
    let pack_repo_bytes = match Command::new("unzip")
        .args(["-p", jar_path.to_str().unwrap(), class_name])
        .output()
    {
        Ok(out) if out.status.success() => out.stdout,
        _ => {
            let temp_dir = std::env::temp_dir().join("redox_extract_pack_repo");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).unwrap();
            let _ = Command::new("jar")
                .args(["xf", jar_path.to_str().unwrap(), class_name])
                .current_dir(&temp_dir)
                .status();
            fs::read(temp_dir.join(class_name)).expect("extracted PackRepository.class")
        }
    };

    assert!(!pack_repo_bytes.is_empty());

    // Transform PackRepository class
    let transformed = transform_class(&pack_repo_bytes, HOOK_TARGETS)
        .expect("transformation of PackRepository succeeds");

    assert!(transformed.is_some(), "PackRepository was modified");
    let transformed_bytes = transformed.unwrap();

    // Verify transformed class with javap
    let temp_file = std::env::temp_dir().join("PackRepositoryTransformed.class");
    fs::write(&temp_file, &transformed_bytes).unwrap();

    let javap = Command::new("javap")
        .args(["-v", "-p", temp_file.to_str().unwrap()])
        .output()
        .expect("javap command runs");

    assert!(
        javap.status.success(),
        "javap validation must succeed: {}",
        String::from_utf8_lossy(&javap.stderr)
    );

    let javap_out = String::from_utf8_lossy(&javap.stdout);
    assert!(
        javap_out.contains("discoverAvailable$original"),
        "Transformed class must have renamed original method"
    );
    assert!(
        javap_out.contains("discoverAvailable"),
        "Transformed class must have synthetic wrapper method"
    );
    assert!(
        javap_out.contains("VirtualModPackRepositorySource"),
        "Transformed wrapper must invoke VirtualModPackRepositorySource"
    );

    let _ = fs::remove_file(temp_file);
}
