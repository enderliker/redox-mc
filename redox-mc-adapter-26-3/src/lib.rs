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
        post_bridge_method: None,
        post_bridge_desc: None,
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
        post_bridge_method: Some("onRegistriesFrozen"),
        post_bridge_desc: Some("()V"),
    },
    // 3. Server tick loop: called every tick on MinecraftServer
    HookTarget {
        class_name: "net/minecraft/server/MinecraftServer",
        method_name: "tickChildren",
        method_desc: "(Ljava/util/function/BooleanSupplier;)V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/event/ServerEvents",
        bridge_method: "onServerTick",
        bridge_desc: "(Lnet/minecraft/server/MinecraftServer;)V",
        post_bridge_method: None,
        post_bridge_desc: None,
    },
    // 4. Server chat handling: intercept incoming player chat packets
    HookTarget {
        class_name: "net/minecraft/server/network/ServerGamePacketListenerImpl",
        method_name: "handleChat",
        method_desc: "(Lnet/minecraft/network/protocol/game/ServerboundChatPacket;)V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/event/ServerEvents",
        bridge_method: "onHandleChat",
        bridge_desc: "(Lnet/minecraft/server/network/ServerGamePacketListenerImpl;Lnet/minecraft/network/protocol/game/ServerboundChatPacket;)Z",
        post_bridge_method: None,
        post_bridge_desc: None,
    },
    // 5. Serverbound mod payloads: play phase override.
    HookTarget {
        class_name: "net/minecraft/server/network/ServerGamePacketListenerImpl",
        method_name: "handleCustomPayload",
        method_desc: "(Lnet/minecraft/network/protocol/common/ServerboundCustomPayloadPacket;)V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/network/ModNetworkBridge",
        bridge_method: "onServerCustomPayload",
        bridge_desc: "(Lnet/minecraft/server/network/ServerCommonPacketListenerImpl;Lnet/minecraft/network/protocol/common/ServerboundCustomPayloadPacket;)V",
        post_bridge_method: None,
        post_bridge_desc: None,
    },
    // 6. Serverbound mod payloads: configuration phase and shared listener fallback.
    HookTarget {
        class_name: "net/minecraft/server/network/ServerCommonPacketListenerImpl",
        method_name: "handleCustomPayload",
        method_desc: "(Lnet/minecraft/network/protocol/common/ServerboundCustomPayloadPacket;)V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/network/ModNetworkBridge",
        bridge_method: "onServerCustomPayload",
        bridge_desc: "(Lnet/minecraft/server/network/ServerCommonPacketListenerImpl;Lnet/minecraft/network/protocol/common/ServerboundCustomPayloadPacket;)V",
        post_bridge_method: None,
        post_bridge_desc: None,
    },
    // 7. Pack discovery: inject virtual mod pack into PackRepository.discoverAvailable()
    HookTarget {
        class_name: "net/minecraft/server/packs/repository/PackRepository",
        method_name: "discoverAvailable",
        method_desc: "()Ljava/util/Map;",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/resource/VirtualModPackRepositorySource",
        bridge_method: "onPreDiscoverPacks",
        bridge_desc: "()V",
        post_bridge_method: Some("injectVirtualPack"),
        post_bridge_desc: Some("(Ljava/util/Map;)Ljava/util/Map;"),
    },
    // 8. Custom packet payload codec: wrap CustomPacketPayload.codec to decode mod payloads into ModRawPayload
    HookTarget {
        class_name: "net/minecraft/network/protocol/common/custom/CustomPacketPayload",
        method_name: "codec",
        method_desc: "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload$FallbackProvider;Ljava/util/List;)Lnet/minecraft/network/codec/StreamCodec;",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/network/ModNetworkBridge",
        bridge_method: "onPreCustomPayloadCodec",
        bridge_desc: "()V",
        post_bridge_method: Some("wrapCodec"),
        post_bridge_desc: Some("(Lnet/minecraft/network/codec/StreamCodec;)Lnet/minecraft/network/codec/StreamCodec;"),
    },
    // 9. Server configuration handshake: intercept startConfiguration() to enqueue ModHandshakeConfigurationTask
    HookTarget {
        class_name: "net/minecraft/server/network/ServerConfigurationPacketListenerImpl",
        method_name: "startConfiguration",
        method_desc: "()V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/network/ModNetworkBridge",
        bridge_method: "onStartConfiguration",
        bridge_desc: "(Lnet/minecraft/server/network/ServerConfigurationPacketListenerImpl;)V",
        post_bridge_method: None,
        post_bridge_desc: None,
    },
];

/// Client-only hook table for Minecraft 26.3.
///
/// Descriptors verified with `javap -s -p` against a legally acquired 26.3
/// client jar. These classes are absent from dedicated server jars, so the
/// loader only transforms them when they are actually loaded.
pub static CLIENT_HOOK_TARGETS: &[HookTarget] = &[
    // 1. Play-phase inbound mod payloads.
    HookTarget {
        class_name: "net/minecraft/client/multiplayer/ClientPacketListener",
        method_name: "handleCustomPayload",
        method_desc: "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;)V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/network/ModNetworkBridge",
        bridge_method: "onClientCustomPayload",
        bridge_desc: "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;)V",
        post_bridge_method: None,
        post_bridge_desc: None,
    },
    // 2. Configuration-phase inbound mod payloads.
    HookTarget {
        class_name: "net/minecraft/client/multiplayer/ClientConfigurationPacketListenerImpl",
        method_name: "handleCustomPayload",
        method_desc: "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;)V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/network/ModNetworkBridge",
        bridge_method: "onClientCustomPayload",
        bridge_desc: "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;)V",
        post_bridge_method: None,
        post_bridge_desc: None,
    },
    // 3. Client tick: flush queued outbound mod messages.
    HookTarget {
        class_name: "net/minecraft/client/Minecraft",
        method_name: "tick",
        method_desc: "()V",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/network/ModNetworkBridge",
        bridge_method: "onClientTick",
        bridge_desc: "()V",
        post_bridge_method: None,
        post_bridge_desc: None,
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

    fn client_hook_targets(&self) -> &'static [HookTarget] {
        CLIENT_HOOK_TARGETS
    }
}

pub static ADAPTER: Adapter26_3 = Adapter26_3;

/// Returns the singleton adapter for Minecraft 26.3.
pub fn get_adapter() -> &'static dyn VersionAdapter {
    &ADAPTER
}
