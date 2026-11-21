# GenericRedoxBlockEntity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Task 8.1: `GenericRedoxBlockEntity` bridge, `BlockEntityType` registration, native ticker dispatching, and persistent state serialization via Minecraft 26.3 `ValueInput`/`ValueOutput` streaming.

**Architecture:** Extend `redox-mc-bridge-game` with `GenericRedoxBlockEntity extends BlockEntity` and `GenericRedoxEntityBlock extends GenericRedoxBlock implements EntityBlock`. The bridge delegates NBT/value persistence (`saveAdditional` / `loadAdditional`) and server ticking (`BlockEntityTicker`) to Rust callbacks in `RedoxBridge` via JNI when active behavior flags are declared. Registration into `BuiltInRegistries.BLOCK_ENTITY_TYPE` is deterministic and included in the network registry handshake hash.

**Tech Stack:** Rust (2021 edition), Java 25 (classfile 69.0), Minecraft 26.3 server, JVMTI, JNI, C-ABI (`#[repr(C)]`).

**Spec:** `tasks/plan.md` (Section 5, Task 8.1) and `tasks/todo.md` (Lines 632–650).

## Global Constraints

- **HotSpot Signal Non-Interference:** Never register OS signal handlers (`SIGSEGV`, `SIGBUS`, `SIGFPE`, `SIGILL`) or VEH.
- **Bytecode Wrapper Method Renaming:** Never alter `StackMapTable` attributes in-place.
- **Deadlock-Free Class Definition:** Never call `DefineClass` inside JVMTI `ClassFileLoadHook`. Define classes lazily outside the hook.
- **Symmetric Memory Ownership:** Memory allocated by loader is freed by loader; memory allocated by mod is freed by mod (`redox_loader_free_*`, `redox_mod_free_*`).
- **Strict Panic Containment:** Every FFI boundary call must be enclosed in `catch_unwind(AssertUnwindSafe(...))` and return fallback status.
- **Zero Warnings:** All code must pass `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check`.
- **100% Tests Passing:** All existing tests in `cargo test --workspace` and integration tests must stay green.

## Review Focus

1. **Missing / Malformed NBT on Load:** If saved NBT is missing `"redox_data"`, `loadAdditional` must handle empty strings without panicking or corrupting state.
2. **BlockEntity Re-creation on Unload/Reload:** When chunks unload and reload, `newBlockEntity` must recreate a fresh entity that successfully invokes `loadAdditional`.
3. **Passive BlockEntity Overhead:** If a block entity has neither ticker nor persistence callbacks declared by the mod, JNI calls must be bypassed entirely.
4. **JNI Local Frame Safety in Tickers:** High-frequency 20 Hz ticking must never leak local references; `PushLocalFrame` / `PopLocalFrame` must wrap all tick dispatches.
5. **Deterministic Hash Consistency:** Block entity type registrations must contribute deterministically to the handshake hash across runs.

---

### Task 1: C-ABI Block Entity Signatures and Behavior Flags (`redox-mc-abi`)

**Files:**
- Modify: `redox-mc-abi/src/types.rs`
- Modify: `redox-mc-abi/src/vtable.rs`
- Test: `redox-mc-abi/tests/layout_tests.rs`

**Interfaces:**
- Consumes: `FfiStatus`, `FfiStr`, `FfiBuffer` from `redox-mc-abi::types`.
- Produces:
  - `BLOCK_ENTITY_FLAG_HAS_TICK: u32 = 1 << 0`
  - `BLOCK_ENTITY_FLAG_HAS_PERSISTENCE: u32 = 1 << 1`
  - `BlockEntityTickFn = unsafe extern "C" fn(id: FfiStr, x: i32, y: i32, z: i32) -> FfiStatus`
  - `BlockEntitySaveFn = unsafe extern "C" fn(id: FfiStr, x: i32, y: i32, z: i32, out_buf: *mut FfiBuffer) -> FfiStatus`
  - `BlockEntityLoadFn = unsafe extern "C" fn(id: FfiStr, x: i32, y: i32, z: i32, data: FfiStr) -> FfiStatus`
  - Fields in `ModVTable`: `on_block_entity_tick`, `on_block_entity_save`, `on_block_entity_load`.

- [ ] **Step 1: Write the failing test**

