//! Version Adapter API for Redox-MC Mod Loader.
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
    /// Bridge class implementing the callback (e.g. "net/redoxmc/bridge/RedoxBridge")
    pub bridge_class: &'static str,
    /// Bridge method name (e.g. "onBootstrapEntry")
    pub bridge_method: &'static str,
    /// Bridge method descriptor (e.g. "()V")
    pub bridge_desc: &'static str,
    /// Optional post-invocation bridge method called immediately after the original method
    pub post_bridge_method: Option<&'static str>,
    /// Optional post-invocation bridge method descriptor
    pub post_bridge_desc: Option<&'static str>,
}

/// Trait implemented by version-specific adapters (e.g. redox-mc-adapter-26-3).
pub trait VersionAdapter: Send + Sync {
    /// Minecraft version string (e.g. "26.3")
    fn version_id(&self) -> &'static str;

    /// Expected network protocol version (e.g. 777)
    fn protocol_version(&self) -> i32;

    /// Expected JVM classfile major version (e.g. 69 for Java 25)
    fn classfile_version(&self) -> u16;

    /// Canonical list of bytecode hook targets for this version
    fn hook_targets(&self) -> &'static [HookTarget];

    /// Client-only hook targets, which are absent from dedicated server jars.
    ///
    /// Adapters that support a graphical client override this to expose hooks
    /// that must be transformed when the client classes are present.
    fn client_hook_targets(&self) -> &'static [HookTarget] {
        &[]
    }

    /// Finds a hook target by internal class name (e.g. "net/minecraft/server/Bootstrap"),
    /// searching both server and client hook tables.
    fn find_hook(&self, class_name: &str) -> Option<&'static HookTarget> {
        self.hook_targets()
            .iter()
            .chain(self.client_hook_targets())
            .find(|t| t.class_name == class_name)
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
