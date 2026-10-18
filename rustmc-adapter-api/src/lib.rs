//! Version Adapter API for RustMC Mod Loader.
//!
//! Provides neutral traits and types for decoupling version-specific Minecraft details
//! (class names, JVM descriptors, protocol versions, bytecode hooks) from the core loader.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookTechnique {
    /// Wraps the original method by creating a prefix/entry or wrapper call
    Wrapper,
    /// Injects at method entry point
    Entry,
    /// Injects at method return point
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookTarget {
    /// Internal JVM class name with slashes (e.g. "net/minecraft/server/Bootstrap")
    pub class_name: &'static str,
    /// Target method name (e.g. "bootStrap")
    pub method_name: &'static str,
    /// Exact JVM method descriptor (e.g. "()V")
    pub method_desc: &'static str,
    /// Injection technique to be used by bytecode transformer
    pub technique: HookTechnique,
    /// Bridge class implementing the callback (e.g. "net/rustmc/bridge/RustBridge")
    pub bridge_class: &'static str,
    /// Bridge method name (e.g. "onBootstrapEntry")
    pub bridge_method: &'static str,
    /// Bridge method descriptor (e.g. "()V")
    pub bridge_desc: &'static str,
}

/// Trait implemented by version-specific adapters (e.g. rustmc-adapter-26-3).
pub trait VersionAdapter: Send + Sync {
    /// Minecraft version string (e.g. "26.3")
    fn version_id(&self) -> &'static str;

    /// Expected network protocol version (e.g. 777)
    fn protocol_version(&self) -> i32;

    /// Expected JVM classfile major version (e.g. 69 for Java 25)
    fn classfile_version(&self) -> u16;

    /// Canonical list of bytecode hook targets for this version
    fn hook_targets(&self) -> &'static [HookTarget];

    /// Finds a hook target by internal class name (e.g. "net/minecraft/server/Bootstrap")
    fn find_hook(&self, class_name: &str) -> Option<&'static HookTarget> {
        self.hook_targets().iter().find(|t| t.class_name == class_name)
    }

    /// Package prefix for game classes (defaults to "net/minecraft/")
    fn game_package_prefix(&self) -> &'static str {
        "net/minecraft/"
    }

    /// Primary server entrypoint class
    fn server_main_class(&self) -> &'static str {
        "net/minecraft/server/Main"
    }
}