In `redox-mc-abi/tests/layout_tests.rs`, add:
```rust
#[test]
fn test_block_entity_abi_layout_and_flags() {
    use redox_mc_abi::types::{
        BLOCK_ENTITY_FLAG_HAS_PERSISTENCE, BLOCK_ENTITY_FLAG_HAS_TICK,
    };
    use redox_mc_abi::vtable::ModVTable;

    assert_eq!(BLOCK_ENTITY_FLAG_HAS_TICK, 1 << 0);
    assert_eq!(BLOCK_ENTITY_FLAG_HAS_PERSISTENCE, 1 << 1);

    // Verify ModVTable alignment and function pointer presence
    assert!(std::mem::size_of::<ModVTable>() >= 48);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p redox-mc-abi --test layout_tests test_block_entity_abi_layout_and_flags`
Expected: FAIL with missing constants or fields in `ModVTable`.

- [ ] **Step 3: Implement ABI flags and function pointers**

In `redox-mc-abi/src/types.rs`:
```rust
pub const BLOCK_ENTITY_FLAG_HAS_TICK: u32 = 1 << 0;
pub const BLOCK_ENTITY_FLAG_HAS_PERSISTENCE: u32 = 1 << 1;
```

In `redox-mc-abi/src/vtable.rs`:
Add function pointer types and update `ModVTable`:
```rust
pub type BlockEntityTickFn = unsafe extern "C" fn(id: FfiStr, x: i32, y: i32, z: i32) -> FfiStatus;
pub type BlockEntitySaveFn = unsafe extern "C" fn(id: FfiStr, x: i32, y: i32, z: i32, out_buf: *mut FfiBuffer) -> FfiStatus;
pub type BlockEntityLoadFn = unsafe extern "C" fn(id: FfiStr, x: i32, y: i32, z: i32, data: FfiStr) -> FfiStatus;
```
Add optional fields to `ModVTable`:
```rust
pub on_block_entity_tick: Option<BlockEntityTickFn>,
pub on_block_entity_save: Option<BlockEntitySaveFn>,
pub on_block_entity_load: Option<BlockEntityLoadFn>,
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p redox-mc-abi --test layout_tests`
Expected: PASS with 7 tests ok.

- [ ] **Step 5: Commit**

```bash
git add redox-mc-abi/src/types.rs redox-mc-abi/src/vtable.rs redox-mc-abi/tests/layout_tests.rs
git commit -m "feat(abi): define block entity flags, function pointer types, and vtable entries"
```

---

### Task 2: Java Bridge Classes for Block Entity & EntityBlock (`redox-mc-bridge`)

**Files:**
- Create: `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/block/GenericRedoxBlockEntity.java`
- Create: `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/block/GenericRedoxEntityBlock.java`
- Modify: `redox-mc-bridge/bridge-core/src/main/java/net/redoxmc/bridge/RedoxBridge.java`
- Modify: `redox-mc-bridge/build.sh`

**Interfaces:**
- Consumes:
  - `net.minecraft.world.level.block.entity.BlockEntity`
  - `net.minecraft.world.level.block.entity.BlockEntityType`
  - `net.minecraft.world.level.block.EntityBlock`
  - `net.minecraft.world.level.block.entity.BlockEntityTicker`
  - `net.minecraft.world.level.storage.ValueInput`
  - `net.minecraft.world.level.storage.ValueOutput`
- Produces:
  - `GenericRedoxBlockEntity(BlockEntityType<?>, BlockPos, BlockState, String, int)`
  - `GenericRedoxEntityBlock(BlockBehaviour.Properties, String, int, String, int)`
  - Native hooks in `RedoxBridge`:
    - `onBlockEntityTick(String id, int x, int y, int z)`
    - `onBlockEntitySave(String id, int x, int y, int z): String`
    - `onBlockEntityLoad(String id, int x, int y, int z, String data)`

- [ ] **Step 1: Declare native hooks in `RedoxBridge.java`**

In `redox-mc-bridge/bridge-core/src/main/java/net/redoxmc/bridge/RedoxBridge.java`:
```java
public static native void onBlockEntityTick(String blockEntityId, int x, int y, int z);
public static native String onBlockEntitySave(String blockEntityId, int x, int y, int z);
public static native void onBlockEntityLoad(String blockEntityId, int x, int y, int z, String data);
```

