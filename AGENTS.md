# AGENTS.md — AI Agent Guidance & Architectural Invariants

> **For AI Assistants (Claude, Antigravity, Cursor, Copilot, etc.):**
> Read this document completely before proposing, writing, or refactoring code in `redox-mc`. This repository operates at the intersection of low-level Rust, JVM internal tool interfaces (JVMTI), JNI C-ABI, and HotSpot JIT internals. Violating these rules will cause silent memory corruption, JVM deadlocks, or process crashes.

---

## 1. Core Architectural Invariants

### 1.1 HotSpot VM Signal Non-Interference (CRITICAL)
- **Rule:** **NEVER** install custom OS signal handlers (`SIGSEGV`, `SIGBUS`, `SIGFPE`, `SIGILL`) on Unix or Vectored Exception Handlers (VEH) on Windows.
- **Why:** The HotSpot JVM relies on hardware signal traps for essential runtime mechanisms:
  - Implicit null-pointer checks (dereferences `null` and traps the `SIGSEGV` to jump to a Java `NullPointerException` handler without branch instructions).
  - JIT safepoint polling and C1/C2 deoptimization.
  - Stack guard page fault handling.
- **Enforcement:** Code in `redox-mc-loader/src/safety.rs` strictly avoids signal registration. Fault isolation must happen via `catch_unwind(AssertUnwindSafe)` and return status codes. See [ADR-0004](docs/decisions/0004-panic-isolation-boundary.md).

### 1.2 Bytecode Wrapper Method Renaming (CRITICAL)
- **Rule:** When intercepting a Minecraft method, **NEVER** attempt in-place bytecode patching or manual `StackMapTable` recalculation.
- **Technique:** Always use the **Wrapper Method Renaming Technique**:
  1. Rename vanilla method to `<name>$original`.
  2. Synthesize a replacement method `<name>` with a straight-line single basic block (pre-hook -> invoke original -> post-hook -> return).
- **Why:** Single basic blocks with no branching or exception handlers require **no `StackMapTable` attribute** under JVM Specification §4.7.4. This guarantees 100% immunity against fatal `java.lang.VerifyError` on Java 25. See [ADR-0002](docs/decisions/0002-bytecode-wrapper-hook-technique.md).

### 1.3 Deadlock-Free Class Definition
- **Rule:** **NEVER** invoke JNI `DefineClass` or trigger classloader lookups from inside JVMTI `ClassFileLoadHook`.
- **Why:** `ClassFileLoadHook` runs while holding JVM internal classloader locks. Calling `DefineClass` or resolving new classes synchronously causes an unrecoverable JVM deadlock.
- **Technique:** Capture classloaders into global JNI references (`NewGlobalRef`) during `ClassFileLoadHook`, and define custom classes (such as `bridge-game` classes) lazily outside the hook. See [ADR-0001](docs/decisions/0001-jvmti-agent-and-classloader-tiering.md).

### 1.4 Symmetric Memory Ownership Across Dynamic Libraries
- **Rule:** Memory allocated by library A must be freed by library A. **NEVER** free host-allocated memory inside a mod or mod-allocated memory inside the host using the opposing allocator.
- **Why:** Each cdylib (`redox-mc-loader` and mod `.so`/`.dll`) may use a different memory allocator or bind to a different C Runtime (CRT) heap on Windows. Freeing across boundaries corrupts the allocator heap.
- **Technique:** Use explicit vtable deallocators (`redox_loader_free_*` and `redox_mod_free_*`). Borrow data via `as_str()` or `as_slice()` across boundaries whenever possible. See [ADR-0003](docs/decisions/0003-c-abi-layout-and-symmetric-memory-ownership.md).

### 1.5 Strict Panic Containment Across FFI
- **Rule:** **NEVER** allow a Rust panic to unwind across an `extern "C"` boundary.
- **Why:** In Rust, unwinding across FFI is undefined behavior and aborts the entire process.
- **Technique:** Annotate all exported mod entry points with `#[redox_export]` from `redox-mc-api-macros`. It wraps function bodies in `std::panic::catch_unwind(std::panic::AssertUnwindSafe(...))` and returns a safe fallback code. See [ADR-0004](docs/decisions/0004-panic-isolation-boundary.md).

### 1.6 Legal & Distribution Integrity
- **Rule:** **NEVER** commit, bundle, or distribute proprietary Mojang/Microsoft assets, classes, or decompiled vanilla jars into this repository.
- **Technique:** All hooks and adapters use offsets, names, or runtime reflection/JNI discovery against the user's legally acquired local game files.

---

## 2. Quality Bar & Verification Standards

Agents must maintain a strict, non-negotiable quality bar:

1. **Zero Warnings:**
   - Code must pass `cargo clippy --workspace --all-targets -- -D warnings` with zero warnings.
   - Code must pass `cargo fmt --all -- --check`.
2. **No Suppression:**
   - Do **NOT** add `#![allow(...)]`, `#[allow(dead_code)]`, or `#[allow(unused)]` to silence warnings. Fix the underlying issue cleanly.
   - Do **NOT** delete, skip, or disable existing tests to get to green.
3. **100% Test Pass Rate:**
   - Run `cargo test --workspace` before claiming completion.
   - When modifying lifecycle hooks or JVM integration, execute the integration suite:
     ```bash
     ./tests/run_task_1_2_tests.sh
     ```

---

## 3. Crate Architecture Guidelines

| Crate | Architectural Role | Allowed Dependencies |
|---|---|---|
| `redox-mc-abi` | Pure C-ABI types (`#[repr(C)]`), vtables, error codes. | Minimal (e.g. `thiserror`). **No JNI, no heavy crates**. |
| `redox-mc-api-macros` | Procedural macro `#[redox_export]`. | `syn`, `quote`, `proc-macro2`. |
| `redox-mc-api` | Public modding SDK, event bus, safe wrappers. | `redox-mc-abi`, `redox-mc-api-macros`. |
| `redox-mc-bindgen` | Offline classfile parser & binding generator. | `zip`, `std`. |
| `redox-mc-adapter-api` | Abstract hook trait interfaces. | Minimal traits. |
| `redox-mc-adapter-26-3` | Minecraft 26.3 hook implementations. | `redox-mc-adapter-api`. |
| `redox-mc-loader` | JVMTI agent, transformer, safety layer. | `jni`, `libloading`, adapters, abi. |

---

## 4. Key Gotchas & Anti-Patterns

- **JNI Local Frame Exhaustion:** When calling JNI methods in loops (e.g. iterating over registry entries or entity lists), call `env.push_local_frame(...)` / `env.pop_local_frame(...)` or `env.delete_local_ref(...)`. HotSpot defaults to a limit of 512 local references per frame.
- **String Encodings:** JVM modified UTF-8 is not standard UTF-8 (it encodes `\0` as `0xC0 0x80`). Use standard JNI string helpers or sanitize before converting to Rust `&str`.
- **Field & Method ID Caching:** Lookups via `GetMethodID` and `GetFieldID` are slow. In generated bindings or critical loops, cache IDs in `AtomicPtr` after the first lookup.
- **Thread Attachment:** Do not assume a native thread is attached to the JVM. Check or attach using `JavaVM::attach_current_thread_as_daemon()` before executing JNI calls.
