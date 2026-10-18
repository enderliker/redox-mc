use redox_mc_loader::manifest::{ManifestError, ModManifest, ModSide};
use redox_mc_loader::mod_resolver::{resolve_load_order, ResolverError};
use semver::{Version, VersionReq};

fn loader_version() -> Version {
    Version::parse("0.1.0").unwrap()
}

#[test]
fn test_manifest_parsing_valid_and_unknown_fields() {
    let toml_data = r#"
id = "redox_test_mod"
name = "Redox Test Mod"
version = "1.2.3"
side = "both"
min_loader_version = ">=0.1.0"
description = "A comprehensive test mod for Redox-MC"
authors = ["Ender", "DeepMind"]
license = "MIT"
future_unknown_field = "should be ignored gracefully"

[dependencies]
simple_dep = "^1.0.0"
detailed_dep = { version = ">=2.1.0", optional = true }
"#;

    let manifest =
        ModManifest::from_toml_str(toml_data).expect("Manifest should parse successfully");
    assert_eq!(manifest.id, "redox_test_mod");
    assert_eq!(manifest.display_name(), "Redox Test Mod");
    assert_eq!(manifest.version, Version::parse("1.2.3").unwrap());
    assert_eq!(manifest.side, ModSide::Both);
    assert_eq!(
        manifest.min_loader_version,
        Some(VersionReq::parse(">=0.1.0").unwrap())
    );
    assert_eq!(manifest.authors, vec!["Ender", "DeepMind"]);
    assert_eq!(manifest.license.as_deref(), Some("MIT"));

    assert_eq!(manifest.dependencies.len(), 2);
    let simple = &manifest.dependencies["simple_dep"];
    assert_eq!(simple.version, VersionReq::parse("^1.0.0").unwrap());
    assert!(!simple.optional);

    let detailed = &manifest.dependencies["detailed_dep"];
    assert_eq!(detailed.version, VersionReq::parse(">=2.1.0").unwrap());
    assert!(detailed.optional);
}

#[test]
fn test_manifest_parsing_invalid_ids() {
    // Upper case not allowed
    assert!(matches!(
        ModManifest::from_toml_str("id = 'InvalidID'\nversion = '1.0.0'"),
        Err(ManifestError::InvalidModId(_))
    ));

    // Spaces not allowed
    assert!(matches!(
        ModManifest::from_toml_str("id = 'invalid id'\nversion = '1.0.0'"),
        Err(ManifestError::InvalidModId(_))
    ));

    // Empty not allowed
    assert!(matches!(
        ModManifest::from_toml_str("id = ''\nversion = '1.0.0'"),
        Err(ManifestError::InvalidModId(_))
    ));
}

#[test]
fn test_environment_side_filtering() {
    let client_mod =
        ModManifest::from_toml_str("id = 'client_mod'\nversion = '1.0.0'\nside = 'client'")
            .unwrap();
    let server_mod =
        ModManifest::from_toml_str("id = 'server_mod'\nversion = '1.0.0'\nside = 'server'")
            .unwrap();
    let common_mod =
        ModManifest::from_toml_str("id = 'common_mod'\nversion = '1.0.0'\nside = 'both'").unwrap();

    let mods = vec![client_mod, server_mod, common_mod];

    // Running on DedicatedServer: client_mod must be ignored
    let server_order =
        resolve_load_order(mods.clone(), ModSide::Server, &loader_version()).unwrap();
    let ids: Vec<&str> = server_order.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["common_mod", "server_mod"]);

    // Running on Client: server_mod must be ignored
    let client_order = resolve_load_order(mods, ModSide::Client, &loader_version()).unwrap();
    let ids: Vec<&str> = client_order.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["client_mod", "common_mod"]);
}

#[test]
fn test_deterministic_lexicographical_order_for_independent_mods() {
    let mod_z = ModManifest::from_toml_str("id = 'zebra_mod'\nversion = '1.0.0'").unwrap();
    let mod_a = ModManifest::from_toml_str("id = 'apple_mod'\nversion = '1.0.0'").unwrap();
    let mod_m = ModManifest::from_toml_str("id = 'mango_mod'\nversion = '1.0.0'").unwrap();
    let mod_b = ModManifest::from_toml_str("id = 'banana_mod'\nversion = '1.0.0'").unwrap();

    // Pass in reverse / arbitrary order
    let input = vec![mod_z.clone(), mod_m.clone(), mod_b.clone(), mod_a.clone()];
    let order = resolve_load_order(input, ModSide::Both, &loader_version()).unwrap();

    let ids: Vec<&str> = order.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["apple_mod", "banana_mod", "mango_mod", "zebra_mod"]
    );

    // Shuffled input must produce the EXACT same order
    let input2 = vec![mod_a, mod_z, mod_m, mod_b];
    let order2 = resolve_load_order(input2, ModSide::Both, &loader_version()).unwrap();
    let ids2: Vec<&str> = order2.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ids2);
}