- [ ] **Step 2: Implement `GenericRedoxBlockEntity.java`**

In `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/block/GenericRedoxBlockEntity.java`:
```java
package net.redoxmc.bridge.block;

import net.minecraft.core.BlockPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.storage.ValueInput;
import net.minecraft.world.level.storage.ValueOutput;
import net.redoxmc.bridge.RedoxBridge;

public class GenericRedoxBlockEntity extends BlockEntity {
    public static final int FLAG_HAS_TICK = 1 << 0;
    public static final int FLAG_HAS_PERSISTENCE = 1 << 1;

    private final String blockEntityId;
    private final int behaviorFlags;

    public GenericRedoxBlockEntity(
            BlockEntityType<?> type,
            BlockPos pos,
            BlockState state,
            String blockEntityId,
            int behaviorFlags
    ) {
        super(type, pos, state);
        this.blockEntityId = (blockEntityId != null) ? blockEntityId : "unknown";
        this.behaviorFlags = behaviorFlags;
    }

    public String getBlockEntityId() {
        return this.blockEntityId;
    }

    public boolean hasBehavior(int flag) {
        return (this.behaviorFlags & flag) != 0;
    }

    public void serverTick(Level level, BlockPos pos, BlockState state) {
        if (hasBehavior(FLAG_HAS_TICK)) {
            int x = (pos != null) ? pos.getX() : 0;
            int y = (pos != null) ? pos.getY() : 0;
            int z = (pos != null) ? pos.getZ() : 0;
            RedoxBridge.onBlockEntityTick(this.blockEntityId, x, y, z);
        }
    }

    @Override
    protected void saveAdditional(ValueOutput output) {
        super.saveAdditional(output);
        if (hasBehavior(FLAG_HAS_PERSISTENCE)) {
            BlockPos pos = getBlockPos();
            int x = (pos != null) ? pos.getX() : 0;
            int y = (pos != null) ? pos.getY() : 0;
            int z = (pos != null) ? pos.getZ() : 0;
            String data = RedoxBridge.onBlockEntitySave(this.blockEntityId, x, y, z);
            if (data != null && !data.isEmpty()) {
                output.putString("redox_data", data);
            }
        }
    }

    @Override
    protected void loadAdditional(ValueInput input) {
        super.loadAdditional(input);
        if (hasBehavior(FLAG_HAS_PERSISTENCE)) {
            BlockPos pos = getBlockPos();
            int x = (pos != null) ? pos.getX() : 0;
            int y = (pos != null) ? pos.getY() : 0;
            int z = (pos != null) ? pos.getZ() : 0;
            String data = input.getStringOr("redox_data", "");
            RedoxBridge.onBlockEntityLoad(this.blockEntityId, x, y, z, data);
        }
    }
}
```

- [ ] **Step 3: Implement `GenericRedoxEntityBlock.java`**

