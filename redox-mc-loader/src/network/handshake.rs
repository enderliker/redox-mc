//! Mod network handshake and content hash verification for CONFIGURATION phase.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeModInfo {
    pub id: String,
    pub version: String,
}

/// Validates client handshake information against server requirements.
///
/// Returns `Ok(())` if validation succeeds, or `Err(reason)` with a descriptive
/// message if client hash or mods do not match the server.
pub fn validate_client_handshake(
    server_hash: &str,
    server_mods: &[HandshakeModInfo],
    client_hash: &str,
    client_mods: &[HandshakeModInfo],
) -> Result<(), String> {
    // 1. Verify content hash (if server has any custom registry content)
    if !server_hash.is_empty() && server_hash != client_hash {
        return Err(format!(
            "Mismatched registry content hash! Server: {}, Client: {}. Please ensure you have identical mods and mod versions installed.",
            server_hash, client_hash
        ));
    }

    // 2. Index client mods by mod ID, rejecting duplicates and malformed entries
    let mut client_mod_map: HashMap<&str, &str> = HashMap::with_capacity(client_mods.len());
    for client_mod in client_mods {
        if client_mod.id.is_empty() {
            return Err("Client reported a mod with an empty identifier.".to_string());
        }
        if client_mod_map
            .insert(client_mod.id.as_str(), client_mod.version.as_str())
            .is_some()
        {
            return Err(format!(
                "Client reported duplicate mod '{}' in its handshake.",
                client_mod.id
            ));
        }
    }

    // 3. Verify all required server mods exist on client with matching versions
    for s_mod in server_mods {
        match client_mod_map.get(s_mod.id.as_str()) {
            None => {
                return Err(format!(
                    "Missing required mod: {} (version {})",
                    s_mod.id, s_mod.version
                ));
            }
            Some(&c_version) => {
                if c_version != s_mod.version.as_str() {
                    return Err(format!(
                        "Incompatible version for mod '{}': Server has {}, but Client has {}",
                        s_mod.id, s_mod.version, c_version
                    ));
                }
            }
        }
    }

    Ok(())
}
