//! Lock-free callback table for hot-reloadable native mods.
//!
//! Callback dispatch must never observe a partially updated mod list. The
//! [`ModManager`](crate::mod_loader::ModManager) publishes an immutable snapshot
//! through [`arc_swap::ArcSwap`], and every dispatch path loads that snapshot
//! before invoking mod callbacks. In-flight snapshots keep their `LoadedMod`
//! instances (and therefore their dynamic libraries) alive until the call
//! returns, even if a reload publishes a new generation in the meantime.

use crate::mod_loader::LoadedMod;
use arc_swap::ArcSwap;
use std::sync::{Arc, OnceLock};

type ModSnapshot = Arc<Vec<Arc<LoadedMod>>>;

static CALLBACKS: OnceLock<ArcSwap<Vec<Arc<LoadedMod>>>> = OnceLock::new();

fn callback_table() -> &'static ArcSwap<Vec<Arc<LoadedMod>>> {
    CALLBACKS.get_or_init(|| ArcSwap::from_pointee(Vec::new()))
}

/// Publishes a new immutable callback snapshot.
///
/// The swap is atomic: concurrent dispatchers see either the complete previous
/// generation or the complete new generation.
pub fn publish(mods: Vec<Arc<LoadedMod>>) {
    callback_table().store(Arc::new(mods));
}

/// Returns the current immutable callback snapshot.
///
/// Holding the returned `Arc` guarantees the referenced libraries remain loaded
/// for the duration of the dispatch, even if a hot reload happens concurrently.
pub fn snapshot() -> ModSnapshot {
    callback_table().load_full()
}
