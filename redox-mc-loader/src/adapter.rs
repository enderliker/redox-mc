use redox_mc_adapter_api::VersionAdapter;

/// Returns the active version adapter configured at build time.
/// Note: Runtime dynamic version detection across multi-version adapters is deferred to Task 10.1.
pub fn active_adapter() -> &'static dyn VersionAdapter {
    #[cfg(feature = "mc-26-3")]
    {
        redox_mc_adapter_26_3::get_adapter()
    }
    #[cfg(not(feature = "mc-26-3"))]
    {
        compile_error!("No Minecraft version adapter enabled. Enable feature 'mc-26-3'.");
    }
}


