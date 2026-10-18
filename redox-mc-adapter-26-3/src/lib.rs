//! Minecraft 26.3 Version Adapter for Redox-MC Mod Loader.
//!
//! Encapsulates all version-specific identifiers, bytecode descriptors, and
//! hook targets verified via javap from official Minecraft 26.3 jars.

use redox_mc_adapter_api::{HookTarget, HookTechnique, VersionAdapter};

pub const VERSION_ID: &str = "26.3";
pub const PROTOCOL_VERSION: i32 = 777;
pub const CLASSFILE_VERSION: u16 = 69; // Java 25

/// Canonical hook table for Minecraft 26.3.
/// Descriptors extracted and verified with `javap -s -p` against server-26.3.jar.
pub static HOOK_TARGETS: &[HookTarget] = &[
    // 1. Game bootstrap entrypoint: called at earliest game initialization
    HookTarget {
        class_name: "net/minecraft/server/Bootstrap",
        method_name: "bootStrap",
        method_desc: "()V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/RedoxBridge",
        bridge_method: "onBootstrapEntry",
        bridge_desc: "()V",
    },
    // 2. Registry sealing point: called right before BuiltInRegistries are frozen
    HookTarget {
        class_name: "net/minecraft/core/registries/BuiltInRegistries",
        method_name: "freeze",
        method_desc: "()V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/RedoxBridge",
        bridge_method: "onRegisterContent",
        bridge_desc: "()V",
    },
    // 3. Server tick loop: called every tick on MinecraftServer
    HookTarget {
        class_name: "net/minecraft/server/MinecraftServer",
        method_name: "tickChildren",
        method_desc: "(Ljava/util/function/BooleanSupplier;)V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/RedoxBridge",
        bridge_method: "onServerTick",
        bridge_desc: "()V",
    },
    // 4. Server chat handling: intercept incoming player chat packets
    HookTarget {
        class_name: "net/minecraft/server/network/ServerGamePacketListenerImpl",
        method_name: "handleChat",
        method_desc: "(Lnet/minecraft/network/protocol/game/ServerboundChatPacket;)V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/RedoxBridge",
        bridge_method: "onHandleChat",
        bridge_desc: "(Ljava/lang/Object;)Z",
    },
];

#[derive(Default, Debug, Clone, Copy)]
pub struct Adapter26_3;

impl VersionAdapter for Adapter26_3 {
    fn version_id(&self) -> &'static str {
        VERSION_ID
    }

    fn protocol_version(&self) -> i32 {
        PROTOCOL_VERSION
    }

    fn classfile_version(&self) -> u16 {
        CLASSFILE_VERSION
    }

    fn hook_targets(&self) -> &'static [HookTarget] {
        HOOK_TARGETS
    }
}

pub static ADAPTER: Adapter26_3 = Adapter26_3;

/// Returns the singleton adapter for Minecraft 26.3.
pub fn get_adapter() -> &'static dyn VersionAdapter {
    &ADAPTER
}
