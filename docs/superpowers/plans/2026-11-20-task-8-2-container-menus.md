# Task 8.2: Container Menus and Client Screens Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `GenericRedoxMenu extends AbstractContainerMenu` in `redox-mc-bridge-game`, register custom `MenuType` entries into `BuiltInRegistries.MENU`, connect slot clicks and menu lifecycle to native Rust callbacks, keep screen abstractions decoupled in `bridge-client`, and verify behavior in headless Minecraft 26.3.

**Architecture:**
- `redox-mc-abi`: Pure C-ABI definitions for `FfiMenuClickType`, `FfiMenuAction`, `FfiMenuClickEvent`, and `ModVTable` function pointers (`on_menu_open`, `on_menu_slot_click`, `on_menu_close`).
- `redox-mc-bridge`:
  - `bridge-core`: Native declarations in `RedoxBridge` for menu open, slot click, and close.
  - `bridge-game`: `GenericRedoxMenu extends AbstractContainerMenu` with customizable slots, quick move, and slot click interception allowing native mods to validate/reject item moves; `GenericRedoxMenuHelper` for reflection-based `MenuType` construction.
  - `bridge-client`: `GenericRedoxScreen` decoupled from server classpath.
- `redox-mc-loader`: JNI native shims, lazy class injection into captured game classloader outside JVMTI hooks, registration in `BuiltInRegistries.MENU`, and deterministic SHA-256 handshake hashing.
- `redox-mc-api`: Safe idiomatic Rust API `redox_mc_api::gui` / `redox_mc_api::menu` (`MenuDefinition`, `MenuClickEvent`, `MenuAction`, callback registration).
- `examples/test-mod`: Registers `testmod:counter_menu`, opens it on block interaction with `testmod:counter_block`, and validates slot clicks.
- `tests/test_container_menu_headless.py`: Integration test verifying registration in `BuiltInRegistries.MENU` and menu slot click lifecycle.

**Tech Stack:** Rust 1.85+ (2024 edition), Java 25 (classfile 69.0), JVMTI, JNI, Minecraft 26.3.

---

## Global Constraints

- **HotSpot Signal Non-Interference:** Never register OS signal handlers (`SIGSEGV`, `SIGBUS`, etc.).
- **Deadlock-Free Class Definition:** Never call `DefineClass` inside JVMTI `ClassFileLoadHook`.
- **Symmetric Memory Ownership:** Memory allocated by host is freed by host; memory allocated by mod is freed by mod.
- **FFI Panic Isolation:** Every `extern "C"` function must wrap logic in `catch_unwind(AssertUnwindSafe(...))` and return fallback codes.
- **Zero Warnings:** Pass `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check`.
- **100% Tests Passing:** Pass `cargo test --workspace` and all tests in `./tests/run_task_1_2_tests.sh`.

---

## Review Focus

1. **JNI Local Frame Safety:** Slot clicks can fire rapidly; ensure `PushLocalFrame`/`PopLocalFrame` or local reference deletion in JNI bridge shims.
2. **ContainerInput Enum Mapping:** Ensure accurate integer mapping between Minecraft 26.3 `ContainerInput` and `FfiMenuClickType`.
3. **Slot Click Cancellation Synchronization:** When a click is rejected (`DENY`), call `broadcastFullState()` so client inventory stays perfectly synced and does not desync ghost items.
4. **Deterministic Registry Hash:** Slices and menu lists must be sorted alphabetically to ensure reproducible SHA-256 handshake digests across client and server.
5. **Classloader Isolation:** `bridge-client` classes must not be referenced or instantiated on the dedicated server.

---

### Task 1: C-ABI Menu Types and ModVTable in `redox-mc-abi`

**Files:**
- Modify: `redox-mc-abi/src/types.rs`
- Modify: `redox-mc-abi/src/vtable.rs`
- Test: `redox-mc-abi/tests/layout_tests.rs`

**Steps:**
- [ ] Add `FfiMenuClickType` enum `#[repr(C)]` (Pickup=0, QuickMove=1, Swap=2, Clone=3, Throw=4, QuickCraft=5, PickupAll=6).
- [ ] Add `FfiMenuAction` enum `#[repr(C)]` (Allow=0, Deny=1).
- [ ] Add `FfiMenuClickEvent` struct `#[repr(C)]` containing `menu_id`, `container_id`, `slot_index`, `button`, `click_type`, `player_name`.
- [ ] Add function pointers to `ModVTable`: `on_menu_open`, `on_menu_slot_click`, `on_menu_close`.
- [ ] Add ABI tests in `redox-mc-abi/tests/layout_tests.rs`.
- [ ] Verify: `cargo test -p redox-mc-abi`.

---

### Task 2: Java Bridge Classes in `redox-mc-bridge`