#[test]
fn test_topological_dependency_ordering() {
    let toml_a = "id = 'core_lib'\nversion = '1.0.0'";
    let toml_b = r#"
id = 'energy_api'
version = '1.0.0'
[dependencies]
core_lib = "^1.0.0"
"#;
    let toml_c = r#"
id = 'tech_machines'
version = '1.0.0'
[dependencies]
energy_api = "^1.0.0"
"#;
    let toml_d = r#"
id = 'addon_solar'
version = '1.0.0'
[dependencies]
tech_machines = "^1.0.0"
core_lib = "^1.0.0"
"#;

    // Independent mod that should sort alphabetically at its level
    let toml_indep = "id = 'cosmetics'\nversion = '1.0.0'";

    let manifests = vec![
        ModManifest::from_toml_str(toml_d).unwrap(),
        ModManifest::from_toml_str(toml_c).unwrap(),
        ModManifest::from_toml_str(toml_indep).unwrap(),
        ModManifest::from_toml_str(toml_b).unwrap(),
        ModManifest::from_toml_str(toml_a).unwrap(),
    ];

    let order = resolve_load_order(manifests, ModSide::Both, &loader_version()).unwrap();
    let ids: Vec<&str> = order.iter().map(|m| m.id.as_str()).collect();

    // core_lib and cosmetics have in-degree 0 initially: cosmetics comes first alphabetically, then core_lib.
    // energy_api depends on core_lib.
    // tech_machines depends on energy_api.
    // addon_solar depends on tech_machines and core_lib.
    assert_eq!(
        ids,
        vec![
            "core_lib",
            "cosmetics",
            "energy_api",
            "tech_machines",
            "addon_solar"
        ]
    );
}

#[test]
fn test_missing_dependency() {
    let toml = r#"
id = 'my_mod'
version = '1.0.0'
[dependencies]
missing_lib = ">=1.0.0"
"#;
    let manifest = ModManifest::from_toml_str(toml).unwrap();
    let err = resolve_load_order(vec![manifest], ModSide::Both, &loader_version()).unwrap_err();

    assert!(matches!(
        err,
        ResolverError::MissingDependency {
            ref mod_id,
            ref dependency,
            ..
        } if mod_id == "my_mod" && dependency == "missing_lib"
    ));
}

#[test]
fn test_version_mismatch() {
    let toml_lib = "id = 'shared_lib'\nversion = '1.5.0'";
    let toml_mod = r#"
id = 'demanding_mod'
version = '1.0.0'
[dependencies]
shared_lib = ">=2.0.0"
"#;
    let manifests = vec![
        ModManifest::from_toml_str(toml_lib).unwrap(),
        ModManifest::from_toml_str(toml_mod).unwrap(),
    ];
    let err = resolve_load_order(manifests, ModSide::Both, &loader_version()).unwrap_err();

    assert!(matches!(
        err,
        ResolverError::VersionMismatch {
            ref mod_id,
            ref dependency,
            ref found,
            ..
        } if mod_id == "demanding_mod" && dependency == "shared_lib" && *found == Version::parse("1.5.0").unwrap()
    ));
}

#[test]
fn test_circular_dependency_detection() {
    let toml_a = r#"
id = 'mod_a'
version = '1.0.0'
[dependencies]
mod_b = "^1.0.0"
"#;
    let toml_b = r#"
id = 'mod_b'
version = '1.0.0'
[dependencies]
mod_c = "^1.0.0"
"#;
    let toml_c = r#"
id = 'mod_c'
version = '1.0.0'
[dependencies]
mod_a = "^1.0.0"
"#;

    let manifests = vec![
        ModManifest::from_toml_str(toml_a).unwrap(),
        ModManifest::from_toml_str(toml_b).unwrap(),
        ModManifest::from_toml_str(toml_c).unwrap(),
    ];

    let err = resolve_load_order(manifests, ModSide::Both, &loader_version()).unwrap_err();
    assert!(matches!(err, ResolverError::CircularDependency(_)));
}

#[test]
fn test_duplicate_mod_detection() {
    let toml1 = "id = 'duplicate_mod'\nversion = '1.0.0'";
    let toml2 = "id = 'duplicate_mod'\nversion = '1.0.1'";

    let manifests = vec![
        ModManifest::from_toml_str(toml1).unwrap(),
        ModManifest::from_toml_str(toml2).unwrap(),
    ];

    let err = resolve_load_order(manifests, ModSide::Both, &loader_version()).unwrap_err();
    assert!(matches!(err, ResolverError::DuplicateMod(ref id, ..) if id == "duplicate_mod"));
}

#[test]
fn test_incompatible_loader_version() {
    let toml = r#"
id = 'future_mod'
version = '1.0.0'
min_loader_version = ">=2.0.0"
"#;
    let manifest = ModManifest::from_toml_str(toml).unwrap();
    let current_loader = Version::parse("0.1.0").unwrap();

    let err = resolve_load_order(vec![manifest], ModSide::Both, &current_loader).unwrap_err();
    assert!(matches!(
        err,
        ResolverError::IncompatibleLoaderVersion {
            ref mod_id,
            ref found,
            ..
        } if mod_id == "future_mod" && *found == current_loader
    ));
}