In `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/block/GenericRedoxEntityBlock.java`:
```java
package net.redoxmc.bridge.block;

import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityTicker;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;

public class GenericRedoxEntityBlock extends GenericRedoxBlock implements EntityBlock {
    private final String blockEntityId;
    private final int entityBehaviorFlags;
    private BlockEntityType<GenericRedoxBlockEntity> blockEntityType;

    public GenericRedoxEntityBlock(
            BlockBehaviour.Properties properties,
            String blockId,
            int blockBehaviorFlags,
            String blockEntityId,
            int entityBehaviorFlags
    ) {
        super(properties, blockId, blockBehaviorFlags);
        this.blockEntityId = blockEntityId;
        this.entityBehaviorFlags = entityBehaviorFlags;
    }

    public void setBlockEntityType(BlockEntityType<GenericRedoxBlockEntity> type) {
        this.blockEntityType = type;
    }

    public BlockEntityType<GenericRedoxBlockEntity> getBlockEntityType() {
        return this.blockEntityType;
    }

    @Override
    public BlockEntity newBlockEntity(BlockPos pos, BlockState state) {
        return new GenericRedoxBlockEntity(
                this.blockEntityType, pos, state, this.blockEntityId, this.entityBehaviorFlags
        );
    }

    @Override
    @SuppressWarnings("unchecked")
    public <T extends BlockEntity> BlockEntityTicker<T> getTicker(
            Level level, BlockState state, BlockEntityType<T> type
    ) {
        if (level.isClientSide() || (this.entityBehaviorFlags & GenericRedoxBlockEntity.FLAG_HAS_TICK) == 0) {
            return null;
        }
        return (lvl, pos, st, be) -> {
            if (be instanceof GenericRedoxBlockEntity grbe) {
                grbe.serverTick(lvl, pos, st);
            }
        };
    }

    public static GenericRedoxEntityBlock registerWithEntity(
            String blockId,
            float destroyTime,
            float explosionResistance,
            boolean requiresTool,
            boolean noCollision,
            int blockBehaviorFlags,
            boolean registerItem,
            String blockEntityId,
            int entityBehaviorFlags
    ) {
        ResourceKey<Block> blockKey = ResourceKey.create(Registries.BLOCK, Identifier.parse(blockId));
        BlockBehaviour.Properties props = BlockBehaviour.Properties.of().setId(blockKey);
        if (destroyTime >= 0.0f || explosionResistance >= 0.0f) {
            props.strength(Math.max(0.0f, destroyTime), Math.max(0.0f, explosionResistance));
        }
        if (requiresTool) {
            props.requiresCorrectToolForDrops();
        }
        if (noCollision) {
            props.noCollision();
        }

        GenericRedoxEntityBlock block = new GenericRedoxEntityBlock(
                props, blockId, blockBehaviorFlags, blockEntityId, entityBehaviorFlags
        );

        Registry.register(BuiltInRegistries.BLOCK, Identifier.parse(blockId), block);
        for (BlockState state : block.getStateDefinition().getPossibleStates()) {
            Block.BLOCK_STATE_REGISTRY.add(state);
            state.initCache();
        }

        if (registerItem) {
            net.minecraft.world.item.Item.Properties itemProps =
                    new net.minecraft.world.item.Item.Properties().useBlockDescriptionPrefix().setId(
                            ResourceKey.create(Registries.ITEM, Identifier.parse(blockId))
                    );
            net.minecraft.world.item.BlockItem blockItem = new net.minecraft.world.item.BlockItem(block, itemProps);
            Registry.register(BuiltInRegistries.ITEM, Identifier.parse(blockId), blockItem);
            net.minecraft.world.item.Item.BY_BLOCK.put(block, blockItem);
        }

        BlockEntityType<GenericRedoxBlockEntity> beType = new BlockEntityType<>(
                (pos, state) -> new GenericRedoxBlockEntity(
                        block.getBlockEntityType(), pos, state, blockEntityId, entityBehaviorFlags
                ),
                Set.of(block)
        );
        block.setBlockEntityType(beType);
        Registry.register(BuiltInRegistries.BLOCK_ENTITY_TYPE, Identifier.parse(blockEntityId), beType);

        return block;
    }
}
```

- [ ] **Step 4: Update `redox-mc-bridge/build.sh` and compile**

Add `GenericRedoxBlockEntity.java` and `GenericRedoxEntityBlock.java` to `build.sh` javac list.
Run: `./redox-mc-bridge/build.sh`
Expected: `Build completed successfully` with updated `bridge-core.jar` and `bridge-game.jar`.

- [ ] **Step 5: Commit**

```bash
git add redox-mc-bridge/
git commit -m "feat(bridge): implement GenericRedoxBlockEntity and GenericRedoxEntityBlock with Value streaming"
```

---

### Task 3: JNI Shims & Class Injection in `redox-mc-loader`

**Files:**
- Modify: `redox-mc-loader/src/class_injector.rs`
- Modify: `redox-mc-loader/src/bridge_jni/events.rs`
- Modify: `redox-mc-loader/src/bridge_jni/register.rs`
- Test: `redox-mc-loader/tests/test_block_entity_bridge.rs`

**Interfaces:**
- Consumes: Java classes from `bridge-game.jar`, `ModVTable` from `redox-mc-abi`.
- Produces:
  - `GENERIC_REDOX_BLOCK_ENTITY_BYTES: &[u8]`
  - `GENERIC_REDOX_ENTITY_BLOCK_BYTES: &[u8]`
  - JNI exports:
    - `Java_net_redoxmc_bridge_RedoxBridge_onBlockEntityTick`
    - `Java_net_redoxmc_bridge_RedoxBridge_onBlockEntitySave`
    - `Java_net_redoxmc_bridge_RedoxBridge_onBlockEntityLoad`