**Files:**
- Modify: `redox-mc-bridge/bridge-core/src/main/java/net/redoxmc/bridge/RedoxBridge.java`
- Create: `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/gui/GenericRedoxMenu.java`
- Create: `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/gui/GenericRedoxMenuHelper.java`
- Create: `redox-mc-bridge/bridge-client/src/main/java/net/redoxmc/bridge/client/gui/GenericRedoxScreen.java`
- Modify: `redox-mc-bridge/build.sh`

**Steps:**
- [ ] Add `onMenuOpen`, `onMenuSlotClick`, `onMenuClose` to `RedoxBridge.java`.
- [ ] Implement `GenericRedoxMenu extends AbstractContainerMenu`:
  - Provide standard player inventory slots (9-35) and hotbar slots (0-8) + custom container slots.
  - Implement `clicked(slotIndex, button, input, player)`: delegate to `RedoxBridge.onMenuSlotClick`. If denied, call `broadcastFullState()` and return without executing super.
  - Implement `stillValid(Player player)`.
  - Implement `quickMoveStack(Player player, int slotIndex)`.
  - Implement `removed(Player player)` calling `onMenuClose`.
- [ ] Implement `GenericRedoxMenuHelper` to construct `MenuType<GenericRedoxMenu>` via reflection using `FeatureFlags.VANILLA_SET`.
- [ ] Implement `GenericRedoxScreen` in `bridge-client` as an isolated client holder.
- [ ] Update `build.sh` to compile new classes and verify jars build cleanly.

---

### Task 3: JNI Shims and Class Injection in `redox-mc-loader`

**Files:**
- Modify: `redox-mc-loader/src/class_injector.rs`
- Modify: `redox-mc-loader/src/bridge_jni/events.rs`
- Modify: `redox-mc-loader/src/bridge_jni/register.rs`
- Modify: `redox-mc-loader/src/events.rs`
- Modify: `redox-mc-loader/src/mod_loader.rs`

**Steps:**
- [ ] Embed compiled bytecode of `GenericRedoxMenu` and `GenericRedoxMenuHelper` in `class_injector.rs`.
- [ ] Define them in captured game classloader outside JVMTI hooks.
- [ ] Implement JNI callbacks `nativeOnMenuOpen`, `nativeOnMenuSlotClick`, `nativeOnMenuClose` in `bridge_jni/events.rs` with `catch_unwind` and `PushLocalFrame`/`PopLocalFrame`.
- [ ] Register native methods on `RedoxBridge` via `RegisterNatives`.
- [ ] Forward events to active loaded mods.
- [ ] Unit tests for JNI dispatch.

---

### Task 4: Declarative Menu API in `redox-mc-api`

**Files:**
- Create: `redox-mc-api/src/gui.rs`
- Modify: `redox-mc-api/src/lib.rs`
- Test: `redox-mc-api/tests/test_menu_api.rs`

**Steps:**
- [ ] Implement `MenuDefinition` builder with `new(id)`, `slots(count)`, `title(title)`.
- [ ] Implement callback registration: `on_open`, `on_slot_click`, `on_close`.
- [ ] Implement C-ABI export symbols with `#[redox_export]`.
- [ ] Unit tests verifying builder configuration and callback routing.

---

### Task 5: Deterministic `BuiltInRegistries.MENU` and Handshake Hash in `redox-mc-loader`

**Files:**
- Create: `redox-mc-loader/src/registry/menu.rs`
- Modify: `redox-mc-loader/src/registry/mod.rs`
- Modify: `redox-mc-loader/src/registry/hash.rs`
- Modify: `redox-mc-loader/src/transformer/registry_hook.rs`
- Test: `redox-mc-loader/tests/test_menu_registration.rs`

**Steps:**
- [ ] Implement `MenuRegistry` to collect registered custom menus.
- [ ] In `BuiltInRegistries.freeze()`, invoke `GenericRedoxMenuHelper.registerMenuType(menuId, slotCount)` into `BuiltInRegistries.MENU`.
- [ ] Include menu IDs (sorted alphabetically) into `compute_full_registry_hash`.
- [ ] Unit tests verifying deterministic hash and registration dispatch.

---

### Task 6: Example Mod Wiring and Headless Verification

**Files:**
- Modify: `examples/test-mod/src/lib.rs`
- Create: `tests/test_container_menu_headless.py`
- Modify: `tests/run_task_1_2_tests.sh`

**Steps:**
- [ ] In `test-mod`, register `testmod:counter_menu` with 9 slots.
- [ ] Wire block use `on_block_use_without_item` on `testmod:counter_block` to open `testmod:counter_menu`.
- [ ] Wire `on_slot_click`: reject clicks on slot 8, allow all others.
- [ ] Create `tests/test_container_menu_headless.py` executing headless server and verifying menu registration, opening, and slot click interception.
- [ ] Wire Test 15 into `./tests/run_task_1_2_tests.sh`.
- [ ] Run full test suite and confirm all 15 tests pass.
