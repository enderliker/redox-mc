# ADR-0005: Strongly-Typed Pub/Sub Event Bus with Deterministic Priorities

## Status
Accepted

## Date
2026-10-18

## Context
Event handling is the core integration point between the Minecraft game engine and native mods. Typical events include server ticks, block placement/breaking, player interaction, chat messages, registry registration, and entity lifecycle hooks.

In Java mod loaders (Forge, NeoForge, Fabric), event systems often rely on runtime reflection, annotation scanners, or runtime bytecode generation (e.g. ASM generated invokers).

In native Rust:
1. **Type Safety:** The event system must leverage Rust's strong compile-time type system, eliminating runtime reflection overhead and type-casting bugs.
2. **Deterministic Ordering:** Multiple mods listening to the same event must execute in a strictly defined order (e.g. permission checks or security mods must execute before gameplay mods).
3. **Cancellation Control:** Critical events (such as damage, command execution, or block breaking) must support cancellation or short-circuiting.
4. **Thread Safety:** Event registration and dispatch must be thread-safe (`Send + Sync`) to accommodate Minecraft's multi-threaded background workers and asynchronous tasks.

## Decision
Implement a strongly-typed, memory-safe Pub/Sub Event Bus in `redox-mc-api`:
1. **Core Traits:**
   - `Event`: Base trait representing an event payload (`'static + Any`).
   - `CancellableEvent`: Extends `Event` with `is_cancelled(&self) -> bool` and `cancel(&mut self)`.
2. **Prioritization:**
   - `EventPriority`: An ordered enum with variants `Highest`, `Normal`, `Lowest`.
   - Handlers are sorted by priority during subscription so that dispatch iterates in deterministic order without sorting during the hot dispatch path.
3. **Dispatch Mechanism:**
   - Handlers are stored indexed by `TypeId::of::<E>()` inside an `RwLock<HashMap<TypeId, Vec<HandlerRecord>>>`.
   - Dispatch passes `&mut E` to registered handlers in sequence.
   - For `CancellableEvent`, subsequent listeners can observe the cancellation state or short-circuit according to event semantics.
4. **Result Control:**
   - `EventResult`: An explicit enum (`Allow`, `Cancel`) for methods that report fine-grained resolution status.

## Alternatives Considered

### Dynamic String-Based Event Identifiers
- **Pros:** Flexible, dynamic registration without concrete Rust types.
- **Cons:** Bypasses compile-time type checking; requires runtime string hashing, deserialization, or dynamic downcasting on every dispatch; error-prone typos.
- **Rejected:** Incompatible with Rust's zero-cost type safety ethos.

### Java-Style Annotation Preprocessing
- **Pros:** Familiar to Fabric/Forge developers (`@SubscribeEvent`).
- **Cons:** Requires complex macro reflection or build-time code generation scanning; adds compile-time overhead.
- **Rejected:** Direct closure/function pointer subscription (`bus.subscribe(...)`) is simpler, more idiomatic in Rust, and easier to test.

### Lockless Channel / Actor Messaging
- **Pros:** Non-blocking async dispatch.
- **Cons:** Game events often require synchronous evaluation (e.g. "should this block be broken right now?"). Asynchronous channels introduce multi-tick latency and complex synchronization when an immediate decision is required.
- **Rejected:** Minecraft game loop ticks require synchronous, in-tick resolution.

## Consequences
- **Zero Reflection:** Dispatches are fast, type-safe Rust function calls with zero JNI overhead once inside native code.
- **Deterministic Pipeline:** Higher priority handlers reliably execute before lower priority ones.
- **Extensible:** Mod developers can define custom event types by simply implementing `Event` or `CancellableEvent`.
- **Unit Testable:** `EventBus` can be instantiated locally in tests without requiring a mock Minecraft server or JVM runtime.