- [ ] **Step 1: Write integration test for class definition and JNI exports**

In `redox-mc-loader/tests/test_block_entity_bridge.rs`:
```rust
#[test]
fn test_block_entity_class_bytes_validity() {
    use redox_mc_loader::class_injector::{
        GENERIC_REDOX_BLOCK_ENTITY_BYTES, GENERIC_REDOX_ENTITY_BLOCK_BYTES,
    };
    assert!(!GENERIC_REDOX_BLOCK_ENTITY_BYTES.is_empty());
    assert!(!GENERIC_REDOX_ENTITY_BLOCK_BYTES.is_empty());

    // Magic bytes for Java classfile 0xCAFEBABE
    assert_eq!(&GENERIC_REDOX_BLOCK_ENTITY_BYTES[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
    assert_eq!(&GENERIC_REDOX_ENTITY_BLOCK_BYTES[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
}
```

- [ ] **Step 2: Embed byte constants in `class_injector.rs`**

Add `GENERIC_REDOX_BLOCK_ENTITY_BYTES` and `GENERIC_REDOX_ENTITY_BLOCK_BYTES` using `include_bytes!`.
Update `define_game_classes` to define both classes when the game classloader is captured.

- [ ] **Step 3: Implement JNI callback functions in `events.rs`**

Implement:
- `Java_net_redoxmc_bridge_RedoxBridge_onBlockEntityTick`
- `Java_net_redoxmc_bridge_RedoxBridge_onBlockEntitySave`
- `Java_net_redoxmc_bridge_RedoxBridge_onBlockEntityLoad`
Each enclosed in `std::panic::catch_unwind(AssertUnwindSafe(...))` and pushing/popping local frames.

- [ ] **Step 4: Register native methods dynamically in `register.rs`**

Add the 3 methods to the `NATIVE_METHODS` array in `register.rs`.

- [ ] **Step 5: Run tests to verify**

