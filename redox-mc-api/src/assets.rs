//! Declarative assets and data generation for Minecraft blocks and items.

use crate::resource::VirtualResource;

/// Helper function to split a namespaced identifier ("namespace:path") into its components.
/// If no ':' is present or if the namespace is empty, defaults to the "minecraft" namespace.
pub fn split_identifier(id: &str) -> (&str, &str) {
    if let Some((ns, path)) = id.split_once(':') {
        let ns = if ns.is_empty() { "minecraft" } else { ns };
        (ns, path)
    } else {
        ("minecraft", id)
    }
}

/// Generates a standard single-variant blockstate JSON.
///
/// Output:
/// ```json
/// {
///   "variants": {
///     "": {
///       "model": "<model_id>"
///     }
///   }
/// }
/// ```
pub fn generate_cube_blockstate(model_id: &str) -> String {
    format!(r#"{{"variants":{{"":{{"model":"{}"}}}}}}"#, model_id)
}

/// Generates a standard `minecraft:block/cube_all` block model JSON.
///
/// Output:
/// ```json
/// {
///   "parent": "minecraft:block/cube_all",
///   "textures": {
///     "all": "<texture_id>"
///   }
/// }
/// ```
pub fn generate_cube_block_model(texture_id: &str) -> String {
    format!(
        r#"{{"parent":"minecraft:block/cube_all","textures":{{"all":"{}"}}}}"#,
        texture_id
    )
}

/// Generates an item definition JSON for a block item (Minecraft 1.21.4+ / 26.3 format).
///
/// Path: `assets/<namespace>/items/<id>.json`
///
/// Output:
/// ```json
/// {
///   "model": {
///     "type": "minecraft:model",
///     "model": "<model_id>"
///   }
/// }
/// ```
pub fn generate_item_model_definition(model_id: &str) -> String {
    format!(
        r#"{{"model":{{"type":"minecraft:model","model":"{}"}}}}"#,
        model_id
    )
}

/// Generates a legacy/fallback item model JSON.
///
/// Path: `assets/<namespace>/models/item/<id>.json`
///
/// Output:
/// ```json
/// {
///   "parent": "<model_id>"
/// }
/// ```
pub fn generate_legacy_item_model(model_id: &str) -> String {
    format!(r#"{{"parent":"{}"}}"#, model_id)
}

/// Generates a standard block loot table JSON that drops `drop_item_id`.
///
/// Path: `data/<namespace>/loot_table/blocks/<path>.json`
///
/// Output:
/// ```json
/// {
///   "type": "minecraft:block",
///   "pools": [
///     {
///       "condition": {
///         "type": "minecraft:survives_explosion"
///       },
///       "entries": [
///         {
///           "type": "minecraft:item",
///           "name": "<drop_item_id>"
///         }
///       ],
///       "rolls": 1
///     }
///   ],
///   "random_sequence": "<random_seq>"
/// }
/// ```
pub fn generate_block_loot_table(drop_item_id: &str, random_seq: &str) -> String {
    format!(
        r#"{{"type":"minecraft:block","pools":[{{"condition":{{"type":"minecraft:survives_explosion"}},"entries":[{{"type":"minecraft:item","name":"{}"}}],"rolls":1}}],"random_sequence":"{}"}}"#,
        drop_item_id, random_seq
    )
}

/// Generates all virtual assets (blockstate, block model, item models) and data (loot table)
/// for a standard cube block.
pub fn generate_cube_block_assets(
    block_id: &str,
    texture: &str,
    drop_item: Option<&str>,
    has_item: bool,
    generate_loot_table: bool,
) -> Vec<VirtualResource> {
    let (namespace, path) = split_identifier(block_id);
    let mut resources = Vec::new();

    let model_id = format!("{}:block/{}", namespace, path);

    // 1. Blockstate: assets/<namespace>/blockstates/<path>.json
    resources.push(VirtualResource {
        pack_type: "assets".to_string(),
        namespace: namespace.to_string(),
        path: format!("blockstates/{}.json", path),
        content: generate_cube_blockstate(&model_id).into_bytes(),
    });

    // 2. Block model: assets/<namespace>/models/block/<path>.json
    resources.push(VirtualResource {
        pack_type: "assets".to_string(),
        namespace: namespace.to_string(),
        path: format!("models/block/{}.json", path),
        content: generate_cube_block_model(texture).into_bytes(),
    });

    // 3. Item models (if block has an item)
    if has_item {
        // Modern format: assets/<namespace>/items/<path>.json
        resources.push(VirtualResource {
            pack_type: "assets".to_string(),
            namespace: namespace.to_string(),
            path: format!("items/{}.json", path),
            content: generate_item_model_definition(&model_id).into_bytes(),
        });

        // Fallback model: assets/<namespace>/models/item/<path>.json
        resources.push(VirtualResource {
            pack_type: "assets".to_string(),
            namespace: namespace.to_string(),
            path: format!("models/item/{}.json", path),
            content: generate_legacy_item_model(&model_id).into_bytes(),
        });
    }

    // 4. Loot table: data/<namespace>/loot_table/blocks/<path>.json
    if generate_loot_table {
        let drop = drop_item.unwrap_or(block_id);
        let random_seq = format!("{}:blocks/{}", namespace, path);
        let loot_json = generate_block_loot_table(drop, &random_seq).into_bytes();

        // Standard 26.3 singular path
        resources.push(VirtualResource {
            pack_type: "data".to_string(),
            namespace: namespace.to_string(),
            path: format!("loot_table/blocks/{}.json", path),
            content: loot_json.clone(),
        });

        // Plural alias for backward compatibility
        resources.push(VirtualResource {
            pack_type: "data".to_string(),
            namespace: namespace.to_string(),
            path: format!("loot_tables/blocks/{}.json", path),
            content: loot_json,
        });
    }

    resources
}
