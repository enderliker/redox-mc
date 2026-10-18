use crate::manifest::{ModManifest, ModSide};
use semver::{Version, VersionReq};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResolverError {
    #[error("Duplicate mod ID found: '{0}' (versions: {1} and {2})")]
    DuplicateMod(String, Version, Version),

    #[error("Mod '{mod_id}' requires Redox-MC loader version {required}, but running on version {found}")]
    IncompatibleLoaderVersion {
        mod_id: String,
        required: VersionReq,
        found: Version,
    },

    #[error("Mod '{mod_id}' depends on missing mod '{dependency}' ({required})")]
    MissingDependency {
        mod_id: String,
        dependency: String,
        required: String,
    },

    #[error("Mod '{mod_id}' requires '{dependency}' version {required}, but found incompatible version {found}")]
    VersionMismatch {
        mod_id: String,
        dependency: String,
        required: String,
        found: Version,
    },

    #[error("Circular dependency detected: {}", .0.join(" -> "))]
    CircularDependency(Vec<String>),
}

/// Resolves dependencies and returns a strictly deterministic load order.
///
/// Determinism guarantees:
/// 1. If mod A depends on mod B, mod B is guaranteed to precede mod A in the load order.
/// 2. If two or more mods have no dependency relationship, their load order is determined
///    lexicographically by their `mod_id`.
pub fn resolve_load_order(
    manifests: Vec<ModManifest>,
    running_side: ModSide,
    loader_version: &Version,
) -> Result<Vec<ModManifest>, ResolverError> {
    // 1. Check duplicates and filter out mods incompatible with the running side
    let mut available_mods: BTreeMap<String, ModManifest> = BTreeMap::new();

    for manifest in manifests {
        if !manifest.is_compatible_with(running_side) {
            continue;
        }

        if let Some(existing) = available_mods.get(&manifest.id) {
            return Err(ResolverError::DuplicateMod(
                manifest.id.clone(),
                existing.version.clone(),
                manifest.version.clone(),
            ));
        }

        available_mods.insert(manifest.id.clone(), manifest);
    }

    // 2. Validate loader version compatibility
    for manifest in available_mods.values() {
        if let Some(ref min_req) = manifest.min_loader_version {
            if !min_req.matches(loader_version) {
                return Err(ResolverError::IncompatibleLoaderVersion {
                    mod_id: manifest.id.clone(),
                    required: min_req.clone(),
                    found: loader_version.clone(),
                });
            }
        }
    }

    // 3. Validate dependencies
    // Graph representation:
    // in_degree: number of unsatisfied dependencies for each mod
    // dependents: map from dependency -> list of mods waiting for it
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for (mod_id, manifest) in &available_mods {
        let mut count = 0;

        for (dep_id, dep_spec) in &manifest.dependencies {
            if let Some(dep_manifest) = available_mods.get(dep_id) {
                if !dep_spec.version.matches(&dep_manifest.version) {
                    return Err(ResolverError::VersionMismatch {
                        mod_id: mod_id.clone(),
                        dependency: dep_id.clone(),
                        required: dep_spec.version.to_string(),
                        found: dep_manifest.version.clone(),
                    });
                }
                // Valid dependency: dep must load before mod_id
                dependents
                    .entry(dep_id.clone())
                    .or_default()
                    .push(mod_id.clone());
                count += 1;
            } else if !dep_spec.optional {
                return Err(ResolverError::MissingDependency {
                    mod_id: mod_id.clone(),
                    dependency: dep_id.clone(),
                    required: dep_spec.version.to_string(),
                });
            }
        }

        in_degree.insert(mod_id.clone(), count);
    }

    // 4. Deterministic Kahn's Algorithm
    // Using BTreeSet ensures that ties are ALWAYS broken in lexicographical order of mod ID.
    let mut ready_set: BTreeSet<String> = BTreeSet::new();

    for (mod_id, deg) in &in_degree {
        if *deg == 0 {
            ready_set.insert(mod_id.clone());
        }
    }

    let mut load_order = Vec::with_capacity(available_mods.len());

    while let Some(next_mod_id) = ready_set.pop_first() {
        if let Some(manifest) = available_mods.get(&next_mod_id) {
            load_order.push(manifest.clone());
        }

        if let Some(waiting_list) = dependents.get(&next_mod_id) {
            for dependent_id in waiting_list {
                if let Some(deg) = in_degree.get_mut(dependent_id) {
                    *deg -= 1;
                    if *deg == 0 {
                        ready_set.insert(dependent_id.clone());
                    }
                }
            }
        }
    }

    // 5. Circular Dependency Check
    if load_order.len() < available_mods.len() {
        let cycle = find_cycle(&in_degree, &dependents);
        return Err(ResolverError::CircularDependency(cycle));
    }

    Ok(load_order)
}

/// Finds and returns a cycle path in the dependency graph for clear error reporting.
fn find_cycle(
    in_degree: &HashMap<String, usize>,
    dependents: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    // Collect all nodes that could not be scheduled (in_degree > 0)
    let cycle_candidates: BTreeSet<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg > 0)
        .map(|(id, _)| id.clone())
        .collect();

    // Perform DFS to find a cycle
    let mut visited: HashMap<String, usize> = HashMap::new(); // 1 = visiting, 2 = visited
    let mut path: Vec<String> = Vec::new();

    for start_node in &cycle_candidates {
        if dfs_cycle(start_node, dependents, &mut visited, &mut path) {
            return path;
        }
    }

    cycle_candidates.into_iter().collect()
}

fn dfs_cycle(
    node: &str,
    dependents: &HashMap<String, Vec<String>>,
    visited: &mut HashMap<String, usize>,
    path: &mut Vec<String>,
) -> bool {
    visited.insert(node.to_string(), 1);
    path.push(node.to_string());

    if let Some(next_nodes) = dependents.get(node) {
        for next in next_nodes {
            match visited.get(next) {
                Some(1) => {
                    // Cycle found: append target to complete the loop
                    path.push(next.clone());
                    // Trim path to only include cycle elements
                    if let Some(first_idx) = path.iter().position(|x| x == next) {
                        *path = path[first_idx..].to_vec();
                    }
                    return true;
                }
                Some(2) => continue,
                _ => {
                    if dfs_cycle(next, dependents, visited, path) {
                        return true;
                    }
                }
            }
        }
    }

    path.pop();
    visited.insert(node.to_string(), 2);
    false
}