Run: `cargo test -p redox-mc-loader --test test_block_entity_bridge`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add redox-mc-loader/src/class_injector.rs redox-mc-loader/src/bridge_jni/ redox-mc-loader/tests/test_block_entity_bridge.rs
git commit -m "feat(loader): embed block entity classes, register JNI handlers and dispatch to mods"
```

---

### Task 4: Declarative Block Entity API in `redox-mc-api`

**Files:**
- Create: `redox-mc-api/src/block_entity.rs`
- Modify: `redox-mc-api/src/block.rs`
- Modify: `redox-mc-api/src/lib.rs`
- Test: `redox-mc-api/tests/test_block_entity_api.rs`

**Interfaces:**
- Consumes: `redox_mc_abi::types::{BLOCK_ENTITY_FLAG_HAS_TICK, BLOCK_ENTITY_FLAG_HAS_PERSISTENCE}`
- Produces:
  - `BlockEntityDefinition`: builder with `.has_tick(bool)`, `.has_persistence(bool)`.
  - `BlockDefinition::block_entity(BlockEntityDefinition)`.
  - Dispatch handlers for tick, save, and load callbacks.

- [ ] **Step 1: Write test for declarative builder**

In `redox-mc-api/tests/test_block_entity_api.rs`:
```rust
#[test]
fn test_block_entity_declarative_builder() {
    use redox_mc_api::block::BlockDefinition;
    use redox_mc_api::block_entity::BlockEntityDefinition;

    let be = BlockEntityDefinition::new("testmod:counter_be")
        .has_tick(true)
        .has_persistence(true);

    let block = BlockDefinition::new("testmod:counter_block")
        .strength(2.0)
        .with_block_entity(be);

    assert!(block.block_entity.is_some());
    let be_def = block.block_entity.as_ref().unwrap();
    assert_eq!(be_def.id, "testmod:counter_be");
    assert!(be_def.has_tick);
    assert!(be_def.has_persistence);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p redox-mc-api --test test_block_entity_api`
Expected: FAIL with unknown module `block_entity`.

- [ ] **Step 3: Implement `BlockEntityDefinition` in `redox-mc-api`**

Create `redox-mc-api/src/block_entity.rs` and extend `BlockDefinition` in `redox-mc-api/src/block.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p redox-mc-api --test test_block_entity_api`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add redox-mc-api/src/block_entity.rs redox-mc-api/src/block.rs redox-mc-api/src/lib.rs redox-mc-api/tests/test_block_entity_api.rs
git commit -m "feat(api): add BlockEntityDefinition declarative builder and block integration"
```

---

### Task 5: Loader Registration & Handshake Hash Integration

**Files:**
- Create: `redox-mc-loader/src/registry/block_entity.rs`
- Modify: `redox-mc-loader/src/registry/mod.rs`
- Modify: `redox-mc-loader/src/registry/hash.rs`
- Test: `redox-mc-loader/tests/test_block_entity_registration.rs`

**Interfaces:**
- Consumes: `RegisteredBlockEntry` from `redox-mc-loader::registry::hash`.
- Produces:
  - `RegisteredBlockEntityEntry { id: String, valid_blocks: Vec<String>, behavior_flags: u32 }`
  - Inclusion in SHA-256 registry hash computation.
  - `register_block_entity_in_game(env, entry)`.

- [ ] **Step 1: Write test for block entity hash contribution**

In `redox-mc-loader/tests/test_block_entity_registration.rs`:
```rust
#[test]
fn test_block_entity_affects_registry_hash() {
    use redox_mc_loader::registry::hash::RegistryHashBuilder;

    let mut b1 = RegistryHashBuilder::new();
    b1.add_item("testmod:ruby", 64);
    let hash1 = b1.compute_hex();

    let mut b2 = RegistryHashBuilder::new();
    b2.add_item("testmod:ruby", 64);
    b2.add_block_entity("testmod:counter_be", &["testmod:counter_block"], 3);
    let hash2 = b2.compute_hex();

    assert_ne!(hash1, hash2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p redox-mc-loader --test test_block_entity_registration`
Expected: FAIL with missing `add_block_entity`.

- [ ] **Step 3: Implement hash calculation and registration logic**

In `redox-mc-loader/src/registry/hash.rs`: add `add_block_entity`.
Create `redox-mc-loader/src/registry/block_entity.rs` implementing `register_block_entity_in_game`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p redox-mc-loader --test test_block_entity_registration`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add redox-mc-loader/src/registry/block_entity.rs redox-mc-loader/src/registry/hash.rs redox-mc-loader/src/registry/mod.rs redox-mc-loader/tests/test_block_entity_registration.rs
git commit -m "feat(loader): include block entities in registry registration and deterministic handshake hash"
```

---

### Task 6: Example Mod Wiring & Headless Server Verification

**Files:**
- Modify: `examples/test-mod/src/lib.rs`
- Create: `tests/test_block_entity_headless.py`
- Modify: `tests/run_task_1_2_tests.sh`

**Interfaces:**
- Consumes: Headless Minecraft server (`nogui`), `test-mod` dynamic library.
- Produces:
  - `testmod:counter_block` placed via `/setblock 0 64 0 testmod:counter_block`.
  - State persistence verified across server restart.
  - Ticker verified in server log output.

- [ ] **Step 1: Register block entity in `test-mod`**

In `examples/test-mod/src/lib.rs`:
Register `testmod:counter_block` with `testmod:counter_be`, ticking every second and saving counter state to `"redox_data"`.

- [ ] **Step 2: Implement headless integration test in Python**

Create `tests/test_block_entity_headless.py`:
- Starts server with `-agentpath:libredox_mc_loader.so`.
- Runs `/setblock 0 64 0 testmod:counter_block`.
- Asserts "Changed the block at 0, 64, 0".
- Observes log for `[test-mod] BlockEntity tick at (0, 64, 0)`.
- Stops server, restarts server, and verifies persistent state recovered.

- [ ] **Step 3: Wire into `run_task_1_2_tests.sh`**

Add `python3 tests/test_block_entity_headless.py` to the end of `tests/run_task_1_2_tests.sh`.

- [ ] **Step 4: Execute verification test**

Run: `./tests/run_task_1_2_tests.sh`
Expected: All tests pass including Test 14 (Block Entity headless verification).

- [ ] **Step 5: Commit**

```bash
git add examples/test-mod/ tests/test_block_entity_headless.py tests/run_task_1_2_tests.sh
git commit -m "test(headless): verify GenericRedoxBlockEntity ticking and persistence in headless server"
```
