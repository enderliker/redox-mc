# ADR-0001: JVMTI Agent and Three-Tier ClassLoader Bridge Architecture

## Status
Accepted

## Date
2026-10-18

## Context
Redox-MC is a native Rust mod loader for Minecraft Java Edition. Unlike Java-based mod loaders (Fabric, NeoForge), mod logic is written in Rust and compiled to native shared libraries (`.so`, `.dll`, `.dylib`).

Key challenges:
1. **Early JVM Attachment:** Mod loading and bytecode transformation must be hooked before the game classes are loaded and verified.
2. **Mojang Bundler ClassLoader Hierarchy:** In modern Minecraft versions (1.18+ through 26.3), the initial entry point (`Main`) executes under the `AppClassLoader`, but extracts and launches the actual game (`net.minecraft.server.Main`) inside a dedicated child `URLClassLoader`. Any classes injected into `AppClassLoader` cannot resolve types defined in the child `URLClassLoader` without triggering `ClassNotFoundException` or `NoClassDefFoundError`.
3. **No JVM Modification / No Proprietary Jar Distribution:** Redox-MC cannot distribute modified Minecraft jars or proprietary Mojang classes.

## Decision
1. Implement the mod loader as a native **JVMTI (JVM Tool Interface)** agent written in Rust (`redox-mc-loader`), loaded via `-agentpath:<path-to-loader>`.
2. Split Java bridge logic into three distinct modules across classloader boundaries:
   - `bridge-core`: A minimalist jar containing `net.redoxmc.bridge.RedoxBridge` with native method bindings (`RegisterNatives`). It has **zero dependencies on Minecraft** and is dynamically appended to the JVM `BootstrapClassLoader` search path via `AddToBootstrapClassLoaderSearch`.
   - `bridge-game`: Contains game-aware bridge classes (e.g. `GenericRustItem`, `TestMinecraftSavedData`) that subclass vanilla Minecraft types. It is dynamically defined into the captured child `URLClassLoader` via JNI `DefineClass` outside of `ClassFileLoadHook` to prevent classloader lock deadlocks.
   - `bridge-client`: Contains client-specific rendering/screen integrations (reserved for client side).
3. Capture the game `ClassLoader` by intercepting the first class loaded under the `net/minecraft/` package prefix inside `ClassFileLoadHook`, storing a global JNI reference (`NewGlobalRef`).

## Alternatives Considered

### Pure Java Agent (`-javaagent`)
- **Pros:** Standard Java mechanism; easy to write.
- **Cons:** Runs in Java; requires bootstrapping JVM runtime before native code can take control; cannot hook at the earliest JVMTI lifecycle events; necessitates JNI wrappers for every action.
- **Rejected:** Fails the core requirement of a native-first Rust loader with deterministic low-level control.

### Modifying the Vanilla Jar on Disk (Static Patching)
- **Pros:** No runtime class injection needed.
- **Cons:** Violates Mojang EULA regarding redistribution; breaks when vanilla files are updated; fragile checksum verification.
- **Rejected:** Strictly prohibited by project distribution and legal constraints.

### Single Injected Jar into System ClassLoader
- **Pros:** Simpler packaging (single jar).
- **Cons:** Fails on Mojang Bundler environments because the child `URLClassLoader` owns game classes, causing `NoClassDefFoundError` when bridge classes attempt to extend `net.minecraft` types.
- **Rejected:** Cannot support modern Minecraft server bundlers.

## Consequences
- The JVMTI agent starts before any Minecraft class is loaded.
- `bridge-core` is universally accessible across all classloaders via the bootstrap loader.
- `bridge-game` classes can directly inherit from vanilla Minecraft classes without visibility or linkage errors.
- `DefineClass` is strictly called outside `ClassFileLoadHook` to avoid JVM internal classloader deadlocks.
